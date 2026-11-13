//! The health of every source of the registry.
//!
//! The registry depends on the outside world: 23 of its entries are backed by
//! a published address space the crate downloads, 21 are verified by the
//! reverse DNS of their vendor, and one states its own prefixes. The check
//! reads every one of those sources and writes the outcome of the run to
//! `target/provider-health/report.json`.
//!
//! The sources are on the public internet, so the tests are ignored: the
//! ordinary suite of the crate never depends on the uptime of a vendor. Run
//! the check with:
//!
//! ```sh
//! cargo test --all-features --test provider_health -- --ignored
//! ```

#![cfg(feature = "full")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use knownbots::{Bot, RefreshFailure, Validator, ValidatorOptions};
use parking_lot::Mutex;
use serde::Serialize;
use serde::Serializer;

/// The bound of the resolution of one domain.
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(30);

/// The number of groups the check is split into.
const GROUPS: usize = 2;

/// The number of the groups that finished their sources.
static GROUPS_DONE: AtomicUsize = AtomicUsize::new(0);

/// The kind of the source an entry of the registry depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// A published address space, downloaded from the documents of the entry.
    List,
    /// The reverse DNS of the vendor.
    Dns,
    /// The prefixes the entry states itself.
    Static,
}

impl Source {
    /// The name of the kind in the report.
    fn name(self) -> &'static str {
        match self {
            Source::List => "list",
            Source::Dns => "dns",
            Source::Static => "static",
        }
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl Serialize for Source {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

/// The outcome of the check for one source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// The check ran and the source is fine.
    Healthy,
    /// The check ran and the source is broken.
    Failed,
    /// The source has no upstream to check.
    Skipped,
    /// The check did not reach the source.
    Pending,
}

impl State {
    /// The name of the state in the report.
    fn name(self) -> &'static str {
        match self {
            State::Healthy => "healthy",
            State::Failed => "failed",
            State::Skipped => "skipped",
            State::Pending => "pending",
        }
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl Serialize for State {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

/// Who is at fault when a source is broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fault {
    /// The upstream or the network is at fault.
    Environment,
    /// This crate is at fault.
    Crate,
}

impl Fault {
    /// The name of the fault in the report.
    fn name(self) -> &'static str {
        match self {
            Fault::Environment => "environment",
            Fault::Crate => "crate",
        }
    }
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl Serialize for Fault {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

/// One row of the report: the outcome of the check for one entry of the
/// registry.
#[derive(Debug, Clone, Serialize)]
struct Row {
    /// The id of the entry in the registry.
    name: String,
    /// The organisation that owns the entry.
    vendor: String,
    /// The category of the entry.
    kind: String,
    /// The kind of the source the entry depends on.
    source: Source,
    /// The outcome of the check.
    state: State,
    /// Who is at fault, when the source is broken.
    fault: Option<Fault>,
    /// The number of prefixes the entry holds.
    prefixes: usize,
    /// The reason of the outcome.
    detail: Option<String>,
}

impl Row {
    /// The row as a line of the verdict of the run.
    fn describe(&self) -> String {
        let mut label = format!("{} [{}", self.name, self.state);
        if let Some(fault) = self.fault {
            label.push_str(", ");
            label.push_str(&fault.to_string());
        }
        label.push(']');
        let reason = self
            .detail
            .as_deref()
            .unwrap_or("the check did not reach the source");
        format!("{label}: {reason}")
    }
}

/// The report of a run.
#[derive(Debug, Serialize)]
struct Report {
    /// The Unix time of the start of the run.
    checked_at: u64,
    /// One row per entry of the registry, sorted by the name of the entry.
    sources: Vec<Row>,
}

/// The state the groups of the check share.
///
/// The two tests of a binary run in parallel, so the report is written behind
/// a lock.
struct Shared {
    /// One row per entry of the registry.
    rows: BTreeMap<String, Row>,
    /// The path of the report.
    path: PathBuf,
    /// The Unix time of the start of the run.
    checked_at: u64,
}

impl Shared {
    /// Rewrites the report.
    ///
    /// The file is replaced and never appended to, and the replacement is
    /// atomic: the file is a complete document at every moment it exists.
    fn write(&self) {
        let report = Report {
            checked_at: self.checked_at,
            sources: self.rows.values().cloned().collect(),
        };
        let body = serde_json::to_string_pretty(&report).expect("the report must be serialisable");
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).expect("the directory of the report must be writable");
        }
        let temporary = self.path.with_extension("tmp");
        std::fs::write(&temporary, body).expect("the report must be writable");
        std::fs::rename(&temporary, &self.path).expect("the report must be replaceable");
    }
}

/// The state of the check, seeded from the registry.
///
/// The seeding happens once, when the first group of the check starts: every
/// entry of the registry gets a row, so that a source the check did not reach
/// is `pending` rather than absent.
fn shared() -> &'static Mutex<Shared> {
    static SHARED: OnceLock<Mutex<Shared>> = OnceLock::new();
    SHARED.get_or_init(|| {
        let mut rows = BTreeMap::new();
        for bot in registry() {
            rows.insert(bot.name.clone(), seed(&bot));
        }
        let shared = Shared {
            rows,
            path: report_path(),
            checked_at: now(),
        };
        shared.write();
        Mutex::new(shared)
    })
}

/// The path of the report.
fn report_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("provider-health")
        .join("report.json")
}

/// The Unix time of now, in seconds.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

/// The registry of the crate.
fn registry() -> Vec<Bot> {
    knownbots::load_bots(None).expect("the embedded registry must load")
}

/// The kind of the source an entry depends on.
fn source_of(bot: &Bot) -> Source {
    if !bot.urls.is_empty() {
        Source::List
    } else if bot.rdns {
        Source::Dns
    } else {
        Source::Static
    }
}

/// The row of an entry the check did not reach yet.
fn seed(bot: &Bot) -> Row {
    let source = source_of(bot);
    let (state, detail) = match source {
        // An entry that states its own prefixes has no upstream to check.
        Source::Static => (
            State::Skipped,
            Some("the entry states its own prefixes".to_string()),
        ),
        _ => (State::Pending, None),
    };
    Row {
        name: bot.name.clone(),
        vendor: bot.vendor.clone(),
        kind: format!("{:?}", bot.kind),
        source,
        state,
        fault: None,
        prefixes: bot.prefix_count(),
        detail,
    }
}

/// The outcome of the check of a source.
enum Outcome {
    /// The source is fine.
    Healthy,
    /// The source is broken.
    Failed {
        /// Who is at fault.
        fault: Fault,
        /// Why the source is broken.
        reason: String,
    },
}

impl Outcome {
    /// The outcome of a refresh the crate reported.
    ///
    /// The fault is the one the crate reported: the check does not classify a
    /// failure a second time.
    fn of(failure: Option<RefreshFailure>) -> Self {
        match failure {
            None => Outcome::Healthy,
            Some(RefreshFailure::Unreachable(reason)) => Outcome::Failed {
                fault: Fault::Environment,
                reason,
            },
            Some(RefreshFailure::Broken(reason)) => Outcome::Failed {
                fault: Fault::Crate,
                reason,
            },
        }
    }
}

/// Writes the outcome of a source into the report and rewrites the report.
fn record(name: &str, outcome: Outcome, prefixes: Option<usize>) {
    let mut shared = shared().lock();
    {
        let row = shared.rows.get_mut(name).expect("the row of the source");
        if let Some(prefixes) = prefixes {
            row.prefixes = prefixes;
        }
        match outcome {
            Outcome::Healthy => {
                row.state = State::Healthy;
                row.fault = None;
                row.detail = None;
            }
            Outcome::Failed { fault, reason } => {
                row.state = State::Failed;
                row.fault = Some(fault);
                row.detail = Some(reason);
            }
        }
    }
    shared.write();
    let row = &shared.rows[name];
    println!(
        "{}: {} ({} prefixes, {})",
        row.name,
        row.state,
        row.prefixes,
        row.detail.as_deref().unwrap_or("no reason")
    );
}

/// Fails the run when a source is not healthy.
///
/// The last group that finishes covers the whole registry; a group that
/// finishes while the other one is still running covers its own sources only,
/// so that a source the other group has not reached yet is not held against
/// it.
fn verdict(group: Source) {
    GROUPS_DONE.fetch_add(1, Ordering::SeqCst);
    let shared = shared().lock();
    let whole = GROUPS_DONE.load(Ordering::SeqCst) == GROUPS;
    let problems: Vec<String> = shared
        .rows
        .values()
        .filter(|row| whole || row.source == group)
        .filter(|row| row.state != State::Healthy && row.state != State::Skipped)
        .map(Row::describe)
        .collect();
    assert!(
        problems.is_empty(),
        "{} of the sources are not healthy:\n  - {}",
        problems.len(),
        problems.join("\n  - ")
    );
}

/// Checks every source that is a published address space.
///
/// The check runs the download of the crate itself, so that the failure it
/// reports is the failure a host would see.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "the endpoints of the vendors are on the public internet"]
async fn the_published_lists_are_read() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let validator = Validator::new(ValidatorOptions {
        root: directory.path().to_path_buf(),
        ..ValidatorOptions::default()
    })
    .expect("a validator");

    let mut tasks = tokio::task::JoinSet::new();
    for bot in registry() {
        if source_of(&bot) != Source::List {
            continue;
        }
        let validator = Arc::clone(&validator);
        tasks.spawn(async move {
            let name = bot.name.clone();
            let failure = validator.refresh_bot(&name).await;
            let prefixes = validator
                .bots()
                .iter()
                .find(|bot| bot.name == name)
                .map(Bot::prefix_count)
                .unwrap_or_default();
            (name, failure, prefixes)
        });
    }

    while let Some(joined) = tasks.join_next().await {
        // A source whose task panicked stays `pending`: the run fails on it,
        // and the other sources are still checked.
        match joined {
            Ok((name, failure, prefixes)) => record(&name, Outcome::of(failure), Some(prefixes)),
            Err(error) => eprintln!("a source could not be checked: {error}"),
        }
    }

    verdict(Source::List);
}

/// Checks every source that is the reverse DNS of a vendor.
///
/// The domains of the definition are resolved in the forward direction, which
/// is the confirmation the crate needs to answer for a reverse record.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "the endpoints of the vendors are on the public internet"]
async fn the_domains_of_the_crawlers_resolve() {
    let mut tasks = tokio::task::JoinSet::new();
    for bot in registry() {
        if source_of(&bot) != Source::Dns {
            continue;
        }
        let name = bot.name.clone();
        let domains = bot.domains.clone();
        tasks.spawn(async move {
            let failure = check_domains(&domains).await;
            (name, failure)
        });
    }

    while let Some(joined) = tasks.join_next().await {
        // A source whose task panicked stays `pending`: the run fails on it,
        // and the other sources are still checked.
        match joined {
            Ok((name, failure)) => {
                let outcome = match failure {
                    None => Outcome::Healthy,
                    Some(reason) => Outcome::Failed {
                        fault: Fault::Environment,
                        reason,
                    },
                };
                record(&name, outcome, None);
            }
            Err(error) => eprintln!("a source could not be checked: {error}"),
        }
    }

    verdict(Source::Dns);
}

/// Resolves every domain of a source and names the ones that do not resolve.
async fn check_domains(domains: &[String]) -> Option<String> {
    let mut failures = Vec::new();
    for domain in domains {
        let name = domain.clone();
        let outcome = tokio::time::timeout(
            RESOLVE_TIMEOUT,
            tokio::task::spawn_blocking(move || resolves(&name)),
        )
        .await;
        match outcome {
            Ok(Ok(true)) => {}
            Ok(Ok(false)) => failures.push(format!("{domain} does not resolve")),
            Ok(Err(error)) => failures.push(format!("{domain}: {error}")),
            Err(_) => failures.push(format!(
                "{domain} did not resolve within {}s",
                RESOLVE_TIMEOUT.as_secs()
            )),
        }
    }
    if failures.is_empty() {
        None
    } else {
        Some(failures.join("; "))
    }
}

/// Whether a domain holds an address.
fn resolves(domain: &str) -> bool {
    dns_lookup::lookup_host(domain)
        .map(|hosts| !hosts.is_empty())
        .unwrap_or(false)
}
