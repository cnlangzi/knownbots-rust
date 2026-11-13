//! The errors of the crate.

/// Why the address space of a bot could not be refreshed.
///
/// The split is load-bearing: [`RefreshFailure::Unreachable`] is a problem of the
/// environment — the vendor refused the request, the network was down, the
/// endpoint answered with something that is not the list. [`RefreshFailure::Broken`]
/// points at this crate: the list arrived in a shape the crate mis-reads, or the
/// transfer failed in a way that points at the client of the crate.
///
/// A host that watches the lists tells the two apart, so that an outage of a
/// vendor does not read as a regression of the crate.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RefreshFailure {
    /// The list could not be fetched, or the body that answered was not the list.
    #[error("unreachable: {0}")]
    Unreachable(String),
    /// The list was fetched but is unusable, or the transfer failed in a way that
    /// points at the client of this crate.
    #[error("broken: {0}")]
    Broken(String),
}

/// Why a body could not be turned into prefixes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// The body is not a document of the kind the parser expects — the endpoint
    /// served something else.
    #[error("the body is not the list: {0}")]
    NotADocument(String),
    /// The body is a document of the expected kind, but its shape changed.
    #[error("the shape of the list changed: {0}")]
    UnexpectedShape(String),
}

/// A problem that keeps the crate from being built.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The registry of the bots could not be loaded.
    #[error("the bot registry could not be loaded: {0}")]
    Registry(String),
    /// The directory that holds the caches could not be prepared.
    #[error("the cache directory could not be prepared: {0}")]
    Io(String),
    /// The HTTP client could not be built.
    #[error("the http client could not be built: {0}")]
    Http(String),
}
