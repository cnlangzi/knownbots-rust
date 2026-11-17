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
    /// The clients the published lists are downloaded with.
    ///
    /// The crate builds a set of its own when the host supplies none. A host
    /// that reaches the vendors through a proxy, or that trusts its own root
    /// store, builds the three clients itself — with the `reqwest` the crate
    /// re-exports, so that it needs no dependency of its own — and hands them
    /// over as a set.
    ///
    /// The crate binds a client to an address family, so that a download that
    /// was refused can be repeated over the other family. A host that brings
    /// its own transport hands over the whole set, so that its own settings
    /// apply to every attempt.
    #[cfg(feature = "full")]
    pub client_set: Option<crate::ClientSet>,
}

impl Default for ValidatorOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from(DEFAULT_ROOT),
            fail_limit: DEFAULT_FAIL_LIMIT,
            refresh_interval: DEFAULT_REFRESH_INTERVAL,
            refresh_on_start: true,
            #[cfg(feature = "full")]
            client_set: None,
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
    pub(crate) client_set: Option<crate::ClientSet>,
}

impl From<ValidatorOptions> for Config {
    fn from(options: ValidatorOptions) -> Self {
        Self {
            root: options.root,
            fail_limit: options.fail_limit,
            refresh_interval: options.refresh_interval,
            refresh_on_start: options.refresh_on_start,
            #[cfg(feature = "full")]
            client_set: options.client_set,
        }
    }
}
