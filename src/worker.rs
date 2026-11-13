//! The background work of the crate.
//!
//! The reverse lookups and the refresh of the published lists run on the
//! runtime of the host, but never on the request path: a request that needs a
//! lookup queues it and fails closed.

use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::engine::Validator;
use crate::model::rdns::match_domain;

/// The number of the reverse lookups that may wait in the queue.
pub(crate) const RDNS_QUEUE: usize = 1024;

/// The time a reverse lookup may take.
const RDNS_TIMEOUT: Duration = Duration::from_secs(2);

/// The number of the reverse lookups that run at the same time.
const RDNS_CONCURRENCY: usize = 32;

/// The wait before the first retry of a failed refresh.
const REFRESH_RETRY_MIN: Duration = Duration::from_secs(5);

/// The longest wait between two retries of a failed refresh.
const REFRESH_RETRY_MAX: Duration = Duration::from_secs(5 * 60);

/// A reverse lookup that waits in the queue.
pub(crate) struct RdnsJob {
    pub(crate) bot_name: String,
    pub(crate) ip: IpAddr,
}

/// Runs the reverse lookups of the queued jobs.
pub(crate) fn spawn_rdns_workers(validator: Arc<Validator>, mut receiver: mpsc::Receiver<RdnsJob>) {
    tokio::spawn(async move {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(RDNS_CONCURRENCY));
        while let Some(job) = receiver.recv().await {
            if validator.shutdown.load(Ordering::Relaxed) {
                break;
            }
            let permit = match Arc::clone(&semaphore).acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => break,
            };
            let worker = Arc::clone(&validator);
            tokio::spawn(async move {
                let _permit = permit;
                run_rdns_job(&worker, job).await;
            });
        }
    });
}

/// Runs the refresh of the published lists, over and over.
pub(crate) fn spawn_scheduler(validator: Arc<Validator>) {
    tokio::spawn(async move {
        let mut backoff = REFRESH_RETRY_MIN;
        // The first pass runs immediately — without it, every bot that is
        // backed by a document would fail closed until the first refresh
        // succeeded.
        loop {
            if validator.shutdown.load(Ordering::Relaxed) {
                break;
            }
            let complete = validator.refresh().await;
            let wait = if complete {
                backoff = REFRESH_RETRY_MIN;
                validator.refresh_interval()
            } else {
                let wait = backoff;
                backoff = (backoff * 2).min(REFRESH_RETRY_MAX);
                tracing::warn!(
                    seconds = wait.as_secs(),
                    "knownbots: the refresh was incomplete; retrying"
                );
                wait
            };
            tokio::time::sleep(wait).await;
        }
    });
}

/// Runs one reverse lookup and remembers its answer.
async fn run_rdns_job(validator: &Validator, job: RdnsJob) {
    let key = (job.bot_name.clone(), job.ip);
    let result = tokio::time::timeout(RDNS_TIMEOUT, lookup_and_confirm(job.ip)).await;
    if let Some(bot) = validator.bots.iter().find(|bot| bot.name == job.bot_name) {
        match result {
            Ok(Ok(hostname)) if match_domain(&hostname, &bot.domains) => {
                if let Some(cache) = bot.rdns_cache.as_ref() {
                    // The memory only — writing the file is the scheduler's job.
                    cache.set(&job.ip.to_string(), &hostname);
                }
            }
            Ok(Ok(_)) | Ok(Err(LookupError::NotFound)) => {
                if let Some(fail_cache) = bot.fail_cache.as_ref() {
                    fail_cache.insert(&job.ip.to_string());
                }
            }
            // A transient failure does not poison the cache of the failures.
            Ok(Err(LookupError::Transient)) | Err(_) => {}
        }
    }
    validator.in_flight.lock().remove(&key);
}

/// Why a reverse lookup did not answer.
enum LookupError {
    /// The address has no reverse record, or the record does not confirm it.
    NotFound,
    /// The lookup failed in a way that says nothing about the address.
    Transient,
}

/// Looks up the hostname of an address and confirms it in the forward direction.
async fn lookup_and_confirm(ip: IpAddr) -> Result<String, LookupError> {
    let lookup = tokio::task::spawn_blocking(move || dns_lookup::lookup_addr(&ip))
        .await
        .map_err(|_| LookupError::Transient)?;

    let hostname = match lookup {
        Ok(hostname) => hostname.trim_end_matches('.').to_string(),
        Err(_) => return Err(LookupError::NotFound),
    };

    // The forward lookup has to answer with the same address.
    let host = hostname.clone();
    let addresses = tokio::task::spawn_blocking(move || dns_lookup::lookup_host(&host))
        .await
        .map_err(|_| LookupError::Transient)?;

    match addresses {
        Ok(list) if list.contains(&ip) => Ok(hostname),
        Ok(_) => Err(LookupError::NotFound),
        Err(_) => Err(LookupError::Transient),
    }
}
