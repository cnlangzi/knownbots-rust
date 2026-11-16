//! The download of the published lists.

use std::time::Duration;

use ipnet::IpNet;
use reqwest::header::{CONTENT_TYPE, RETRY_AFTER};
use reqwest::{Client, Response, StatusCode};

use crate::error::{ParseError, RefreshFailure};
use crate::feed::parsers;
use crate::http;

/// How many times a transient refusal is retried.
const FETCH_RETRIES: usize = 2;

/// The delay between those retries.
const FETCH_RETRY_DELAY: Duration = Duration::from_secs(3);

/// The number of bytes of a body a message carries.
const EXCERPT_LIMIT: usize = 64;

/// The prefixes a download produced and the failure that stopped the rest.
#[derive(Debug, Default)]
pub struct Downloaded {
    /// The prefixes the documents that answered hold.
    pub prefixes: Vec<IpNet>,
    /// The failure of the document that did not answer.
    pub failure: Option<RefreshFailure>,
}

/// Downloads the documents of a bot and reads the prefixes they hold.
///
/// A document that fails does not discard the documents that answered: the
/// prefixes they hold are kept and the failure is reported.
pub async fn download_prefixes(http: &Client, parser: &str, urls: &[String]) -> Downloaded {
    let mut prefixes = Vec::new();
    let mut failure = None;

    for url in urls {
        match fetch(http, url).await {
            Ok(answer) => match parsers::parse(parser, &answer.body) {
                Ok(nets) if nets.is_empty() => {
                    if failure.is_none() {
                        failure = Some(RefreshFailure::Broken(format!(
                            "the list at {url} holds no usable prefix"
                        )));
                    }
                }
                Ok(nets) => prefixes.extend(nets),
                Err(error) => {
                    if failure.is_none() {
                        failure = Some(classify(url, &answer, error));
                    }
                }
            },
            Err(refusal) => {
                if failure.is_none() {
                    failure = Some(refusal);
                }
            }
        }
    }

    if prefixes.is_empty() && failure.is_none() {
        failure = Some(RefreshFailure::Broken(
            "the list holds no usable prefix".to_string(),
        ));
    }

    Downloaded { prefixes, failure }
}

/// The answer of an endpoint: the status, the content type and the body.
struct Answer {
    /// The status of the response.
    status: StatusCode,
    /// The content type of the response.
    content_type: String,
    /// The body of the response.
    body: Vec<u8>,
}

/// Classifies a body that could not be read.
///
/// A body that is not the document of the vendor is a refusal of the endpoint:
/// the message names what the endpoint answered. A document whose shape
/// changed is a fact of the crate that has to follow it.
fn classify(url: &str, answer: &Answer, error: ParseError) -> RefreshFailure {
    match error {
        ParseError::NotADocument(_) => RefreshFailure::Unreachable(format!(
            "{url} answered {} with {}: {}",
            answer.status.as_u16(),
            answer.content_type,
            excerpt(&answer.body)
        )),
        ParseError::UnexpectedShape(reason) => RefreshFailure::Broken(format!("{url}: {reason}")),
    }
}

/// Fetches the body of a document.
async fn fetch(http: &Client, url: &str) -> Result<Answer, RefreshFailure> {
    let mut attempt = 0;
    loop {
        let response = http
            .get(url)
            .send()
            .await
            .map_err(|error| http::map_transport_error(&error, url))?;
        let status = response.status();
        let content_type = content_type(&response);
        let retry_after = http::parse_retry_after(
            response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok()),
        );
        // A status other than 200 is a refusal of the endpoint, not a document
        // the crate mis-reads.
        if status == StatusCode::OK {
            let body = response
                .bytes()
                .await
                .map(|body| body.to_vec())
                .map_err(|error| RefreshFailure::Broken(format!("{url}: {error}")))?;
            return Ok(Answer {
                status,
                content_type,
                body,
            });
        }
        // A rate limit or a server error is transient — a CDN hiccup or an
        // upstream that is busy. A deliberate refusal (403, 404) only wastes
        // time when it is retried.
        let transient = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
        if !transient || attempt >= FETCH_RETRIES {
            // The vendor answered and refused us: rate limit, bot protection or
            // an endpoint that moved. Environmental.
            return Err(refusal(url, status, &content_type, response).await);
        }
        attempt += 1;
        tracing::warn!(
            url,
            %status,
            attempt,
            "knownbots: the endpoint refused the request; retrying"
        );
        tokio::time::sleep(retry_after.unwrap_or(FETCH_RETRY_DELAY)).await;
    }
}

/// Names the answer of an endpoint that refused the request.
///
/// The body of the refusal is not read in full: the message carries an excerpt
/// of it, and the body of an endpoint is not bounded. A body that cannot be
/// read does not change the refusal — the endpoint has refused us either way.
async fn refusal(
    url: &str,
    status: StatusCode,
    content_type: &str,
    mut response: Response,
) -> RefreshFailure {
    let body = response.chunk().await.ok().flatten().unwrap_or_default();
    RefreshFailure::Unreachable(refusal_message(url, status, content_type, &body))
}

/// The message of a refusal: what the endpoint answered.
fn refusal_message(url: &str, status: StatusCode, content_type: &str, body: &[u8]) -> String {
    if body.is_empty() {
        format!("{url} returned {status}")
    } else {
        format!(
            "{url} returned {status} ({content_type}): {}",
            excerpt(body)
        )
    }
}

/// The content type of a response.
fn content_type(response: &Response) -> String {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("no content type")
        .to_string()
}

/// The first bytes of a body, for a message.
///
/// The excerpt is bounded and printable: the body of an endpoint is not
/// bounded, and a message has to stay short and readable.
fn excerpt(body: &[u8]) -> String {
    if body.is_empty() {
        return "an empty body".to_string();
    }
    let cut = body.len() > EXCERPT_LIMIT;
    let text = String::from_utf8_lossy(&body[..excerpt_end(body)]);
    let mut excerpt: String = text
        .chars()
        .map(|character| {
            if character.is_control() {
                '·'
            } else {
                character
            }
        })
        .collect();
    if cut {
        excerpt.push('…');
    }
    excerpt
}

/// The end of the excerpt of a body: the limit, on a boundary of a character.
///
/// A body that is cut in the middle of a character would carry a replacement
/// character into the message.
fn excerpt_end(body: &[u8]) -> usize {
    let mut end = body.len().min(EXCERPT_LIMIT);
    while end > 0 && end < body.len() && (body[end] & 0xC0) == 0x80 {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The excerpt of a long body is cut and marked.
    #[test]
    fn the_excerpt_of_a_long_body_is_cut() {
        let body = vec![b'a'; 200];
        let text = excerpt(&body);
        assert!(text.ends_with('…'), "{text}");
        assert_eq!(text.trim_end_matches('…').len(), EXCERPT_LIMIT);
    }

    /// The message of a refusal names the status of the endpoint and the
    /// answer of its body.
    #[test]
    fn the_message_of_a_refusal_names_the_answer_of_the_endpoint() {
        assert_eq!(
            refusal_message(
                "https://vendor/list.json",
                StatusCode::FORBIDDEN,
                "text/xml",
                b"OK"
            ),
            "https://vendor/list.json returned 403 Forbidden (text/xml): OK"
        );
    }

    /// A refusal without a body names the status alone.
    #[test]
    fn the_message_of_a_refusal_without_a_body_names_the_status() {
        assert_eq!(
            refusal_message(
                "https://vendor/list.json",
                StatusCode::FORBIDDEN,
                "text/xml",
                b""
            ),
            "https://vendor/list.json returned 403 Forbidden"
        );
    }

    /// The control characters of a body do not reach the message.
    #[test]
    fn the_excerpt_replaces_the_control_characters() {
        assert_eq!(excerpt(b"a\x00b\nc"), "a·b·c");
    }

    /// An excerpt does not cut a character in half.
    #[test]
    fn the_excerpt_cuts_on_a_character_boundary() {
        let mut body = vec![b'a'; 63];
        body.extend_from_slice("é".as_bytes());
        let text = excerpt(&body);
        assert!(text.ends_with('…'), "{text}");
        assert!(!text.contains('\u{fffd}'), "{text}");
        assert_eq!(text.len(), 63 + '…'.len_utf8());
    }

    /// An empty body says so.
    #[test]
    fn the_excerpt_of_an_empty_body_says_so() {
        assert_eq!(excerpt(b""), "an empty body");
    }
}
