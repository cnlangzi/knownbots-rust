//! The cache of the reverse DNS lookups that succeeded.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::RwLock;

use crate::error::ConfigError;

/// The cache of the addresses whose reverse DNS is known.
///
/// The cache is written behind an `RwLock`. Reads are the common case; writes
/// (a handful of new addresses per day) clone and replace.
///
/// The persistence to disk is **dirty-flagged**: [`RdnsCache::set`] only updates
/// the memory, [`RdnsCache::persist_if_dirty`] (called from the background
/// refresh) writes the file. That avoids a full rewrite on every cold lookup
/// under a crawl burst.
pub struct RdnsCache {
    path: PathBuf,
    map: RwLock<Arc<HashMap<String, String>>>,
    dirty: AtomicBool,
}

impl RdnsCache {
    /// Opens the cache that lives in `path`.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| ConfigError::Io(error.to_string()))?;
        }
        let map = load_file(&path)?;
        Ok(Self {
            path,
            map: RwLock::new(Arc::new(map)),
            dirty: AtomicBool::new(false),
        })
    }

    /// The hostname of the address, when the cache knows it.
    pub fn get(&self, ip: &str) -> Option<String> {
        self.map.read().get(ip).cloned()
    }

    /// Remembers the hostname of the address.
    pub fn set(&self, ip: &str, hostname: &str) {
        let mut guard = self.map.write();
        if guard.contains_key(ip) {
            return;
        }
        let mut next = (**guard).clone();
        next.insert(ip.to_string(), hostname.to_string());
        *guard = Arc::new(next);
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// Drops the entries whose hostname no longer matches `domains`.
    pub fn prune(&self, domains: &[String]) {
        let mut guard = self.map.write();
        let before = guard.len();
        let mut next = HashMap::new();
        for (ip, host) in guard.iter() {
            if match_domain(host, domains) {
                next.insert(ip.clone(), host.clone());
            }
        }
        let changed = next.len() != before;
        *guard = Arc::new(next);
        if changed {
            self.dirty.store(true, Ordering::Relaxed);
        }
    }

    /// Writes the cache to disk if [`RdnsCache::set`] or
    /// [`RdnsCache::prune`] marked it dirty since the last successful write.
    ///
    /// The dirty flag is cleared **before** the snapshot is taken, under the
    /// same write lock as the map. A `set` that lands after the snapshot
    /// re-dirties the cache, so the next flush picks it up. Clearing the flag
    /// *after* the write would drop that concurrent `set` (the new entry is in
    /// memory, the flag is then forced false, and a crash before the next
    /// `set` loses it). A failed write puts the flag back.
    pub fn persist_if_dirty(&self) -> Result<(), ConfigError> {
        let snapshot = {
            let guard = self.map.write();
            if !self.dirty.swap(false, Ordering::AcqRel) {
                return Ok(());
            }
            Arc::clone(&guard)
        };
        if let Err(error) = write_map(&self.path, &snapshot) {
            self.dirty.store(true, Ordering::Release);
            return Err(error);
        }
        Ok(())
    }

    /// Writes the cache to disk.
    pub fn persist(&self) -> Result<(), ConfigError> {
        let map = Arc::clone(&self.map.read());
        write_map(&self.path, &map)
    }

    /// The number of addresses the cache holds.
    pub fn size(&self) -> usize {
        self.map.read().len()
    }
}

fn write_map(path: &Path, map: &HashMap<String, String>) -> Result<(), ConfigError> {
    let mut body = String::new();
    for (ip, host) in map.iter() {
        body.push_str(ip);
        body.push(' ');
        body.push_str(host);
        body.push('\n');
    }
    std::fs::write(path, body)
        .map_err(|error| ConfigError::Io(format!("persist rdns cache {}: {error}", path.display())))
}

fn load_file(path: &Path) -> Result<HashMap<String, String>, ConfigError> {
    let mut map = HashMap::new();
    let Ok(data) = std::fs::read_to_string(path) else {
        return Ok(map);
    };
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((ip, host)) = line.split_once(' ') {
            let ip = ip.trim();
            let host = host.trim().trim_end_matches('.');
            if !ip.is_empty() && !host.is_empty() {
                map.insert(ip.to_string(), host.to_string());
            }
        }
    }
    Ok(map)
}

/// Whether the hostname is the domain or a subdomain of it.
///
/// The call sits on the request path, so it allocates nothing: the hostname and
/// the domain are compared byte by byte.
pub fn match_domain(hostname: &str, domains: &[String]) -> bool {
    let host = hostname.trim_end_matches('.').as_bytes();
    for domain in domains {
        let domain = domain.trim_end_matches('.').as_bytes();
        if host.eq_ignore_ascii_case(domain) {
            return true;
        }
        // The hostname has to end with the domain, and the character in front of
        // it has to be a dot.
        if host.len() > domain.len()
            && host[host.len() - domain.len()..].eq_ignore_ascii_case(domain)
            && host[host.len() - domain.len() - 1] == b'.'
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_domain_suffix() {
        assert!(match_domain(
            "crawl-66-249-66-1.googlebot.com",
            &["googlebot.com".into()]
        ));
        assert!(!match_domain(
            "evil-googlebot.com",
            &["googlebot.com".into()]
        ));
        // The suffix has to sit on a dot.
        assert!(!match_domain("notgooglebot.com", &["googlebot.com".into()]));
    }

    #[test]
    fn match_domain_ignores_the_case() {
        assert!(match_domain(
            "crawl.googlebot.com",
            &["GoogleBot.com".into()]
        ));
        assert!(match_domain("GOOGLEBOT.COM", &["googlebot.com".into()]));
    }

    #[test]
    fn persist_if_dirty_skips_clean() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rdns.txt");
        let cache = RdnsCache::open(&path).unwrap();
        // No set → nothing to write; the file stays absent.
        cache.persist_if_dirty().unwrap();
        assert!(!path.exists());
        cache.set("1.2.3.4", "a.example.com");
        cache.persist_if_dirty().unwrap();
        assert!(path.exists());
        // The second call is a no-op (the dirty flag is cleared).
        std::fs::remove_file(&path).unwrap();
        cache.persist_if_dirty().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn persist_failure_keeps_dirty_for_retry() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rdns.txt");
        let cache = RdnsCache::open(&path).unwrap();
        cache.set("1.2.3.4", "a.example.com");

        use std::os::unix::fs::PermissionsExt;
        let original = std::fs::metadata(directory.path())
            .unwrap()
            .permissions()
            .mode();
        let mut permissions = std::fs::metadata(directory.path()).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(directory.path(), permissions).unwrap();

        let error = cache.persist_if_dirty().unwrap_err();
        assert!(error.to_string().contains("rdns"));

        let mut permissions = std::fs::metadata(directory.path()).unwrap().permissions();
        permissions.set_mode(original);
        std::fs::set_permissions(directory.path(), permissions).unwrap();

        cache.persist_if_dirty().unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("1.2.3.4 a.example.com"));
    }
}
