//! The clients that reach the loopback of both address families.
//!
//! A download that the endpoint refused is repeated over the other address
//! family, and the repetition has to reach the endpoint. The resolver of the
//! system answers with the IPv4 address of the loopback alone, so the tests
//! that exercise the repetition bring their own clients: the resolver of the
//! test answers with the addresses of both families.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use knownbots::{ClientSet, Validator, ValidatorOptions, reqwest};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The host the clients of the tests reach.
pub const HOST: &str = "vendor.test";

/// The list the vendor of the tests publishes.
pub const LIST: &str = r#"{"prefixes":[{"ipv4Prefix":"1.2.3.0/24"}]}"#;

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

/// The clients of the tests: they reach the loopback of their family.
///
/// The resolver answers with the IPv4 address first, so the client that is
/// bound to no family prefers that family.
pub fn clients() -> ClientSet {
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

/// A server that answers over both families of the loopback.
///
/// The mock server binds to the IPv4 loopback alone, so the test binds the
/// listener itself: a listener on `[::]` takes the IPv4 connections as well.
pub async fn server() -> MockServer {
    let listener = std::net::TcpListener::bind("[::]:0").expect("the listener of the test");
    MockServer::builder().listener(listener).start().await
}

/// The URL of a document of the host.
pub fn url(port: u16) -> String {
    format!("http://{HOST}:{port}/ips.json")
}

/// A server that refuses the next download, and the validator that downloads
/// from it.
///
/// The first download delivers the list, and the family that delivered it is
/// remembered for the host. The server refuses the next request once and lists
/// again afterwards, so that the repetition over the other address family
/// repairs the refusal.
///
/// The URL of the document is built from the port of the server.
pub async fn refusing_server(
    url: impl Fn(u16) -> String,
) -> (MockServer, Arc<Validator>, tempfile::TempDir) {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_string(LIST),
        )
        .mount(&server)
        .await;

    let directory = tempfile::tempdir().expect("a temporary directory");
    let conf = directory.path().join("conf.d");
    std::fs::create_dir_all(&conf).expect("the override directory");
    std::fs::write(
        conf.join("testbot.yaml"),
        format!(
            "name: testbot\nua: \"TestBot\"\nvendor: \"Example\"\nparser: google\nurls:\n  - \"{}\"\n",
            url(server.address().port())
        ),
    )
    .expect("the definition");

    let validator = Validator::new(ValidatorOptions {
        root: directory.path().to_path_buf(),
        client_set: Some(clients()),
        ..ValidatorOptions::default()
    })
    .expect("a validator");

    // The first download delivers the list, and the family that delivered it
    // is remembered for the host.
    assert!(
        validator.refresh_bot("testbot").await.is_none(),
        "the first download must deliver the list"
    );

    // The server refuses the next request once, and lists again afterwards.
    server.reset().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(ResponseTemplate::new(203).set_body_raw("OK", "text/xml"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_string(LIST),
        )
        .mount(&server)
        .await;

    (server, validator, directory)
}
