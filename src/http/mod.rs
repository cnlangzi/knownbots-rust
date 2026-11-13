//! The HTTP client of the crate and the classification of its failures.

use std::time::Duration;

use crate::error::{ConfigError, RefreshFailure};

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
pub fn build_client() -> Result<reqwest::Client, ConfigError> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .user_agent(user_agent())
        .build()
        .map_err(|error| ConfigError::Http(error.to_string()))
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
