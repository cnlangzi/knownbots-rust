//! The HTTP clients of the crate and the classification of its failures.

use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use crate::error::{ConfigError, RefreshFailure};

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

/// The address family a client is bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AddressFamily {
    /// IPv4.
    Ipv4,
    /// IPv6.
    Ipv6,
}

impl AddressFamily {
    /// The family that is not this one.
    pub fn other(self) -> AddressFamily {
        match self {
            AddressFamily::Ipv4 => AddressFamily::Ipv6,
            AddressFamily::Ipv6 => AddressFamily::Ipv4,
        }
    }

    /// The family an address belongs to.
    pub fn of(address: IpAddr) -> AddressFamily {
        match address {
            IpAddr::V4(_) => AddressFamily::Ipv4,
            IpAddr::V6(_) => AddressFamily::Ipv6,
        }
    }
}

/// The clients the crate downloads the lists with.
///
/// The set holds a client that is bound to no family and a client for each
/// family. A client that is bound to a family reaches no address of the other
/// one, so the crate can repeat a request over the family that is left.
#[derive(Clone, Debug)]
pub struct ClientSet {
    /// The client that is bound to no family.
    normal: reqwest::Client,
    /// The client that reaches the IPv4 addresses only.
    ipv4: reqwest::Client,
    /// The client that reaches the IPv6 addresses only.
    ipv6: reqwest::Client,
}

impl ClientSet {
    /// Builds a set of clients.
    ///
    /// The client that is bound to no family reaches every address of a host;
    /// the two others reach the addresses of their family only.
    pub fn new(normal: reqwest::Client, ipv4: reqwest::Client, ipv6: reqwest::Client) -> Self {
        Self { normal, ipv4, ipv6 }
    }

    /// The client that is bound to no family.
    pub fn normal(&self) -> &reqwest::Client {
        &self.normal
    }

    /// The client that is bound to the family.
    pub fn pinned(&self, family: AddressFamily) -> &reqwest::Client {
        match family {
            AddressFamily::Ipv4 => &self.ipv4,
            AddressFamily::Ipv6 => &self.ipv6,
        }
    }
}

/// Builds the clients the crate downloads the lists with.
///
/// The clients use rustls, so the crate builds without a system TLS stack, and
/// they verify the certificate of the endpoint they talk to.
///
/// The two pinned clients are bound to the local address of their family, so
/// that they reach no address of the other one. The unbound client of the
/// crate is bound to no family: the stack splits the addresses of a host into
/// two groups by the family of the first one and connects the first group
/// first — the second group starts 300 ms later, and the two groups are raced
/// against each other.
pub fn build_clients() -> Result<ClientSet, ConfigError> {
    Ok(ClientSet::new(
        build_client(None)?,
        build_client(Some(IpAddr::V4(Ipv4Addr::UNSPECIFIED)))?,
        build_client(Some(IpAddr::V6(Ipv6Addr::UNSPECIFIED)))?,
    ))
}

/// Builds one client of the set.
///
/// A client that is bound to a local address reaches the addresses of that
/// family only.
fn build_client(local: Option<IpAddr>) -> Result<reqwest::Client, ConfigError> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .user_agent(user_agent());
    if let Some(local) = local {
        builder = builder.local_address(local);
    }
    builder
        .build()
        .map_err(|error| ConfigError::Http(error.to_string()))
}

/// Classifies a transport failure.
///
/// A connect error or a timeout is the environment: the host is down, or the
/// runner has no route to it. Every other failure — a handshake failure above
/// all — points at the client of this crate.
///
/// The URL of the message is the one of the caller: it carries neither a query
/// nor a fragment.
pub fn map_transport_error(error: &reqwest::Error, url: &str) -> RefreshFailure {
    let message = format!("{url}: {}", describe(error));
    if error.is_connect() || error.is_timeout() {
        RefreshFailure::Unreachable(message)
    } else {
        RefreshFailure::Broken(message)
    }
}

/// The description of a transport failure.
///
/// The description of the error of `reqwest` carries the URL of the request —
/// the query and the fragment included — so the crate builds its own: the
/// kind of the failure and the chain of its causes.
fn describe(error: &reqwest::Error) -> String {
    let mut description = kind(error).to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        description.push_str(": ");
        description.push_str(&cause.to_string());
        source = cause.source();
    }
    description
}

/// The kind of a transport failure.
///
/// The names are the ones of `reqwest`, without the URL that its own
/// description appends.
fn kind(error: &reqwest::Error) -> &'static str {
    if error.is_builder() {
        "builder error"
    } else if error.is_request() {
        "error sending request"
    } else if error.is_body() {
        "request or response body error"
    } else if error.is_decode() {
        "error decoding response body"
    } else if error.is_redirect() {
        "error following redirect"
    } else if error.is_upgrade() {
        "error upgrading connection"
    } else if error.is_status() {
        "HTTP status error"
    } else if error.is_timeout() {
        "the request timed out"
    } else if error.is_connect() {
        "error connecting to the endpoint"
    } else {
        "error of the request"
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
    use std::io::{Read, Write};
    use std::net::SocketAddr;

    /// A server that answers with a canned response, on the loopback.
    ///
    /// The answer tells the client to close the connection, so that the client
    /// never reuses a connection the server closed.
    fn server(response: &'static [u8]) -> SocketAddr {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("the listener of the test");
        let address = listener.local_addr().expect("the address of the listener");
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(response);
                let _ = stream.flush();
            }
        });
        address
    }

    /// A client that is bound to a family reaches no address of the other one.
    #[tokio::test]
    async fn a_pinned_client_reaches_no_address_of_the_other_family() {
        let address = server(b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
        let url = format!("http://{address}/");

        let clients = build_clients().expect("the clients of the crate");
        let response = clients
            .pinned(AddressFamily::Ipv4)
            .get(&url)
            .send()
            .await
            .expect("the IPv4 client reaches the server");
        assert_eq!(response.status(), reqwest::StatusCode::OK);

        let error = clients
            .pinned(AddressFamily::Ipv6)
            .get(&url)
            .send()
            .await
            .expect_err("the IPv6 client reaches no IPv4 address");
        assert!(error.is_connect(), "{error}");
    }

    /// The message of a transport failure names the URL of the crate and not
    /// the URL of the request, which carries the query and the fragment.
    #[tokio::test]
    async fn the_message_of_a_transport_failure_carries_no_url_of_the_request() {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("the listener of the test");
        let port = listener
            .local_addr()
            .expect("the address of the listener")
            .port();
        drop(listener);

        let error = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("the client")
            .get(format!(
                "http://127.0.0.1:{port}/ips.json?token=secret#part"
            ))
            .send()
            .await
            .expect_err("the port answers nothing");

        let failure = map_transport_error(&error, &format!("http://127.0.0.1:{port}/ips.json"));
        let message = failure.to_string();
        assert!(message.contains("ips.json"), "{message}");
        assert!(!message.contains("token=secret"), "{message}");
        assert!(!message.contains("#part"), "{message}");
    }

    /// The family of an address is the family it belongs to.
    #[test]
    fn the_family_of_an_address_is_known() {
        assert_eq!(
            AddressFamily::of("1.2.3.4".parse::<IpAddr>().unwrap()),
            AddressFamily::Ipv4
        );
        assert_eq!(
            AddressFamily::of("2001:db8::1".parse::<IpAddr>().unwrap()),
            AddressFamily::Ipv6
        );
    }

    /// The family that is not this one is the other one.
    #[test]
    fn the_other_family_is_the_other_one() {
        assert_eq!(AddressFamily::Ipv4.other(), AddressFamily::Ipv6);
        assert_eq!(AddressFamily::Ipv6.other(), AddressFamily::Ipv4);
    }
}
