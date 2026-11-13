//! The download of the published lists.

use std::time::Duration;

use ipnet::IpNet;
use reqwest::{Client, StatusCode};

use crate::error::{ParseError, RefreshFailure};
use crate::feed::parsers;
use crate::http;

/// How many times a transient refusal is retried.
const FETCH_RETRIES: usize = 2;

/// The delay between those retries.
const FETCH_RETRY_DELAY: Duration = Duration::from_secs(3);

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
            Ok(body) => match parsers::parse(parser, &body) {
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
                        failure = Some(classify(error));
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

/// Classifies a body that could not be read.
fn classify(error: ParseError) -> RefreshFailure {
    match error {
        ParseError::NotADocument(message) => RefreshFailure::Unreachable(message),
        ParseError::UnexpectedShape(message) => RefreshFailure::Broken(message),
    }
}

/// Fetches the body of a document.
async fn fetch(http: &Client, url: &str) -> Result<Vec<u8>, RefreshFailure> {
    let mut attempt = 0;
    loop {
        let response = http
            .get(url)
            .send()
            .await
            .map_err(|error| http::map_transport_error(&error, url))?;
        let status = response.status();
        if status.is_success() {
            return response
                .bytes()
                .await
                .map(|body| body.to_vec())
                .map_err(|error| RefreshFailure::Broken(format!("{url}: {error}")));
        }
        // A rate limit or a server error is transient — a CDN hiccup or an
        // upstream that is busy. A deliberate refusal (403, 404) only wastes
        // time when it is retried.
        let transient = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
        if !transient || attempt >= FETCH_RETRIES {
            // The vendor answered and refused us: rate limit, bot protection or
            // an endpoint that moved. Environmental.
            return Err(RefreshFailure::Unreachable(format!(
                "{url} returned {status}"
            )));
        }
        attempt += 1;
        let wait = http::parse_retry_after(
            response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok()),
        )
        .unwrap_or(FETCH_RETRY_DELAY);
        tracing::warn!(
            url,
            %status,
            attempt,
            "knownbots: the endpoint refused the request; retrying"
        );
        tokio::time::sleep(wait).await;
    }
}
