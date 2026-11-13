//! The worker thread of the crate.
//!
//! The refresh of the published lists and the reverse lookups run on a thread
//! of the crate that has a runtime of its own. The runtime of the host is never
//! used for them, the caller is never blocked by them, and the host needs no
//! runtime of its own for the work of the crate.
//!
//! The thread starts when the host asks for it and stops when the last handle
//! of the validator is dropped.

use std::net::IpAddr;
use std::sync::Arc;
use std::sync::Weak;
use std::time::Duration;

use tokio::sync::{mpsc, watch};

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

/// Runs the worker on a thread with a runtime of its own.
///
/// The thread holds the validator weakly, so that it never keeps the validator
/// alive: the validator stops the thread when it is dropped.
pub(crate) fn run(
    validator: Weak<Validator>,
    receiver: mpsc::Receiver<RdnsJob>,
    shutdown: watch::Receiver<bool>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(
                %error,
                "knownbots: the runtime of the worker could not be built"
            );
            return;
        }
    };

    runtime.block_on(async {
        let scheduler = tokio::spawn(run_scheduler(validator.clone(), shutdown.clone()));
        run_rdns_pool(validator, receiver, shutdown).await;
        scheduler.abort();
    });
}

/// Runs the reverse lookups of the queued jobs until the worker is stopped.
async fn run_rdns_pool(
    validator: Weak<Validator>,
    mut receiver: mpsc::Receiver<RdnsJob>,
    mut shutdown: watch::Receiver<bool>,
) {
    let semaphore = Arc::new(tokio::sync::Semaphore::new(RDNS_CONCURRENCY));
    loop {
        let job = tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            job = receiver.recv() => match job {
                Some(job) => job,
                None => break,
            },
        };
        let Some(validator) = validator.upgrade() else {
            break;
        };
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
}

/// Runs the refresh of the published lists over and over.
async fn run_scheduler(validator: Weak<Validator>, mut shutdown: watch::Receiver<bool>) {
    let mut backoff = REFRESH_RETRY_MIN;
    let mut first = true;
    loop {
        // The validator is only held while the work runs, so that it can be
        // dropped while the worker waits for the next refresh.
        let Some(validator) = validator.upgrade() else {
            break;
        };
        let interval = validator.refresh_interval();
        // The first pass runs immediately — without it, every crawler that is
        // backed by a document would fail closed until the first refresh
        // succeeded. A host that refreshes the lists itself turns that off.
        let complete = if first && !validator.refreshes_on_start() {
            first = false;
            true
        } else {
            first = false;
            validator.refresh().await
        };
        let wait = if complete {
            backoff = REFRESH_RETRY_MIN;
            interval
        } else {
            let wait = backoff;
            backoff = (backoff * 2).min(REFRESH_RETRY_MAX);
            tracing::warn!(
                seconds = wait.as_secs(),
                "knownbots: the refresh was incomplete; retrying"
            );
            wait
        };
        drop(validator);

        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            _ = tokio::time::sleep(wait) => {}
        }
    }
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
