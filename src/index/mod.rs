//! The indexes of the registry.

pub(crate) mod prefix;
pub(crate) mod ua;

pub use ua::{contains_word, find_bot_by_ua};
