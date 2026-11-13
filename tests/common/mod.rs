//! The classification of the source of a crawler, shared by the tests.
//!
//! A crawler depends on the outside world in three ways: a published list of
//! addresses, the reverse DNS of its vendor, or the prefixes its definition
//! states itself. The health check and the table of the README name the three
//! the same way, so the classification has one owner.

use knownbots::Bot;

/// The kind of the source a crawler depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A published address space, downloaded from the documents of the crawler.
    List,
    /// The reverse DNS of the vendor.
    Dns,
    /// The prefixes the crawler states itself.
    Static,
}

impl Source {
    /// The kind of the source a crawler depends on.
    pub fn of(bot: &Bot) -> Self {
        if !bot.urls.is_empty() {
            Source::List
        } else if bot.rdns {
            Source::Dns
        } else {
            Source::Static
        }
    }

    /// The name of the kind, in the report and in the table of the README.
    pub fn name(self) -> &'static str {
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

impl serde::Serialize for Source {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}
