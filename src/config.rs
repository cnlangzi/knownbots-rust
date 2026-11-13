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
}

impl Default for ValidatorOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from(DEFAULT_ROOT),
            fail_limit: DEFAULT_FAIL_LIMIT,
            refresh_interval: DEFAULT_REFRESH_INTERVAL,
        }
    }
}

/// The configuration a validator runs with.
pub(crate) struct Config {
    pub(crate) root: PathBuf,
    pub(crate) fail_limit: usize,
    pub(crate) refresh_interval: Duration,
}

impl From<ValidatorOptions> for Config {
    fn from(options: ValidatorOptions) -> Self {
        Self {
            root: options.root,
            fail_limit: options.fail_limit,
            refresh_interval: options.refresh_interval,
        }
    }
}
