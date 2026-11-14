//! The classification of the source of a crawler, shared by the tests.
//!
//! A crawler depends on the outside world in three ways: a published list of
//! addresses, the reverse DNS of its vendor, or the prefixes its definition
//! states itself. A crawler may depend on more than one of them — the vendor
//! publishes an address space for the IPv6 addresses of its crawler and keeps
//! the reverse DNS of the IPv4 ones. The health check and the table of the
//! README name the sources the same way, so the classification has one owner.

use knownbots::Bot;

/// The kind of the source a crawler depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// A published address space, downloaded from the documents of the crawler.
    List,
    /// The reverse DNS of the vendor.
    Dns,
    /// The prefixes the crawler states itself.
    Static,
}

impl Source {
    /// The name of the kind, in the report and in the table of the README.
    pub fn name(self) -> &'static str {
        match self {
            Source::List => "list",
            Source::Dns => "dns",
            Source::Static => "static",
        }
    }
}

/// The sources a crawler depends on.
///
/// The set renders as the label of the crawler: the names of its sources,
/// joined with `+`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources(Vec<Source>);

impl Sources {
    /// The sources a crawler depends on.
    ///
    /// A crawler that names a published document depends on `list`, a crawler
    /// that declares a reverse DNS of its own and holds a domain depends on
    /// `dns`, and a crawler that names neither depends on `static`.
    pub fn of(bot: &Bot) -> Sources {
        let mut sources = Vec::new();
        if !bot.urls.is_empty() {
            sources.push(Source::List);
        }
        if bot.rdns && !bot.domains.is_empty() {
            sources.push(Source::Dns);
        }
        if sources.is_empty() {
            sources.push(Source::Static);
        }
        Sources(sources)
    }

    /// Whether the crawler depends on the source.
    pub fn contains(&self, source: &Source) -> bool {
        self.0.contains(source)
    }
}

impl<'a> IntoIterator for &'a Sources {
    type Item = Source;
    type IntoIter = std::iter::Copied<std::slice::Iter<'a, Source>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().copied()
    }
}

impl std::fmt::Display for Sources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, source) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str("+")?;
            }
            f.write_str(source.name())?;
        }
        Ok(())
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
