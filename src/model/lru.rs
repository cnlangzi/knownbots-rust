//! The bounded cache of the addresses whose reverse lookup failed.

use std::num::NonZeroUsize;

use parking_lot::Mutex;

/// The addresses whose reverse lookup already failed.
///
/// Once an address failed the lookup of a bot, the next claim of that bot does
/// not look it up again until the entry is evicted.
pub struct FailLru {
    inner: Mutex<lru::LruCache<String, ()>>,
}

impl FailLru {
    /// Builds a cache that holds at most `limit` addresses.
    pub fn new(limit: usize) -> Self {
        let limit = NonZeroUsize::new(limit.max(1)).expect("the limit is not zero");
        Self {
            inner: Mutex::new(lru::LruCache::new(limit)),
        }
    }

    /// Whether the address is in the cache.
    pub fn contains(&self, ip: &str) -> bool {
        self.inner.lock().contains(ip)
    }

    /// Remembers the address.
    pub fn insert(&self, ip: &str) {
        self.inner.lock().put(ip.to_string(), ());
    }
}
