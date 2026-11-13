//! The configuration of a validator.

use std::path::PathBuf;
use std::time::Duration;

/// The default number of failed reverse lookups a bot remembers.
pub const DEFAULT_FAIL_LIMIT: usize = 1000;

/// The default interval between two refreshes of the published lists.
pub const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// The default directory of the caches.
pub const DEFAULT_ROOT: &str = "./knownbots";

/// The options a validator is built with.
#[derive(Debug, Clone)]
pub struct ValidatorOptions {
    /// The directory that holds the cache of every bot and the definitions an
    /// operator dropped into `<root>/conf.d/`.
    pub root: PathBuf,
    /// How many failed reverse lookups a bot remembers.
    pub fail_limit: usize,
    /// How often the published lists are refreshed.
    pub refresh_interval: Duration,
    /// Whether the background work refreshes the lists as soon as it starts.
    ///
    /// The worker refreshes the lists right away, so that the crawlers that are
    /// backed by a document are usable within seconds of the start of the
    /// process. A host that refreshes the lists itself — and that wants to see
    /// the outcome before it serves traffic — turns this off; the worker then
    /// keeps them fresh from the first interval on.
    pub refresh_on_start: bool,
    /// The client the published lists are downloaded with.
    ///
    /// The crate builds a client of its own when the host supplies none. A host
    /// that reaches the vendors through a proxy, or that trusts its own root
    /// store, supplies a client of its own — built with
    /// [`HttpClient`](crate::HttpClient), so that it needs no dependency of
    /// its own.
    #[cfg(feature = "full")]
    pub http_client: Option<crate::HttpClient>,
}

impl Default for ValidatorOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from(DEFAULT_ROOT),
            fail_limit: DEFAULT_FAIL_LIMIT,
            refresh_interval: DEFAULT_REFRESH_INTERVAL,
            refresh_on_start: true,
            #[cfg(feature = "full")]
            http_client: None,
        }
    }
}

/// The configuration a validator runs with.
pub(crate) struct Config {
    pub(crate) root: PathBuf,
    pub(crate) fail_limit: usize,
    pub(crate) refresh_interval: Duration,
    pub(crate) refresh_on_start: bool,
    #[cfg(feature = "full")]
    pub(crate) http_client: Option<crate::HttpClient>,
}

impl From<ValidatorOptions> for Config {
    fn from(options: ValidatorOptions) -> Self {
        Self {
            root: options.root,
            fail_limit: options.fail_limit,
            refresh_interval: options.refresh_interval,
            refresh_on_start: options.refresh_on_start,
            #[cfg(feature = "full")]
            http_client: options.http_client,
        }
    }
}
