#![doc = include_str!("../README.md")]
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
