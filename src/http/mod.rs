//! The HTTP client of the crate and the classification of its failures.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use crate::error::{ConfigError, RefreshFailure};

/// The HTTP client the crate downloads the lists with.
///
/// The crate re-exports the type, so that a host that supplies a client of its
/// own does not have to depend on `reqwest` itself — and cannot end up with a
/// second copy of it.
pub use reqwest::Client as HttpClient;

/// The HTTP stack of the crate.
///
/// The crate re-exports it, so that a host that builds a client of its own
/// needs no dependency of its own.
pub use reqwest;

/// The time the client waits for the connection to a vendor.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The time the client waits for a whole request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The user agent the crate sends upstream.
pub fn user_agent() -> String {
    format!("knownbots/{}", env!("CARGO_PKG_VERSION"))
}

/// Builds the HTTP client the crate downloads the lists with.
///
/// The client uses rustls, so the crate builds without a system TLS stack, and
/// it verifies the certificate of the endpoint it talks to.
///
/// The client resolves a host through the resolver of the crate, which puts the
/// IPv4 addresses of the host first. The stack races the addresses of a host
/// and prefers the family of the first one, so the order of the addresses
/// decides which path to the vendor is taken — a path that is tampered with
/// answers with a document that is not the list of the vendor.
pub fn build_client() -> Result<reqwest::Client, ConfigError> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .user_agent(user_agent())
        .dns_resolver(Arc::new(PreferIpv4))
        .build()
        .map_err(|error| ConfigError::Http(error.to_string()))
}

/// The resolver of the crate: the IPv4 addresses of a host come first.
///
/// The lookup runs in the background, so that the request path never waits for
/// the resolver of the system.
struct PreferIpv4;

impl reqwest::dns::Resolve for PreferIpv4 {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let resolved = tokio::task::spawn_blocking(move || dns_lookup::lookup_host(&host))
                .await
                .map_err(|error| std::io::Error::other(error.to_string()))
                .and_then(|resolved| resolved)
                .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })?;
            let addrs: reqwest::dns::Addrs = Box::new(ipv4_first(&resolved).into_iter());
            Ok(addrs)
        })
    }
}

/// The addresses of a host, the IPv4 ones first.
///
/// The order within a family is the order the resolver of the system returned.
fn ipv4_first(addrs: &[IpAddr]) -> Vec<SocketAddr> {
    let mut ordered: Vec<IpAddr> = Vec::with_capacity(addrs.len());
    ordered.extend(addrs.iter().filter(|addr| addr.is_ipv4()));
    ordered.extend(addrs.iter().filter(|addr| addr.is_ipv6()));
    ordered
        .into_iter()
        .map(|addr| SocketAddr::new(addr, 0))
        .collect()
}

/// Classifies a transport failure.
///
/// A connect error or a timeout is the environment: the host is down, or the
/// runner has no route to it. Every other failure — a handshake failure above
/// all — points at the client of this crate.
pub fn map_transport_error(error: &reqwest::Error, url: &str) -> RefreshFailure {
    if error.is_connect() || error.is_timeout() {
        RefreshFailure::Unreachable(format!("{url}: {error}"))
    } else {
        RefreshFailure::Broken(format!("{url}: {error}"))
    }
}

/// Reads the `Retry-After` header of a refusal.
pub fn parse_retry_after(value: Option<&str>) -> Option<Duration> {
    let seconds = value?.trim().parse::<u64>().ok()?;
    Some(Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host that answers with both families is tried with IPv4 first.
    #[test]
    fn the_ipv4_addresses_of_a_mixed_host_come_first() {
        let addrs = vec![
            "2001:db8::1".parse::<IpAddr>().unwrap(),
            "1.2.3.4".parse::<IpAddr>().unwrap(),
            "5.6.7.8".parse::<IpAddr>().unwrap(),
            "2001:db8::2".parse::<IpAddr>().unwrap(),
        ];
        let ordered = ipv4_first(&addrs);
        assert_eq!(
            ordered,
            vec![
                SocketAddr::new("1.2.3.4".parse::<IpAddr>().unwrap(), 0),
                SocketAddr::new("5.6.7.8".parse::<IpAddr>().unwrap(), 0),
                SocketAddr::new("2001:db8::1".parse::<IpAddr>().unwrap(), 0),
                SocketAddr::new("2001:db8::2".parse::<IpAddr>().unwrap(), 0),
            ]
        );
    }

    /// A host that answers with IPv4 only keeps its addresses.
    #[test]
    fn the_addresses_of_an_ipv4_host_stay() {
        let addrs = vec![
            "1.2.3.4".parse::<IpAddr>().unwrap(),
            "5.6.7.8".parse::<IpAddr>().unwrap(),
        ];
        assert_eq!(
            ipv4_first(&addrs),
            vec![
                SocketAddr::new("1.2.3.4".parse::<IpAddr>().unwrap(), 0),
                SocketAddr::new("5.6.7.8".parse::<IpAddr>().unwrap(), 0),
            ]
        );
    }

    /// A host that answers with IPv6 only keeps its addresses.
    #[test]
    fn the_addresses_of_an_ipv6_host_stay() {
        let addrs = vec![
            "2001:db8::1".parse::<IpAddr>().unwrap(),
            "2001:db8::2".parse::<IpAddr>().unwrap(),
        ];
        assert!(ipv4_first(&addrs).iter().all(|addr| addr.is_ipv6()));
    }

    /// The resolver of the crate answers with the addresses of the host, the
    /// IPv4 ones first.
    ///
    /// The lookup of the host is a lookup of the loopback — the test reaches no
    /// network.
    #[tokio::test]
    async fn the_resolver_answers_with_the_ipv4_addresses_first() {
        use reqwest::dns::Resolve;
        use std::str::FromStr;

        let name = reqwest::dns::Name::from_str("localhost").expect("the name of a host");
        let addrs = PreferIpv4
            .resolve(name)
            .await
            .expect("the addresses of the host")
            .collect::<Vec<SocketAddr>>();
        assert!(!addrs.is_empty(), "the host must have addresses");

        let first_ipv6 = addrs.iter().position(|addr| addr.is_ipv6());
        let last_ipv4 = addrs.iter().rposition(|addr| addr.is_ipv4());
        if let (Some(first_ipv6), Some(last_ipv4)) = (first_ipv6, last_ipv4) {
            assert!(
                last_ipv4 < first_ipv6,
                "the IPv4 addresses come first: {addrs:?}"
            );
        }
    }
}
