//! The download of the published lists.
//!
//! The endpoints are mocked, so the tests never reach the internet.

#![cfg(feature = "full")]

use std::io::Write;
use std::net::IpAddr;
use std::sync::Arc;

use knownbots::{BotRefreshState, RefreshFailure, Validator, ValidatorOptions};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Compresses a body the way a vendor that serves a gzip encoded list does.
fn gzip(body: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(body).expect("the body must be written");
    encoder.finish().expect("the body must be finished")
}

/// A validator whose registry holds one crawler that reads its list from the
/// mocked endpoint.
async fn validator_for(server: &MockServer, parser: &str) -> (tempfile::TempDir, Arc<Validator>) {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let conf = directory.path().join("conf.d");
    std::fs::create_dir_all(&conf).expect("the override directory");
    std::fs::write(
        conf.join("testbot.yaml"),
        format!(
            "name: testbot\nua: \"TestBot\"\nvendor: \"Example\"\nparser: {parser}\nurls:\n  - \"{}/ips.json\"\n",
            server.uri()
        ),
    )
    .expect("the definition");
    let validator = Validator::new(ValidatorOptions {
        root: directory.path().to_path_buf(),
        ..ValidatorOptions::default()
    })
    .expect("a validator");
    (directory, validator)
}

/// The number of the prefixes the crawler holds.
fn prefix_count(validator: &Validator) -> usize {
    validator
        .refresh_status()
        .into_iter()
        .find(|status| status.name == "testbot")
        .expect("the crawler in the registry")
        .prefixes
}

/// The state of the address space of the crawler.
fn state(validator: &Validator) -> BotRefreshState {
    validator
        .refresh_status()
        .into_iter()
        .find(|status| status.name == "testbot")
        .expect("the crawler in the registry")
        .state
}

/// A compressed list is decoded before it is read.
#[tokio::test]
async fn a_gzipped_list_is_decoded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Encoding", "gzip")
                .set_body_bytes(gzip(br#"{"prefixes":[{"ipv4Prefix":"1.2.3.0/24"}]}"#)),
        )
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "google").await;
    assert!(validator.refresh_bot("testbot").await.is_none());
    assert_eq!(prefix_count(&validator), 1);

    let result = validator.verify("TestBot/1.0", "1.2.3.4".parse::<IpAddr>().unwrap());
    assert!(result.is_verified(), "{result:?}");
}

/// A body that is not the list of the vendor is the fault of the environment.
#[tokio::test]
async fn an_error_page_is_unreachable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "text/html")
                .set_body_string("<html><body>Service unavailable</body></html>"),
        )
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "google").await;
    let failure = validator
        .refresh_bot("testbot")
        .await
        .expect("the refresh must fail");
    assert!(
        matches!(failure, RefreshFailure::Unreachable(_)),
        "{failure:?}"
    );
}

/// An empty body is not the list of the vendor either.
#[tokio::test]
async fn an_empty_body_is_unreachable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(Vec::new()))
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "google").await;
    let failure = validator
        .refresh_bot("testbot")
        .await
        .expect("the refresh must fail");
    assert!(
        matches!(failure, RefreshFailure::Unreachable(_)),
        "{failure:?}"
    );
}

/// A document that lost its list changed its shape — the crate has to follow.
#[tokio::test]
async fn a_document_without_its_list_is_broken() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_string(r#"{"creationTime":"2026-01-01T00:00:00Z"}"#),
        )
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "google").await;
    let failure = validator
        .refresh_bot("testbot")
        .await
        .expect("the refresh must fail");
    assert!(matches!(failure, RefreshFailure::Broken(_)), "{failure:?}");
}

/// One entry that holds no usable prefix does not discard the document.
#[tokio::test]
async fn a_broken_entry_does_not_discard_the_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_string(
                    r#"{"prefixes":[{"ipv4Prefix":"1.2.3.0/24"},{"ipv4Prefix":"not-a-cidr"},{"ipv6Prefix":"2001:db8::/32"}]}"#,
                ),
        )
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "google").await;
    assert!(validator.refresh_bot("testbot").await.is_none());
    assert_eq!(prefix_count(&validator), 2);

    let result = validator.verify("TestBot/1.0", "1.2.3.4".parse::<IpAddr>().unwrap());
    assert!(result.is_verified(), "{result:?}");
}

/// A failed refresh keeps the prefixes the crawler had.
#[tokio::test]
async fn a_failed_refresh_keeps_the_prefixes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_string(r#"{"prefixes":[{"ipv4Prefix":"1.2.3.0/24"}]}"#),
        )
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "google").await;
    assert!(validator.refresh_bot("testbot").await.is_none());
    assert_eq!(prefix_count(&validator), 1);

    // The endpoint answers with something else now.
    server.reset().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "text/html")
                .set_body_string("<html><body>Service unavailable</body></html>"),
        )
        .mount(&server)
        .await;

    let failure = validator
        .refresh_bot("testbot")
        .await
        .expect("the refresh must fail");
    assert!(
        matches!(failure, RefreshFailure::Unreachable(_)),
        "{failure:?}"
    );
    // The prefixes are still there, and the crawler is reported as failed.
    assert_eq!(prefix_count(&validator), 1);
    assert_eq!(state(&validator), BotRefreshState::Failed);

    let result = validator.verify("TestBot/1.0", "1.2.3.4".parse::<IpAddr>().unwrap());
    assert!(result.is_verified(), "{result:?}");
}

/// A refusal of the vendor is the fault of the environment.
#[tokio::test]
async fn a_refusal_is_unreachable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "google").await;
    let failure = validator
        .refresh_bot("testbot")
        .await
        .expect("the refresh must fail");
    assert!(
        matches!(failure, RefreshFailure::Unreachable(_)),
        "{failure:?}"
    );
}

/// A rate limit is transient — it is retried, and it stays a failure of the
/// environment.
#[tokio::test]
async fn a_rate_limit_is_retried_and_unreachable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "google").await;
    let failure = validator
        .refresh_bot("testbot")
        .await
        .expect("the refresh must fail");
    assert!(
        matches!(failure, RefreshFailure::Unreachable(_)),
        "{failure:?}"
    );
}

/// A redirect is followed, and the list is read from the last answer.
#[tokio::test]
async fn a_redirect_is_followed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(301)
                .insert_header("Location", format!("{}/moved.json", server.uri())),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/moved.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_string(r#"{"prefixes":[{"ipv4Prefix":"5.6.7.0/24"}]}"#),
        )
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "google").await;
    assert!(validator.refresh_bot("testbot").await.is_none());
    assert_eq!(prefix_count(&validator), 1);
}

/// A list that lists its addresses line by line is read as well.
#[tokio::test]
async fn a_list_of_lines_is_read() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "text/plain")
                .set_body_string("9.9.9.0/24\n# a comment\n10.10.10.10\n"),
        )
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "txt").await;
    assert!(validator.refresh_bot("testbot").await.is_none());
    assert_eq!(prefix_count(&validator), 2);
}

/// A page that holds no address is not the list of the vendor.
#[tokio::test]
async fn an_error_page_of_a_text_list_is_unreachable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "text/html")
                .set_body_string("<html><body>Service unavailable</body></html>"),
        )
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "txt").await;
    let failure = validator
        .refresh_bot("testbot")
        .await
        .expect("the refresh must fail");
    assert!(
        matches!(failure, RefreshFailure::Unreachable(_)),
        "{failure:?}"
    );
}

/// A list that holds no usable prefix is a document the crate mis-reads.
#[tokio::test]
async fn a_list_without_a_prefix_is_broken() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_string(r#"{"prefixes":[]}"#),
        )
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server, "google").await;
    let failure = validator
        .refresh_bot("testbot")
        .await
        .expect("the refresh must fail");
    assert!(matches!(failure, RefreshFailure::Broken(_)), "{failure:?}");
}

/// The crawler that is backed by several documents keeps the prefixes of the
/// documents that answered.
#[tokio::test]
async fn the_prefixes_of_the_documents_that_answered_are_kept() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_string(r#"{"prefixes":[{"ipv4Prefix":"1.2.3.0/24"}]}"#),
        )
        .mount(&server)
        .await;

    let directory = tempfile::tempdir().expect("a temporary directory");
    let conf = directory.path().join("conf.d");
    std::fs::create_dir_all(&conf).expect("the override directory");
    std::fs::write(
        conf.join("testbot.yaml"),
        format!(
            "name: testbot\nua: \"TestBot\"\nvendor: \"Example\"\nparser: google\nurls:\n  - \"{}/ips.json\"\n  - \"{}/broken.json\"\n",
            server.uri(),
            server.uri()
        ),
    )
    .expect("the definition");
    Mock::given(method("GET"))
        .and(path("/broken.json"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let validator = Validator::new(ValidatorOptions {
        root: directory.path().to_path_buf(),
        ..ValidatorOptions::default()
    })
    .expect("a validator");
    let failure = validator
        .refresh_bot("testbot")
        .await
        .expect("the refresh must fail");
    assert!(
        matches!(failure, RefreshFailure::Unreachable(_)),
        "{failure:?}"
    );
    // The prefixes of the document that answered are kept.
    assert_eq!(prefix_count(&validator), 1);
}

/// The client of the host is the client the crate downloads the lists with.
#[tokio::test]
async fn the_client_of_the_host_is_used() {
    use knownbots::reqwest::header::HeaderMap;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .and(header("x-knownbots", "yes"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_string(r#"{"prefixes":[{"ipv4Prefix":"1.2.3.0/24"}]}"#),
        )
        .mount(&server)
        .await;

    let directory = tempfile::tempdir().expect("a temporary directory");
    let conf = directory.path().join("conf.d");
    std::fs::create_dir_all(&conf).expect("the override directory");
    std::fs::write(
        conf.join("testbot.yaml"),
        format!(
            "name: testbot\nua: \"TestBot\"\nvendor: \"Example\"\nparser: google\nurls:\n  - \"{}/ips.json\"\n",
            server.uri()
        ),
    )
    .expect("the definition");

    let mut headers = HeaderMap::new();
    headers.insert("x-knownbots", "yes".parse().expect("a header value"));
    let client = knownbots::HttpClient::builder()
        .default_headers(headers)
        .build()
        .expect("a client");

    let validator = Validator::new(ValidatorOptions {
        root: directory.path().to_path_buf(),
        http_client: Some(client),
        ..ValidatorOptions::default()
    })
    .expect("a validator");

    // The endpoint answers only when the header of the client of the host is
    // there, so a list means that the client of the host was used.
    assert!(validator.refresh_bot("testbot").await.is_none());
    assert_eq!(prefix_count(&validator), 1);
}
