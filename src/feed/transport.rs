//! The transport of the crate: the clients it downloads with and the memory of
//! the family that delivered the document.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ipnet::IpNet;
use parking_lot::Mutex;
use reqwest::header::{CONTENT_TYPE, RETRY_AFTER};
use reqwest::{Client, Response, StatusCode};

use crate::error::{ParseError, RefreshFailure};
use crate::feed::parsers;
use crate::http::{AddressFamily, ClientSet, map_transport_error, parse_retry_after};

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

/// The transport the crate downloads the lists with.
///
/// The transport owns the clients and the memory of the family that delivered
/// the document of a host. A download that the endpoint refused is repeated
/// over the other address family, so that a path that is tampered with is
/// repaired whichever path it is. The family that delivered the document is
/// remembered for the host, so that the next download of that host does not
/// pay for the refusal again.
#[derive(Clone)]
pub struct Transport {
    /// The clients the crate downloads the lists with.
    clients: ClientSet,
    /// The family that delivered the document of a host.
    families: Arc<Mutex<HashMap<String, AddressFamily>>>,
}

impl Transport {
    /// Builds a transport that downloads the lists with the clients.
    pub fn new(clients: ClientSet) -> Self {
        Self {
            clients,
            families: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Downloads the documents of a bot and reads the prefixes they hold.
    ///
    /// A document that fails does not discard the documents that answered: the
    /// prefixes they hold are kept and the failure is reported.
    pub async fn download(&self, parser: &str, urls: &[String]) -> Downloaded {
        let mut prefixes = Vec::new();
        let mut failure = None;

        for url in urls {
            match self.download_one(parser, url).await {
                Ok(nets) => prefixes.extend(nets),
                Err(error) => {
                    if failure.is_none() {
                        failure = Some(error);
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

    /// Downloads one document and reads the prefixes it holds.
    ///
    /// The download is repeated over the other address family when the endpoint
    /// refused the answer or when the route to the vendor failed. An answer
    /// the upstream decided itself is never repeated: it is the vendor's
    /// answer about the request.
    async fn download_one(&self, parser: &str, url: &str) -> Result<Vec<IpNet>, RefreshFailure> {
        let host = host_of(url);
        let remembered = self.family_of(&host);
        // The families that are known to be bad: the ones that refused and the
        // ones whose route failed.
        let mut failed: Vec<AddressFamily> = Vec::new();
        let mut refusal: Option<Refusal> = None;
        let mut decided: Option<RefreshFailure> = None;
        let mut route: Option<RefreshFailure> = None;
        let mut last_peer: Option<AddressFamily> = None;
        let mut round = 0;

        while let Some(choice) = next_choice(round, remembered, last_peer, &failed) {
            let (client, family) = match choice {
                Choice::Normal => (self.clients.normal(), None),
                Choice::Pinned(family) => (self.clients.pinned(family), Some(family)),
            };

            match attempt(client, parser, url).await {
                Outcome::Document { prefixes, peer } => {
                    // The family that delivered the document is remembered for
                    // the host: the lists of a vendor share a host, and the
                    // next download of that host uses the pinned client of
                    // that family.
                    if let Some(delivered) = family.or(peer) {
                        self.remember(&host, delivered);
                    }
                    if let Some(refusal) = &refusal {
                        // The other address family repaired the refusal.
                        tracing::warn!(
                            url = %sanitized_url(url),
                            status = %refusal.status,
                            content_type = %refusal.content_type,
                            excerpt = %refusal.excerpt,
                            "knownbots: the endpoint refused the request; the other address family delivered the list"
                        );
                    }
                    return Ok(prefixes);
                }
                Outcome::Broken(failure) => return Err(failure),
                Outcome::Decided(failure) => {
                    // The upstream decided about the request: the download is
                    // never repeated.
                    decided = Some(failure);
                    break;
                }
                Outcome::Refusal(answer) => {
                    if let Some(peer) = answer.peer {
                        failed.push(peer);
                        last_peer = Some(peer);
                    }
                    if refusal.is_none() {
                        refusal = Some(answer);
                    }
                }
                Outcome::Route(failure) => {
                    if route.is_none() {
                        route = Some(failure);
                    }
                    if let Some(family) = family {
                        failed.push(family);
                        if remembered == Some(family) {
                            // The route of the remembered family is broken:
                            // the family is forgotten, and the next download of
                            // the host starts with the unbound client.
                            self.forget(&host);
                        }
                    }
                }
            }
            round += 1;
        }

        // The failure that is closest to the vendor's own answer wins: an
        // answer the upstream decided itself beats a refusal, and a refusal
        // beats a transport failure.
        Err(decided
            .or_else(|| refusal.map(|refusal| RefreshFailure::Unreachable(refusal.message)))
            .or(route)
            .unwrap_or_else(|| RefreshFailure::Broken(format!("{url} could not be downloaded"))))
    }

    /// The family that delivered the document of the host.
    fn family_of(&self, host: &str) -> Option<AddressFamily> {
        self.families.lock().get(host).copied()
    }

    /// Remembers the family that delivered the document of the host.
    fn remember(&self, host: &str, family: AddressFamily) {
        self.families.lock().insert(host.to_string(), family);
    }

    /// Forgets the family of the host.
    fn forget(&self, host: &str) {
        self.families.lock().remove(host);
    }
}

/// The client of an attempt.
enum Choice {
    /// The client that is bound to no family.
    Normal,
    /// The client that is bound to the family.
    Pinned(AddressFamily),
}

/// The client of the next attempt, or `None` when no family is left.
///
/// The first attempt uses the pinned client of the remembered family, or the
/// unbound client when the host has no remembered family. A repetition uses
/// the other family: the family of the peer that answered, the other family of
/// the remembered one when the peer is unknown, and the family that is left
/// when nothing is known.
fn next_choice(
    attempt: usize,
    remembered: Option<AddressFamily>,
    last_peer: Option<AddressFamily>,
    failed: &[AddressFamily],
) -> Option<Choice> {
    let family = match attempt {
        0 => {
            return Some(match remembered {
                Some(family) => Choice::Pinned(family),
                None => Choice::Normal,
            });
        }
        1 => match last_peer.or(remembered) {
            Some(family) => family.other(),
            None => first_untried(failed)?,
        },
        _ => first_untried(failed)?,
    };
    Some(Choice::Pinned(family))
}

/// The first of the two families that was not tried yet.
fn first_untried(failed: &[AddressFamily]) -> Option<AddressFamily> {
    [AddressFamily::Ipv4, AddressFamily::Ipv6]
        .into_iter()
        .find(|family| !failed.contains(family))
}

/// What an attempt at one document produced.
enum Outcome {
    /// The document of the vendor arrived.
    Document {
        /// The prefixes the document holds.
        prefixes: Vec<IpNet>,
        /// The family of the peer that answered.
        peer: Option<AddressFamily>,
    },
    /// The endpoint answered with something that is not the list.
    Refusal(Refusal),
    /// The upstream decided about the request.
    Decided(RefreshFailure),
    /// The route to the vendor failed.
    Route(RefreshFailure),
    /// The document arrived in a shape the crate mis-reads.
    Broken(RefreshFailure),
}

/// A refusal of an endpoint: the answer that is not the list of the vendor.
struct Refusal {
    /// The message the host reads.
    message: String,
    /// The status of the answer.
    status: StatusCode,
    /// The content type of the answer.
    content_type: String,
    /// The excerpt of the body of the answer.
    excerpt: String,
    /// The family of the peer that answered.
    peer: Option<AddressFamily>,
}

impl Refusal {
    /// The refusal of a status that is not 200.
    fn status(
        url: &str,
        status: StatusCode,
        content_type: String,
        body: &[u8],
        peer: Option<AddressFamily>,
    ) -> Self {
        let message = if body.is_empty() {
            format!("{url} returned {status}")
        } else {
            format!(
                "{url} returned {status} ({content_type}): {}",
                excerpt(body)
            )
        };
        Self {
            message,
            status,
            content_type,
            excerpt: excerpt(body),
            peer,
        }
    }

    /// The refusal of a 200 whose body the parser does not read as the list.
    fn document(url: &str, content_type: String, body: &[u8], peer: Option<AddressFamily>) -> Self {
        let message = format!(
            "{url} answered {} with {content_type}: {}",
            StatusCode::OK.as_u16(),
            excerpt(body)
        );
        Self {
            message,
            status: StatusCode::OK,
            content_type,
            excerpt: excerpt(body),
            peer,
        }
    }
}

/// Sends one request and classifies the answer.
///
/// A status of the 2xx range other than 200 is a refusal: an answer that is
/// not the list of the vendor. A status the upstream decided itself — 401,
/// 403, 404, 429 and the server errors — is never a refusal. A rate limit
/// and a server error are transient, and they are retried on the same family.
async fn attempt(client: &Client, parser: &str, url: &str) -> Outcome {
    let mut retry = 0;
    loop {
        let mut response = match client.get(url).send().await {
            Ok(response) => response,
            Err(error) => return Outcome::Route(map_transport_error(&error, url)),
        };
        let status = response.status();
        let content_type = content_type(&response);
        let peer = response
            .remote_addr()
            .map(|address| AddressFamily::of(address.ip()));

        if status == StatusCode::OK {
            let body = match response.bytes().await {
                Ok(body) => body.to_vec(),
                Err(error) => {
                    return Outcome::Broken(RefreshFailure::Broken(format!("{url}: {error}")));
                }
            };
            return match parsers::parse(parser, &body) {
                Ok(nets) if nets.is_empty() => Outcome::Broken(RefreshFailure::Broken(format!(
                    "the list at {url} holds no usable prefix"
                ))),
                Ok(nets) => Outcome::Document {
                    prefixes: nets,
                    peer,
                },
                Err(ParseError::NotADocument(_)) => {
                    Outcome::Refusal(Refusal::document(url, content_type, &body, peer))
                }
                Err(ParseError::UnexpectedShape(reason)) => {
                    Outcome::Broken(RefreshFailure::Broken(format!("{url}: {reason}")))
                }
            };
        }

        // A rate limit or a server error is transient — a CDN hiccup or an
        // upstream that is busy. A deliberate refusal (403, 404) only wastes
        // time when it is retried. The retry stays on the same family: a busy
        // CDN is busy on both families.
        let retry_after = parse_retry_after(
            response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok()),
        );
        let transient = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
        if transient && retry < FETCH_RETRIES {
            retry += 1;
            tracing::warn!(
                url,
                %status,
                attempt = retry,
                "knownbots: the endpoint refused the request; retrying"
            );
            tokio::time::sleep(retry_after.unwrap_or(FETCH_RETRY_DELAY)).await;
            continue;
        }

        // The body of a refusal is not read in full: the message carries an
        // excerpt of it, and the body of an endpoint is not bounded. A body
        // that cannot be read does not change the refusal — the endpoint has
        // refused us either way.
        let body = response.chunk().await.ok().flatten().unwrap_or_default();
        let refusal = Refusal::status(url, status, content_type, &body, peer);
        return if status.is_success() {
            Outcome::Refusal(refusal)
        } else {
            Outcome::Decided(RefreshFailure::Unreachable(refusal.message))
        };
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

/// The host of a URL, without its scheme.
///
/// The lists of a vendor are published on one host, and they share the family
/// that delivered the document.
fn host_of(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(parsed) => parsed
            .host_str()
            .map(str::to_string)
            .unwrap_or_else(|| parsed.to_string()),
        Err(_) => url.to_string(),
    }
}

/// The URL of a message: without its query and without its fragment.
fn sanitized_url(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(mut parsed) => {
            parsed.set_query(None);
            parsed.set_fragment(None);
            parsed.to_string()
        }
        Err(_) => url.to_string(),
    }
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
    use std::io::{Read, Write};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// The list the stub of the document answers with.
    const LIST: &str = r#"{"prefixes":[{"ipv4Prefix":"1.2.3.0/24"}]}"#;

    /// The answer of a path that is tampered with: a status of the 2xx range
    /// other than 200, and a body that is not the list.
    fn refusal() -> Vec<u8> {
        response(
            "203 Non-Authoritative Information",
            &[("Content-Type", "text/xml")],
            "OK",
        )
    }

    /// The answer of a vendor that refuses the request.
    fn forbidden() -> Vec<u8> {
        response("403 Forbidden", &[("Content-Type", "text/xml")], "")
    }

    /// The answer of a vendor that limits the rate of the requests.
    fn rate_limit() -> Vec<u8> {
        response(
            "429 Too Many Requests",
            &[("Content-Type", "text/xml"), ("Retry-After", "0")],
            "",
        )
    }

    /// The answer of a vendor that publishes its list.
    fn document() -> Vec<u8> {
        response("200 OK", &[("Content-Type", "application/json")], LIST)
    }

    /// A document whose shape changed: it holds no list.
    fn changed() -> Vec<u8> {
        response(
            "200 OK",
            &[("Content-Type", "application/json")],
            r#"{"creationTime":"2026-01-01T00:00:00Z"}"#,
        )
    }

    /// The answer of an endpoint that announces a length it does not deliver.
    fn truncated() -> Vec<u8> {
        let mut answer =
            b"HTTP/1.1 403 Forbidden\r\nContent-Type: text/xml\r\nContent-Length: 100\r\n\r\nOK"
                .to_vec();
        answer.truncate(answer.len() - 2);
        answer
    }

    /// The answer of a stub: the status line, the headers and the body.
    fn response(status: &str, headers: &[(&str, &str)], body: &str) -> Vec<u8> {
        let mut answer = format!("HTTP/1.1 {status}\r\n");
        for (name, value) in headers {
            answer.push_str(&format!("{name}: {value}\r\n"));
        }
        answer.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
        answer.push_str(body);
        answer.into_bytes()
    }

    /// A stub that answers with a canned response and counts the requests.
    struct Stub {
        /// The port it listens on.
        port: u16,
        /// The number of requests it received.
        requests: Arc<AtomicUsize>,
    }

    impl Stub {
        /// Starts a stub on the address, or answers `None` by dropping the
        /// connection.
        fn start(address: &str, response: Option<&[u8]>) -> std::io::Result<Stub> {
            let listener = std::net::TcpListener::bind(address)?;
            let port = listener.local_addr()?.port();
            let requests = Arc::new(AtomicUsize::new(0));
            let counter = requests.clone();
            let response = response.map(<[u8]>::to_vec);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    counter.fetch_add(1, Ordering::SeqCst);
                    let mut buffer = [0_u8; 1024];
                    let _ = stream.read(&mut buffer);
                    if let Some(response) = &response {
                        let _ = stream.write_all(response);
                        let _ = stream.flush();
                    }
                }
            });
            Ok(Stub { port, requests })
        }

        /// The number of requests the stub received.
        fn requests(&self) -> usize {
            self.requests.load(Ordering::SeqCst)
        }
    }

    /// Two stubs that share a port: one on each family of the loopback.
    async fn stubs(ipv4: Option<Vec<u8>>, ipv6: Option<Vec<u8>>) -> (Stub, Stub) {
        for _ in 0..16 {
            let Ok(first) = Stub::start("127.0.0.1:0", ipv4.as_deref()) else {
                continue;
            };
            if let Ok(second) = Stub::start(&format!("[::1]:{}", first.port), ipv6.as_deref()) {
                return (first, second);
            }
        }
        panic!("the stubs of the test could not share a port");
    }

    /// A resolver that answers with the loopback of both families.
    struct Loopback;

    impl reqwest::dns::Resolve for Loopback {
        fn resolve(&self, _name: reqwest::dns::Name) -> reqwest::dns::Resolving {
            Box::pin(async move {
                let addrs: reqwest::dns::Addrs = Box::new(
                    vec![
                        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
                        SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
                    ]
                    .into_iter(),
                );
                Ok(addrs)
            })
        }
    }

    /// The clients of the test: they reach the loopback of their family.
    ///
    /// The resolver answers with the IPv4 address first, so the unbound client
    /// prefers that family.
    fn clients() -> ClientSet {
        let unbound = reqwest::Client::builder()
            .no_proxy()
            .dns_resolver(Arc::new(Loopback))
            .build()
            .expect("the unbound client");
        let ipv4 = reqwest::Client::builder()
            .no_proxy()
            .dns_resolver(Arc::new(Loopback))
            .local_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
            .build()
            .expect("the IPv4 client");
        let ipv6 = reqwest::Client::builder()
            .no_proxy()
            .dns_resolver(Arc::new(Loopback))
            .local_address(IpAddr::V6(Ipv6Addr::UNSPECIFIED))
            .build()
            .expect("the IPv6 client");
        ClientSet::new(unbound, ipv4, ipv6)
    }

    /// The URL of a document of the host the stubs answer for.
    fn url(port: u16) -> String {
        format!("http://vendor.test:{port}/ips.json")
    }

    /// A refusal of the endpoint is repeated over the other family, and the
    /// document of the second answer is delivered.
    #[tokio::test]
    async fn a_refused_document_is_repaired_over_the_other_family() {
        let (ipv4, ipv6) = stubs(Some(refusal()), Some(document())).await;
        let transport = Transport::new(clients());

        let downloaded = transport.download("google", &[url(ipv4.port)]).await;

        assert_eq!(downloaded.prefixes.len(), 1, "{downloaded:?}");
        assert!(downloaded.failure.is_none(), "{downloaded:?}");
        assert_eq!(ipv4.requests(), 1, "the refusal was asked once");
        assert_eq!(ipv6.requests(), 1, "the other family was asked once");
    }

    /// The family that delivered the document is remembered for the host, and
    /// the next download of that host uses the pinned client of that family.
    #[tokio::test]
    async fn the_family_that_delivered_the_document_is_remembered() {
        let (ipv4, ipv6) = stubs(Some(refusal()), Some(document())).await;
        let transport = Transport::new(clients());

        let downloaded = transport.download("google", &[url(ipv4.port)]).await;
        assert_eq!(downloaded.prefixes.len(), 1, "{downloaded:?}");

        let downloaded = transport.download("google", &[url(ipv4.port)]).await;
        assert_eq!(downloaded.prefixes.len(), 1, "{downloaded:?}");
        assert_eq!(ipv4.requests(), 1, "the refused family was asked again");
        assert_eq!(ipv6.requests(), 2, "the family of the list was asked twice");
    }

    /// An answer the upstream decided itself is never repeated over the other
    /// family.
    #[tokio::test]
    async fn a_decided_answer_is_never_repeated() {
        let (ipv4, ipv6) = stubs(Some(forbidden()), Some(document())).await;
        let transport = Transport::new(clients());

        let downloaded = transport.download("google", &[url(ipv4.port)]).await;

        let failure = downloaded.failure.expect("the refresh must fail");
        assert!(
            matches!(failure, RefreshFailure::Unreachable(_)),
            "{failure:?}"
        );
        assert_eq!(ipv4.requests(), 1);
        assert_eq!(ipv6.requests(), 0, "the other family was asked");
    }

    /// A document whose shape changed is a failure of the crate, and it is
    /// never repeated over the other family.
    #[tokio::test]
    async fn a_document_whose_shape_changed_is_never_repeated() {
        let (ipv4, ipv6) = stubs(Some(changed()), Some(document())).await;
        let transport = Transport::new(clients());

        let downloaded = transport.download("google", &[url(ipv4.port)]).await;

        let failure = downloaded.failure.expect("the refresh must fail");
        assert!(matches!(failure, RefreshFailure::Broken(_)), "{failure:?}");
        assert_eq!(ipv4.requests(), 1);
        assert_eq!(ipv6.requests(), 0, "the other family was asked");
    }

    /// A route that fails is repeated over the other family, where the
    /// endpoint answers with the document.
    #[tokio::test]
    async fn a_failed_route_is_repeated_over_the_other_family() {
        let (ipv4, ipv6) = stubs(None, Some(document())).await;
        let transport = Transport::new(clients());
        // The host is reached over IPv4, and the route of that family is
        // broken.
        transport.remember("vendor.test", AddressFamily::Ipv4);

        let downloaded = transport.download("google", &[url(ipv4.port)]).await;

        assert_eq!(downloaded.prefixes.len(), 1, "{downloaded:?}");
        assert!(downloaded.failure.is_none(), "{downloaded:?}");
        assert_eq!(ipv4.requests(), 1);
        assert_eq!(ipv6.requests(), 1, "the other family was asked once");
    }

    /// A route that fails on every family reports the failure of the first
    /// attempt.
    #[tokio::test]
    async fn a_host_that_cannot_be_reached_reports_the_first_failure() {
        let (ipv4, ipv6) = stubs(None, None).await;
        let transport = Transport::new(clients());
        transport.remember("vendor.test", AddressFamily::Ipv4);

        let downloaded = transport.download("google", &[url(ipv4.port)]).await;

        let failure = downloaded.failure.expect("the refresh must fail");
        assert!(failure.to_string().contains(&url(ipv4.port)), "{failure:?}");
        assert_eq!(ipv4.requests(), 1);
        assert_eq!(ipv6.requests(), 1, "the other family was asked once");
    }

    /// A refusal of the first attempt and an answer the upstream decided
    /// itself: the status of the second attempt is reported.
    #[tokio::test]
    async fn a_decided_answer_beats_a_refusal() {
        let (ipv4, _ipv6) = stubs(Some(refusal()), Some(forbidden())).await;
        let transport = Transport::new(clients());

        let downloaded = transport.download("google", &[url(ipv4.port)]).await;

        let failure = downloaded.failure.expect("the refresh must fail");
        let message = failure.to_string();
        assert!(message.contains("403"), "{message}");
        assert!(!message.contains("203"), "{message}");
    }

    /// A refusal of the first attempt and a route that fails: the refusal of
    /// the first attempt is reported.
    #[tokio::test]
    async fn a_refusal_beats_a_failed_route() {
        let (ipv4, _ipv6) = stubs(Some(refusal()), None).await;
        let transport = Transport::new(clients());

        let downloaded = transport.download("google", &[url(ipv4.port)]).await;

        let failure = downloaded.failure.expect("the refresh must fail");
        assert!(
            matches!(failure, RefreshFailure::Unreachable(_)),
            "{failure:?}"
        );
        let message = failure.to_string();
        assert!(message.contains(&url(ipv4.port)), "{message}");
        assert!(message.contains("203"), "{message}");
        assert!(message.contains("text/xml"), "{message}");
        assert!(message.contains("OK"), "{message}");
    }

    /// A rate limit is retried on the same family, and it is never repeated
    /// over the other family.
    #[tokio::test]
    async fn a_rate_limit_is_retried_on_the_same_family() {
        let (ipv4, ipv6) = stubs(Some(rate_limit()), Some(document())).await;
        let transport = Transport::new(clients());

        let downloaded = transport.download("google", &[url(ipv4.port)]).await;

        let failure = downloaded.failure.expect("the refresh must fail");
        assert!(
            matches!(failure, RefreshFailure::Unreachable(_)),
            "{failure:?}"
        );
        assert_eq!(ipv4.requests(), 3, "the rate limit was retried twice");
        assert_eq!(ipv6.requests(), 0, "the other family was asked");
    }

    /// A refusal whose body cannot be read stays a refusal of the
    /// environment.
    #[tokio::test]
    async fn a_refusal_whose_body_cannot_be_read_stays_a_refusal() {
        let (ipv4, _ipv6) = stubs(Some(truncated()), Some(document())).await;
        let transport = Transport::new(clients());

        let downloaded = transport.download("google", &[url(ipv4.port)]).await;

        let failure = downloaded.failure.expect("the refresh must fail");
        assert!(
            matches!(failure, RefreshFailure::Unreachable(_)),
            "{failure:?}"
        );
        let message = failure.to_string();
        assert!(message.contains(&url(ipv4.port)), "{message}");
        assert!(message.contains("403"), "{message}");
    }

    /// The excerpt of a long body is cut and marked.
    #[test]
    fn the_excerpt_of_a_long_body_is_cut() {
        let body = vec![b'a'; 200];
        let text = excerpt(&body);
        assert!(text.ends_with('…'), "{text}");
        assert_eq!(text.trim_end_matches('…').len(), EXCERPT_LIMIT);
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

    /// The message of a refusal names the status of the endpoint and the
    /// answer of its body.
    #[test]
    fn the_message_of_a_refusal_names_the_answer_of_the_endpoint() {
        let refusal = Refusal::status(
            "https://vendor/list.json",
            StatusCode::FORBIDDEN,
            "text/xml".to_string(),
            b"OK",
            None,
        );
        assert_eq!(
            refusal.message,
            "https://vendor/list.json returned 403 Forbidden (text/xml): OK"
        );
    }

    /// A refusal without a body names the status alone.
    #[test]
    fn the_message_of_a_refusal_without_a_body_names_the_status() {
        let refusal = Refusal::status(
            "https://vendor/list.json",
            StatusCode::FORBIDDEN,
            "text/xml".to_string(),
            b"",
            None,
        );
        assert_eq!(
            refusal.message,
            "https://vendor/list.json returned 403 Forbidden"
        );
    }

    /// The message of a document the parser does not read names the content
    /// type and the beginning of the body.
    #[test]
    fn the_message_of_a_body_that_is_not_the_list_names_the_answer() {
        let refusal = Refusal::document(
            "https://vendor/list.json",
            "text/html".to_string(),
            b"<html><body>Service unavailable</body></html>",
            None,
        );
        assert_eq!(
            refusal.message,
            "https://vendor/list.json answered 200 with text/html: <html><body>Service unavailable</body></html>"
        );
    }

    /// The host of a URL is the host without its scheme.
    #[test]
    fn the_host_of_a_url_is_known() {
        assert_eq!(
            host_of("https://www.bing.com/toolbox/bingbot.json"),
            "www.bing.com"
        );
        assert_eq!(host_of("http://vendor.test:8080/ips.json"), "vendor.test");
    }

    /// The URL of a log carries neither a query nor a fragment.
    #[test]
    fn the_url_of_a_log_carries_no_query_and_no_fragment() {
        assert_eq!(
            sanitized_url("https://vendor/list.json?token=secret#part"),
            "https://vendor/list.json"
        );
    }
}
