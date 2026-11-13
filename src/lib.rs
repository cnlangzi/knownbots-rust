//! `knownbots` verifies whether a request that claims to be a known crawler is one.
//!
//! A crawler proves its identity in two steps: it names itself in the
//! `User-Agent` header, and it comes from an address the vendor published for
//! that crawler. The crate checks both steps. A request that names a known
//! crawler but comes from an address the vendor does not publish is not that
//! crawler.
//!
//! The check is synchronous and allocation-light: the `User-Agent` is matched
//! against the registry of the crate, and the address is looked up in the
//! prefixes of the matched entry. A reverse DNS lookup never happens on the
//! request path — a cold cache queues the lookup and the request fails closed.
//!
//! # Dependencies
//!
//! ```sh
//! cargo add knownbots --features full
//! ```
//!
//! The `full` feature compiles in the network parts of the crate: the download
//! of the published lists and the reverse DNS lookups. A host that only matches
//! the `User-Agent` of a request against the registry of the crate depends on it
//! without the feature:
//!
//! ```sh
//! cargo add knownbots
//! ```
//!
//! # Usage
//!
//! [`Validator`] is the handle of the crate, [`ValidatorOptions`] is its
//! configuration, and the result of a verification carries the outcome in
//! [`VerifyStatus`].
//!
//! ```
//! use std::net::IpAddr;
//!
//! use knownbots::{Validator, ValidatorOptions, VerifyStatus};
//!
//! let directory = tempfile::tempdir().unwrap();
//! let validator = Validator::new(ValidatorOptions {
//!     root: directory.path().to_path_buf(),
//!     ..ValidatorOptions::default()
//! })
//! .unwrap();
//!
//! // The marker of a known crawler, from an address the vendor does not
//! // publish: the request is not that crawler.
//! let result = validator.verify(
//!     "Mozilla/5.0 (compatible; Googlebot/2.1)",
//!     "203.0.113.7".parse::<IpAddr>().unwrap(),
//! );
//! assert_eq!(result.status, VerifyStatus::Failed);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod config;
mod engine;
mod error;
mod index;
mod model;
#[cfg(feature = "full")]
mod worker;

pub mod feed;
#[cfg(feature = "full")]
pub mod http;

pub use config::{DEFAULT_FAIL_LIMIT, DEFAULT_REFRESH_INTERVAL, DEFAULT_ROOT, ValidatorOptions};
pub use engine::Validator;
pub use error::{ConfigError, ParseError, RefreshFailure};
pub use feed::{is_known_parser, parse};
#[cfg(feature = "full")]
pub use http::{HttpClient, reqwest};
pub use index::{contains_word, find_bot_by_ua};
pub use model::bot::{Bot, BotConfigError, BotKind};
pub use model::rdns::match_domain;
pub use model::refresh::{BotRefreshState, BotRefreshStatus};
pub use model::registry::{EMBEDDED_CONFIGS, load_bots, load_bots_with_errors, vendor_for};
pub use model::verify::{VerifyResult, VerifyStatus};
