//! The lists of the addresses of the bots.
//!
//! A list is a document a vendor publishes. The feed turns such a document into
//! prefixes and — with the `full` feature — downloads the documents and keeps
//! them fresh.

mod parsers;

#[cfg(feature = "full")]
mod transport;

pub use parsers::{is_known_parser, parse};

#[cfg(feature = "full")]
pub use transport::{Downloaded, Transport};
