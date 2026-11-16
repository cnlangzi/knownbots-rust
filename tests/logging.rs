//! The failure of the refresh of a crawler is logged.
//!
//! The test lives in a binary of its own: tracing registers the line of a log
//! message once per process, and the interest of that registration is the one
//! of the subscriber that is installed when the line is emitted for the first
//! time. A binary of its own keeps the test clear of that race.

#![cfg(feature = "full")]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use knownbots::{RefreshFailure, Validator, ValidatorOptions};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The fields of the events the crate emits, kept for the assertions.
#[derive(Clone, Default)]
struct Warnings {
    /// The fields of the events so far.
    entries: Arc<Mutex<Vec<String>>>,
    /// The ids of the spans, so that every span carries its own.
    spans: Arc<AtomicU64>,
}

impl Warnings {
    /// The fields of the events so far.
    fn captured(&self) -> Vec<String> {
        self.entries
            .lock()
            .expect("the lock of the warnings")
            .clone()
    }
}

impl tracing::Subscriber for Warnings {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(self.spans.fetch_add(1, Ordering::Relaxed) + 1)
    }

    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut visitor = Visitor(self.entries.clone());
        event.record(&mut visitor);
    }

    fn enter(&self, _: &tracing::span::Id) {}

    fn exit(&self, _: &tracing::span::Id) {}
}

/// The fields of one event, as strings.
struct Visitor(Arc<Mutex<Vec<String>>>);

impl tracing::field::Visit for Visitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .lock()
            .expect("the lock of the warnings")
            .push(format!("{}={value:?}", field.name()));
    }
}

/// A validator whose registry holds one crawler that reads its list from the
/// mocked endpoint.
async fn validator_for(server: &MockServer) -> (tempfile::TempDir, Arc<Validator>) {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let conf = directory.path().join("conf.d");
    std::fs::create_dir_all(&conf).expect("the override directory");
    std::fs::write(
        conf.join("testbot.yaml"),
        format!(
            "name: testbot\nua: \"TestBot\"\nvendor: \"Example\"\nparser: google\nurls:\n  - \"{}/ips.json\"\n",
            server.uri()
        ),
    )
    .expect("the definition");
    let validator = Validator::new(ValidatorOptions {
        root: directory.path().to_path_buf(),
        ..ValidatorOptions::default()
    })
    .expect("a validator");
    (directory, validator)
}

/// The failure of the refresh of one crawler is logged with the crawler and
/// the reason.
#[tokio::test]
async fn a_failed_refresh_of_one_bot_is_logged() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ips.json"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let (_directory, validator) = validator_for(&server).await;
    let warnings = Warnings::default();
    let _guard = tracing::subscriber::set_default(warnings.clone());
    let failure = validator.refresh_bot("testbot").await;
    drop(_guard);

    let failure = failure.expect("the refresh must fail");
    assert!(
        matches!(failure, RefreshFailure::Unreachable(_)),
        "{failure:?}"
    );

    let entries = warnings.captured();
    assert!(
        entries.iter().any(|entry| entry.contains("bot=testbot")),
        "{entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.contains("failure=unreachable")),
        "{entries:?}"
    );
    assert!(
        entries.iter().any(|entry| entry.contains("403")),
        "{entries:?}"
    );
}
