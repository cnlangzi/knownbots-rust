//! The decision that verifies a request.
//!
//! The decision is a total function of the state the validator keeps: the
//! registry, the prefixes of the matched bot and the reverse DNS cache of the
//! bot. It never blocks on the network — a reverse lookup that is not cached
//! yet is queued and the request fails closed.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::Mutex;

use crate::config::{Config, ValidatorOptions};
use crate::error::{ConfigError, RefreshFailure};
use crate::index::ua::find_bot_by_ua;
use crate::model::bot::{Bot, BotConfigError};
use crate::model::lru::FailLru;
use crate::model::rdns::{RdnsCache, match_domain};
use crate::model::refresh::{BotRefreshState, BotRefreshStatus};
use crate::model::registry::{count_cold_url_bots, load_bots_with_errors};
use crate::model::verify::{VerifyResult, VerifyStatus};

#[cfg(feature = "full")]
use std::collections::HashSet;
#[cfg(feature = "full")]
use std::sync::atomic::AtomicU64;

#[cfg(feature = "full")]
use std::thread::JoinHandle;

#[cfg(feature = "full")]
use tokio::sync::{mpsc, watch};

#[cfg(feature = "full")]
use crate::feed;
#[cfg(feature = "full")]
use crate::worker::RdnsJob;

/// The verifier of the requests.
///
/// The handle is cheap to clone and holds its state behind an [`Arc`].
pub struct Validator {
    pub(crate) root: PathBuf,
    pub(crate) bots: Vec<Bot>,
    pub(crate) shutdown: AtomicBool,
    /// Whether the background work was started.
    #[cfg(feature = "full")]
    pub(crate) workers_started: AtomicBool,
    /// Why the most recent refresh of a bot failed.
    ///
    /// A bot that is absent from the map refreshed fine. An entry says how it
    /// failed — [`RefreshFailure::Unreachable`] (the vendor refused us, the
    /// network was down) or [`RefreshFailure::Broken`] (the list did not
    /// parse, our own client failed). That distinction lets a host tell an
    /// environment problem from a regression.
    pub(crate) refresh_errors: Mutex<HashMap<String, RefreshFailure>>,
    /// The definitions of the operator that were skipped at load time.
    ///
    /// A broken override is an operator error, not a reason to refuse to
    /// start: the file is skipped, logged, and reported here.
    pub(crate) config_errors: Vec<BotConfigError>,
    /// The interval between two refreshes of the published lists.
    pub(crate) refresh_interval: Duration,
    /// Whether the worker refreshes the lists as soon as it starts.
    pub(crate) refresh_on_start: bool,
    #[cfg(feature = "full")]
    pub(crate) rdns_tx: mpsc::Sender<RdnsJob>,
    #[cfg(feature = "full")]
    pub(crate) rdns_rx: Mutex<Option<mpsc::Receiver<RdnsJob>>>,
    /// The reverse lookups that were dropped because the queue was full or the
    /// worker side had shut down.
    #[cfg(feature = "full")]
    pub(crate) rdns_overflow: AtomicU64,
    /// The reverse lookups that run, so that the same address is not looked up
    /// twice.
    #[cfg(feature = "full")]
    pub(crate) in_flight: Mutex<HashSet<(String, IpAddr)>>,
    #[cfg(feature = "full")]
    pub(crate) http: reqwest::Client,
    /// Tells the worker thread to stop.
    #[cfg(feature = "full")]
    pub(crate) shutdown_tx: watch::Sender<bool>,
    /// The end of the worker thread.
    #[cfg(feature = "full")]
    pub(crate) shutdown_rx: watch::Receiver<bool>,
    /// The thread the background work runs on.
    #[cfg(feature = "full")]
    pub(crate) worker_thread: Mutex<Option<JoinHandle<()>>>,
}

impl Validator {
    /// Builds a validator, loads the embedded (and the overridden) definitions,
    /// and hydrates the caches from disk.
    ///
    /// The constructor removes the reverse-DNS cache of a crawler whose
    /// definition no longer asks for the reverse DNS: the file belongs to the
    /// crate, and no code reads it any more.
    ///
    /// The method does **not** start the background work — call
    /// [`Validator::start_background`] for that.
    pub fn new(options: ValidatorOptions) -> Result<Arc<Self>, ConfigError> {
        let config = Config::from(options);
        std::fs::create_dir_all(&config.root)
            .map_err(|error| ConfigError::Io(format!("{}: {error}", config.root.display())))?;
        let (mut bots, config_errors) = load_bots_with_errors(Some(&config.root))?;

        for bot in &mut bots {
            let bot_dir = config.root.join(&bot.name);
            std::fs::create_dir_all(&bot_dir)
                .map_err(|error| ConfigError::Io(format!("{}: {error}", bot_dir.display())))?;
            bot.load_cached_ips(&bot_dir.join("ips.txt"));
            if bot.rdns {
                bot.rdns_cache = Some(Arc::new(RdnsCache::open(bot_dir.join("rdns.txt"))?));
                bot.fail_cache = Some(Arc::new(FailLru::new(config.fail_limit)));
            } else {
                let stale = bot_dir.join("rdns.txt");
                if let Err(error) = remove_stale_rdns_cache(&stale) {
                    tracing::warn!(
                        "knownbots: the stale rdns cache {} could not be removed: {error}",
                        stale.display()
                    );
                }
            }
        }

        let cold = count_cold_url_bots(&bots);
        if cold > 0 {
            tracing::warn!(
                cold,
                "knownbots: cold start, the bots that are backed by a document are unverified until the first refresh succeeds"
            );
        }

        #[cfg(feature = "full")]
        let (rdns_tx, rdns_rx) = mpsc::channel(crate::worker::RDNS_QUEUE);
        #[cfg(feature = "full")]
        let http = match config.http_client.clone() {
            Some(client) => client,
            None => crate::http::build_client()?,
        };
        #[cfg(feature = "full")]
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        Ok(Arc::new(Self {
            root: config.root,
            bots,
            shutdown: AtomicBool::new(false),
            #[cfg(feature = "full")]
            workers_started: AtomicBool::new(false),
            refresh_errors: Mutex::new(HashMap::new()),
            config_errors,
            refresh_interval: config.refresh_interval,
            refresh_on_start: config.refresh_on_start,
            #[cfg(feature = "full")]
            rdns_tx,
            #[cfg(feature = "full")]
            rdns_rx: Mutex::new(Some(rdns_rx)),
            #[cfg(feature = "full")]
            rdns_overflow: AtomicU64::new(0),
            #[cfg(feature = "full")]
            in_flight: Mutex::new(HashSet::new()),
            #[cfg(feature = "full")]
            http,
            #[cfg(feature = "full")]
            shutdown_tx,
            #[cfg(feature = "full")]
            shutdown_rx,
            #[cfg(feature = "full")]
            worker_thread: Mutex::new(None),
        }))
    }

    /// Builds a validator with the default options and the given directory.
    pub fn with_root(root: impl Into<PathBuf>) -> Result<Arc<Self>, ConfigError> {
        Self::new(ValidatorOptions {
            root: root.into(),
            ..ValidatorOptions::default()
        })
    }

    /// Starts the background work: the reverse lookups and the refresh of the
    /// published lists.
    ///
    /// The work runs on a thread of the crate that has a runtime of its own, so
    /// the host needs no runtime for it and its own runtime never carries the
    /// work of the crate.
    ///
    /// The scheduler runs an **immediate** refresh, so that the bots that are
    /// backed by a document are usable within seconds of the start of the
    /// process. A failed refresh is retried with an exponential backoff instead
    /// of waiting a whole day.
    ///
    /// The method is idempotent — the second call answers `false` — and it
    /// answers `false` when the thread could not be started.
    #[cfg(feature = "full")]
    #[must_use = "the background work only runs when the call is made"]
    pub fn start_background(self: &Arc<Self>) -> bool {
        if self
            .workers_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return false;
        }
        let Some(receiver) = self.rdns_rx.lock().take() else {
            return false;
        };
        let validator = Arc::downgrade(self);
        let shutdown = self.shutdown_rx.clone();
        match std::thread::Builder::new()
            .name("knownbots-worker".to_string())
            .spawn(move || crate::worker::run(validator, receiver, shutdown))
        {
            Ok(thread) => {
                *self.worker_thread.lock() = Some(thread);
                true
            }
            Err(error) => {
                tracing::error!(
                    %error,
                    "knownbots: the worker thread could not be started"
                );
                false
            }
        }
    }

    /// Verifies the claim of a request.
    ///
    /// Only [`VerifyStatus::Verified`] says that the request is the bot it
    /// claims to be. A request that names a bot of the registry but comes from
    /// an address the vendor does not publish is not that bot.
    ///
    /// When the reverse DNS of the address decides and the cache of the bot is
    /// cold, the lookup is queued and the request fails closed: the next
    /// request may succeed.
    pub fn verify(&self, ua: &str, ip: IpAddr) -> VerifyResult {
        let Some(position) = find_bot_by_ua(ua, &self.bots) else {
            return VerifyResult::unknown();
        };
        let bot = &self.bots[position];

        // 1) The published address space — the fastest path.
        if bot.contains_ip(ip) {
            return VerifyResult::verified(bot);
        }

        // 2) The reverse DNS path, only for the bots that are configured for it.
        if bot.rdns {
            let key = ip.to_string();
            if let Some(fail_cache) = bot.fail_cache.as_ref()
                && fail_cache.contains(&key)
            {
                return VerifyResult::non_verified(VerifyStatus::Failed, bot);
            }
            if let Some(cache) = bot.rdns_cache.as_ref()
                && let Some(hostname) = cache.get(&key)
            {
                if match_domain(&hostname, &bot.domains) {
                    return VerifyResult::verified(bot);
                }
                return VerifyResult::non_verified(VerifyStatus::Failed, bot);
            }
            // The cache is cold. The lookup is queued and this request fails
            // closed.
            #[cfg(feature = "full")]
            {
                self.enqueue_rdns(bot.name.clone(), ip);
                return VerifyResult::non_verified(VerifyStatus::Pending, bot);
            }
            #[cfg(not(feature = "full"))]
            {
                return VerifyResult::non_verified(VerifyStatus::Failed, bot);
            }
        }

        // The marker named a bot that is verified by its address space only,
        // and the address is not in it — including the window before the first
        // refresh filled it.
        VerifyResult::non_verified(VerifyStatus::Failed, bot)
    }

    /// Queues the reverse lookup of an address.
    #[cfg(feature = "full")]
    fn enqueue_rdns(&self, bot_name: String, ip: IpAddr) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }
        {
            let mut in_flight = self.in_flight.lock();
            if !in_flight.insert((bot_name.clone(), ip)) {
                return;
            }
        }
        match self.rdns_tx.try_send(RdnsJob { bot_name, ip }) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(job))
            | Err(mpsc::error::TrySendError::Closed(job)) => {
                self.in_flight.lock().remove(&(job.bot_name, job.ip));
                let total = self.rdns_overflow.fetch_add(1, Ordering::Relaxed) + 1;
                // The first drop is reported, and then every thousandth.
                if total == 1 || total.is_multiple_of(1000) {
                    tracing::warn!(
                        total,
                        queue = crate::worker::RDNS_QUEUE,
                        "knownbots: the queue of the reverse lookups overflowed; the verification stays fail-closed until the queue drains"
                    );
                }
            }
        }
    }

    /// Seeds a prefix into the address space of a bot.
    ///
    /// The method is the seam for the tests of a host and for the fixtures of an
    /// offline installation.
    pub fn seed_prefix(&self, bot_name: &str, cidr: &str) -> Result<(), ConfigError> {
        let net: ipnet::IpNet = cidr
            .parse()
            .map_err(|error| ConfigError::Registry(format!("{cidr} is not a CIDR: {error}")))?;
        let bot = self.bot(bot_name)?;
        bot.store_downloaded(vec![net]);
        Ok(())
    }

    /// Seeds a reverse DNS entry into the cache of a bot.
    pub fn seed_rdns(&self, bot_name: &str, ip: &str, hostname: &str) -> Result<(), ConfigError> {
        let bot = self.bot(bot_name)?;
        let cache = bot
            .rdns_cache
            .as_ref()
            .ok_or_else(|| ConfigError::Registry(format!("bot {bot_name} has no rdns cache")))?;
        cache.set(ip, hostname);
        Ok(())
    }

    /// Records the failure of a refresh.
    ///
    /// The sibling of [`Validator::seed_prefix`] and [`Validator::seed_rdns`]:
    /// the failure a refresh would record can be injected without network
    /// access.
    pub fn seed_refresh_error(
        &self,
        bot_name: &str,
        failure: RefreshFailure,
    ) -> Result<(), ConfigError> {
        let bot = self.bot(bot_name)?;
        self.refresh_errors.lock().insert(bot.name.clone(), failure);
        Ok(())
    }

    /// Downloads the lists of every bot that is backed by a document.
    ///
    /// The downloads run concurrently — a dead host would otherwise hold up the
    /// bots that follow it for the whole timeout of the client.
    ///
    /// The method answers `true` when every bot produced a non-empty prefix set
    /// or kept a previous one after a failure.
    #[cfg(feature = "full")]
    pub async fn refresh(&self) -> bool {
        let mut tasks = tokio::task::JoinSet::new();
        for bot in &self.bots {
            if bot.urls.is_empty() {
                continue;
            }
            let http = self.http.clone();
            let parser = bot.parser.clone();
            let urls = bot.urls.clone();
            let name = bot.name.clone();
            tasks.spawn(async move {
                let downloaded = feed::download_prefixes(&http, &parser, &urls).await;
                (name, downloaded)
            });
        }

        let mut all_ok = true;
        while let Some(joined) = tasks.join_next().await {
            let (name, downloaded) = match joined {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!("knownbots: the refresh of a bot failed: {error}");
                    all_ok = false;
                    continue;
                }
            };
            let Some(bot) = self.bots.iter().find(|bot| bot.name == name) else {
                continue;
            };
            if !downloaded.prefixes.is_empty() {
                bot.store_downloaded(downloaded.prefixes);
            }
            match downloaded.failure {
                None => {
                    self.refresh_errors.lock().remove(&name);
                    let path = self.root.join(&bot.name).join("ips.txt");
                    if let Err(error) = bot.persist_ips(&path) {
                        tracing::warn!(
                            bot = %name,
                            %error,
                            "knownbots: the prefixes could not be persisted"
                        );
                        self.refresh_errors.lock().insert(
                            name.clone(),
                            RefreshFailure::Broken(format!("persist ips: {error}")),
                        );
                        all_ok = false;
                    } else {
                        tracing::info!(
                            bot = %name,
                            prefixes = bot.prefixes.read().len(),
                            "knownbots: the list was refreshed"
                        );
                    }
                }
                Some(failure) => {
                    tracing::warn!(bot = %name, %failure, "knownbots: the list could not be refreshed");
                    self.refresh_errors.lock().insert(name.clone(), failure);
                    if bot.prefixes.read().is_empty() && bot.custom.is_empty() {
                        all_ok = false;
                    }
                }
            }
        }

        self.flush_rdns_caches();
        all_ok
    }

    /// Downloads the list of one bot and applies it.
    ///
    /// The method is the single-bot form of [`Validator::refresh`]. It answers
    /// the failure of the refresh, if the refresh failed.
    #[cfg(feature = "full")]
    pub async fn refresh_bot(&self, name: &str) -> Option<RefreshFailure> {
        let Some(bot) = self.bots.iter().find(|bot| bot.name == name) else {
            return Some(RefreshFailure::Broken(format!("unknown bot {name}")));
        };
        if bot.urls.is_empty() {
            return None;
        }
        let downloaded = feed::download_prefixes(&self.http, &bot.parser, &bot.urls).await;
        if !downloaded.prefixes.is_empty() {
            bot.store_downloaded(downloaded.prefixes);
        }
        match downloaded.failure {
            None => {
                self.refresh_errors.lock().remove(name);
                let path = self.root.join(&bot.name).join("ips.txt");
                if let Err(error) = bot.persist_ips(&path) {
                    let failure = RefreshFailure::Broken(format!("persist ips: {error}"));
                    self.refresh_errors
                        .lock()
                        .insert(name.to_string(), failure.clone());
                    return Some(failure);
                }
                None
            }
            Some(failure) => {
                self.refresh_errors
                    .lock()
                    .insert(name.to_string(), failure.clone());
                Some(failure)
            }
        }
    }

    /// Writes the caches of the reverse DNS to disk.
    #[cfg(feature = "full")]
    fn flush_rdns_caches(&self) {
        for bot in &self.bots {
            if bot.rdns
                && let Some(cache) = bot.rdns_cache.as_ref()
            {
                cache.prune(&bot.domains);
                if let Err(error) = cache.persist_if_dirty() {
                    tracing::warn!(bot = %bot.name, %error, "knownbots: the rdns cache could not be persisted");
                }
            }
        }
    }

    /// Why the most recent refresh of a bot failed, if it did.
    ///
    /// `None` means the last refresh succeeded, or never ran.
    /// [`RefreshFailure::Unreachable`] is an *environment* signal — the
    /// endpoint of the vendor refused the request or the network was down —
    /// while [`RefreshFailure::Broken`] points at this crate (the list did not
    /// parse, the handshake failed).
    pub fn last_refresh_error(&self, bot_name: &str) -> Option<RefreshFailure> {
        self.refresh_errors.lock().get(bot_name).cloned()
    }

    /// The health of the address space of every bot of the registry.
    ///
    /// One entry per bot, in the order of the registry. The state is derived
    /// from the state the validator already knows: a recorded failure means
    /// `Failed`; a bot that is backed by a document and holds no prefix means
    /// `Cold`; everything else is `Ready`.
    pub fn refresh_status(&self) -> Vec<BotRefreshStatus> {
        let errors = self.refresh_errors.lock();
        self.bots
            .iter()
            .map(|bot| {
                let prefixes = bot.prefixes.read().len();
                let error = errors.get(&bot.name).cloned();
                let state = if error.is_some() {
                    BotRefreshState::Failed
                } else if !bot.urls.is_empty() && prefixes == 0 {
                    BotRefreshState::Cold
                } else {
                    BotRefreshState::Ready
                };
                BotRefreshStatus {
                    name: bot.name.clone(),
                    vendor: bot.vendor.clone(),
                    kind: bot.kind,
                    prefixes,
                    state,
                    error,
                    loaded_at: bot.loaded_at(),
                }
            })
            .collect()
    }

    /// The definitions of the operator that were skipped at load time.
    ///
    /// The list is empty when every file of the override directory parsed. A
    /// malformed or incomplete **embedded** definition never reaches this list —
    /// it fails the constructor instead.
    pub fn config_errors(&self) -> Vec<BotConfigError> {
        self.config_errors.clone()
    }

    /// The reverse lookups that were dropped because the queue was full.
    ///
    /// The verification stays fail-closed for the affected requests until the
    /// queue drains; the count tells the operator that the path is not keeping
    /// up.
    pub fn rdns_overflow(&self) -> u64 {
        #[cfg(feature = "full")]
        {
            self.rdns_overflow.load(Ordering::Relaxed)
        }
        #[cfg(not(feature = "full"))]
        {
            0
        }
    }

    /// An address inside the address space of a bot, if one exists.
    pub fn covered_ip(&self, bot_name: &str) -> Option<IpAddr> {
        let bot = self.bots.iter().find(|bot| bot.name == bot_name)?;
        let prefixes = bot.prefixes.read();
        crate::index::prefix::witness(&prefixes)
    }

    /// The number of bots of the registry.
    pub fn bot_count(&self) -> usize {
        self.bots.len()
    }

    /// The registry this verifier knows.
    ///
    /// The embedded definitions plus whatever the operator dropped into
    /// `<root>/conf.d/` — the same bots a request is checked against.
    pub fn bots(&self) -> &[Bot] {
        &self.bots
    }

    /// The directory that holds the caches of the bots.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The interval between two refreshes of the published lists.
    pub fn refresh_interval(&self) -> Duration {
        self.refresh_interval
    }

    /// Whether the worker refreshes the lists as soon as it starts.
    pub fn refreshes_on_start(&self) -> bool {
        self.refresh_on_start
    }

    /// Stops the background work.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        #[cfg(feature = "full")]
        let _ = self.shutdown_tx.send(true);
    }

    /// The bot of the registry with that id.
    fn bot(&self, bot_name: &str) -> Result<&Bot, ConfigError> {
        self.bots
            .iter()
            .find(|bot| bot.name == bot_name)
            .ok_or_else(|| ConfigError::Registry(format!("unknown bot {bot_name}")))
    }
}

/// Removes the reverse-DNS cache a previous definition left behind.
///
/// The file belongs to the crate, and the definition of the crawler states that
/// no code reads it any more, so the crate removes it. A file that is not there
/// is not an error — the crawler never had one. Any other failure is reported
/// to the caller: a stale file is harmless, and the startup goes on.
fn remove_stale_rdns_cache(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// The validator stops its worker thread when it is dropped.
///
/// The thread holds the validator weakly, so it never keeps the validator
/// alive: the thread is told to stop and finishes the work that is in flight.
impl Drop for Validator {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        #[cfg(feature = "full")]
        let _ = self.shutdown_tx.send(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn google_ua() -> &'static str {
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)"
    }

    #[test]
    fn verified_on_seeded_cidr() {
        let directory = tempfile::tempdir().unwrap();
        let validator = Validator::with_root(directory.path()).unwrap();
        validator
            .seed_prefix("googlebot", "66.249.64.0/19")
            .unwrap();
        let ip = IpAddr::V4(Ipv4Addr::new(66, 249, 66, 1));
        let result = validator.verify(google_ua(), ip);
        assert!(result.is_verified());
        assert_eq!(result.name, "googlebot");
        assert_eq!(result.vendor, "Google");
    }

    #[test]
    fn forged_ip_fails_closed() {
        let directory = tempfile::tempdir().unwrap();
        let validator = Validator::with_root(directory.path()).unwrap();
        validator
            .seed_prefix("googlebot", "66.249.64.0/19")
            .unwrap();
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        let result = validator.verify(google_ua(), ip);
        assert_eq!(result.status, VerifyStatus::Failed);
        assert!(!result.is_verified());
    }

    #[test]
    fn unknown_ua() {
        let directory = tempfile::tempdir().unwrap();
        let validator = Validator::with_root(directory.path()).unwrap();
        let ip = IpAddr::V4(Ipv4Addr::new(66, 249, 66, 1));
        let result = validator.verify("Mozilla/5.0 Chrome/120.0", ip);
        assert_eq!(result.status, VerifyStatus::Unknown);
    }

    #[test]
    fn wrong_case_ua_is_unknown() {
        let directory = tempfile::tempdir().unwrap();
        let validator = Validator::with_root(directory.path()).unwrap();
        validator
            .seed_prefix("googlebot", "66.249.64.0/19")
            .unwrap();
        let ip = IpAddr::V4(Ipv4Addr::new(66, 249, 66, 1));
        let result = validator.verify("mozilla/5.0 (compatible; googlebot/2.1)", ip);
        assert_eq!(result.status, VerifyStatus::Unknown);
    }

    #[test]
    fn rdns_cache_hit_verifies() {
        let directory = tempfile::tempdir().unwrap();
        let validator = Validator::with_root(directory.path()).unwrap();
        let ip = IpAddr::V4(Ipv4Addr::new(220, 181, 108, 94));
        validator
            .seed_rdns("baiduspider", &ip.to_string(), "crawl.baidu.com")
            .unwrap();
        let ua =
            "Mozilla/5.0 (compatible; Baiduspider/2.0; +http://www.baidu.com/search/spider.html)";
        let result = validator.verify(ua, ip);
        assert!(result.is_verified());
        assert_eq!(result.name, "baiduspider");
    }

    #[test]
    fn facebook_custom_cidr() {
        let directory = tempfile::tempdir().unwrap();
        let validator = Validator::with_root(directory.path()).unwrap();
        let ip = IpAddr::V4(Ipv4Addr::new(31, 13, 24, 10));
        let ua = "facebookexternalhit/1.1 (+http://www.facebook.com/externalhit_uatext.php)";
        let result = validator.verify(ua, ip);
        assert!(result.is_verified());
        assert_eq!(result.name, "facebookexternalhit");
    }

    #[test]
    fn covered_ip_is_network_address_of_seeded_prefix() {
        let directory = tempfile::tempdir().unwrap();
        let validator = Validator::with_root(directory.path()).unwrap();
        validator
            .seed_prefix("googlebot", "66.249.64.0/27")
            .unwrap();
        assert_eq!(
            validator.covered_ip("googlebot").unwrap(),
            "66.249.64.0".parse::<IpAddr>().unwrap()
        );
        assert!(validator.covered_ip("missing-bot").is_none());
    }

    #[test]
    fn cold_url_bot_fails_until_seeded() {
        // Without a cached or seeded prefix list, a Googlebot claim fails
        // closed. Production relies on the immediate refresh of the scheduler to
        // fill this gap.
        let directory = tempfile::tempdir().unwrap();
        let validator = Validator::with_root(directory.path()).unwrap();
        let ip = IpAddr::V4(Ipv4Addr::new(66, 249, 66, 1));
        let result = validator.verify(google_ua(), ip);
        assert_eq!(result.status, VerifyStatus::Failed);
    }

    /// The three states of the address space, derived from the state the
    /// validator already keeps:
    ///
    /// - `Ready` — a loaded prefix set (downloaded or stated) or no document
    ///   at all (the bots that are verified by their reverse DNS).
    /// - `Cold` — backed by a document, never downloaded.
    /// - `Failed` — the last refresh failed.
    #[test]
    fn refresh_status_reports_ready_cold_and_failed() {
        let directory = tempfile::tempdir().unwrap();
        let validator = Validator::with_root(directory.path()).unwrap();

        let status = |name: &str| {
            validator
                .refresh_status()
                .into_iter()
                .find(|status| status.name == name)
                .unwrap_or_else(|| panic!("no status for {name}"))
        };

        // A bot that is verified by its reverse DNS has no document to
        // download, so it is ready.
        assert_eq!(status("baiduspider").state, BotRefreshState::Ready);
        // Backed by a document, never downloaded.
        assert_eq!(status("bingbot").state, BotRefreshState::Cold);
        assert_eq!(status("bingbot").prefixes, 0);

        // A downloaded list makes it ready.
        validator.seed_prefix("bingbot", "40.77.0.0/16").unwrap();
        assert_eq!(status("bingbot").state, BotRefreshState::Ready);
        assert_eq!(status("bingbot").prefixes, 1);

        // A recorded failure outranks everything.
        validator
            .seed_refresh_error(
                "bingbot",
                RefreshFailure::Unreachable("https://example.invalid refused".into()),
            )
            .unwrap();
        let bing = status("bingbot");
        assert_eq!(bing.state, BotRefreshState::Failed);
        assert!(matches!(bing.error, Some(RefreshFailure::Unreachable(_))));
        // Unknown bots are rejected by the seam, not silently ignored.
        assert!(
            validator
                .seed_refresh_error("no-such-bot", RefreshFailure::Broken("x".into()))
                .is_err()
        );
    }

    /// A bot with only the prefixes of its definition is `Ready` — those
    /// prefixes *are* its address space.
    #[test]
    fn custom_cidr_bot_is_ready() {
        let directory = tempfile::tempdir().unwrap();
        let conf = directory.path().join("conf.d");
        std::fs::create_dir_all(&conf).unwrap();
        std::fs::write(
            conf.join("custombot.yaml"),
            "name: custombot\nua: \"CustomBot\"\nvendor: \"Example\"\ncustom:\n  - \"31.13.24.0/24\"\n",
        )
        .unwrap();
        let validator = Validator::with_root(directory.path()).unwrap();
        let status = validator
            .refresh_status()
            .into_iter()
            .find(|status| status.name == "custombot")
            .expect("custombot in the registry");
        assert_eq!(status.state, BotRefreshState::Ready);
        assert_eq!(status.prefixes, 1);
    }

    /// A malformed operator override is skipped and reported — it does not
    /// stop the process from starting, and it does not take the rest of the
    /// registry with it.
    #[test]
    fn malformed_override_file_does_not_stop_startup() {
        let directory = tempfile::tempdir().unwrap();
        let conf = directory.path().join("conf.d");
        std::fs::create_dir_all(&conf).unwrap();
        let broken = conf.join("broken.yaml");
        std::fs::write(&broken, "name: brokenbot\n  ua: [unclosed\n").unwrap();
        std::fs::write(
            conf.join("goodbot.yaml"),
            "name: goodbot\nua: \"GoodBot\"\nvendor: \"Example\"\n",
        )
        .unwrap();

        let validator = Validator::with_root(directory.path())
            .expect("a malformed override file must not stop the startup");
        assert!(!validator.bots().iter().any(|bot| bot.name == "brokenbot"));
        assert!(validator.bots().iter().any(|bot| bot.name == "goodbot"));
        assert!(validator.bots().iter().any(|bot| bot.name == "googlebot"));

        let errors = validator.config_errors();
        assert_eq!(errors.len(), 1, "errors: {errors:?}");
        assert_eq!(errors[0].path, broken.display().to_string());
        assert!(
            errors[0].message.contains("parse bot config"),
            "message must name the parse failure: {}",
            errors[0].message
        );
    }

    /// Drops a definition of a crawler of the host into the override
    /// directory of the root.
    ///
    /// The crawler of the test states its own premise, so a change of the
    /// registry cannot break it.
    fn write_override(root: &std::path::Path, name: &str, definition: &str) {
        let conf = root.join("conf.d");
        std::fs::create_dir_all(&conf).unwrap();
        std::fs::write(conf.join(format!("{name}.yaml")), definition).unwrap();
    }

    /// A crawler that is no longer verified by its reverse DNS leaves no
    /// reverse-DNS cache behind.
    ///
    /// The file belongs to the crate, and the definition of the crawler states
    /// that no code reads it any more, so the constructor of the validator
    /// removes it. The rule is driven by the `rdns` flag of the definition, so
    /// it holds for the definitions of a host as well.
    #[test]
    fn a_crawler_without_reverse_dns_leaves_no_cache_behind() {
        let directory = tempfile::tempdir().unwrap();
        write_override(
            directory.path(),
            "listbot",
            "name: listbot\nua: \"ListBot\"\nvendor: \"Example\"\nurls:\n  - \"https://example.invalid/listbot.json\"\n",
        );
        let bot_dir = directory.path().join("listbot");
        std::fs::create_dir_all(&bot_dir).unwrap();
        let cache = bot_dir.join("rdns.txt");
        std::fs::write(&cache, "203.0.113.7 listbot.example.com\n").unwrap();

        Validator::with_root(directory.path()).expect("a validator");
        assert!(!cache.exists(), "the stale cache is still there");
    }

    /// A crawler that is verified by its reverse DNS keeps its cache file and
    /// reads it.
    #[test]
    fn a_reverse_dns_crawler_keeps_its_cache_and_reads_it() {
        let directory = tempfile::tempdir().unwrap();
        let bot_dir = directory.path().join("baiduspider");
        std::fs::create_dir_all(&bot_dir).unwrap();
        let cache = bot_dir.join("rdns.txt");
        std::fs::write(&cache, "220.181.108.94 crawl.baidu.com\n").unwrap();

        let validator = Validator::with_root(directory.path()).unwrap();
        let ip = IpAddr::V4(Ipv4Addr::new(220, 181, 108, 94));
        let ua =
            "Mozilla/5.0 (compatible; Baiduspider/2.0; +http://www.baidu.com/search/spider.html)";
        let result = validator.verify(ua, ip);
        assert!(result.is_verified());
        assert_eq!(result.name, "baiduspider");
        assert!(
            cache.exists(),
            "the cache of a reverse-DNS crawler was removed"
        );
    }

    /// A cache file that is not there is not a failure — the crawler never had
    /// one, and the removal of it stays silent.
    #[test]
    fn a_missing_cache_file_is_not_a_failure() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rdns.txt");
        remove_stale_rdns_cache(&path).expect("a missing file is not a failure");
    }

    /// A cache file that cannot be removed is reported, and the startup goes
    /// on: a stale file is harmless, because no code reads it any more.
    #[test]
    fn a_cache_file_that_cannot_be_removed_does_not_stop_the_startup() {
        let directory = tempfile::tempdir().unwrap();
        write_override(
            directory.path(),
            "listbot",
            "name: listbot\nua: \"ListBot\"\nvendor: \"Example\"\nurls:\n  - \"https://example.invalid/listbot.json\"\n",
        );
        let bot_dir = directory.path().join("listbot");
        std::fs::create_dir_all(&bot_dir).unwrap();
        // A directory in the place of the file cannot be unlinked — not even by
        // root — so the removal fails for every user and on every platform.
        let cache = bot_dir.join("rdns.txt");
        std::fs::create_dir(&cache).unwrap();

        let started = Validator::with_root(directory.path());

        assert!(started.is_ok(), "the startup must go on");
        assert!(cache.exists(), "the stale cache is still there");
    }
}
