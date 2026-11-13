//! The check against the endpoints of the vendors.
//!
//! The endpoints of the vendors are on the public internet, so the test is
//! ignored: the suite of the crate never depends on the uptime of a vendor.
//! Run it with:
//!
//! ```sh
//! cargo test --all-features --test live -- --ignored
//! ```

#![cfg(feature = "full")]

use knownbots::{RefreshFailure, Validator, ValidatorOptions};

/// The crawlers whose lists the test reads.
const CRAWLERS: &[&str] = &[
    "googlebot",
    "bingbot",
    "uptimerobot",
    "ahrefsbot",
    "amazonbot",
    "claudebot",
];

#[tokio::test]
#[ignore = "the endpoints of the vendors are on the public internet"]
async fn the_lists_of_the_vendors_are_read() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let validator = Validator::new(ValidatorOptions {
        root: directory.path().to_path_buf(),
        ..ValidatorOptions::default()
    })
    .expect("a validator");

    let mut failures = Vec::new();
    for crawler in CRAWLERS {
        match validator.refresh_bot(crawler).await {
            None => assert!(
                validator.covered_ip(crawler).is_some(),
                "{crawler} holds no prefix"
            ),
            // A vendor that refuses the request is a problem of the
            // environment, not of the crate.
            Some(RefreshFailure::Unreachable(reason)) => {
                eprintln!("SKIP {crawler}: the endpoint refused the request ({reason})");
            }
            Some(RefreshFailure::Broken(reason)) => {
                failures.push(format!("{crawler}: {reason}"));
            }
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
