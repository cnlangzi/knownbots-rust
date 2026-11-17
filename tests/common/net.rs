//! The clients that reach the loopback of both address families.
//!
//! A download that the endpoint refused is repeated over the other address
//! family, and the repetition has to reach the endpoint. The resolver of the
//! system answers with the IPv4 address of the loopback alone, so the tests
//! that exercise the repetition bring their own clients: the resolver of the
//! test answers with the addresses of both families.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use knownbots::{ClientSet, reqwest};
use wiremock::MockServer;

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
