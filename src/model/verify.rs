//! The outcome of a verification.

use crate::model::bot::{Bot, BotKind};

/// The outcome of a verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyStatus {
    /// The marker matched and the address belongs to the bot.
    Verified,
    /// The marker matched but the address does not belong to the bot.
    Failed,
    /// The marker matched, the reverse DNS of the address decides and the cache
    /// of the bot is cold — the lookup is queued for a later request.
    Pending,
    /// The marker names no bot of the registry.
    Unknown,
}

/// The result of a verification.
///
/// Only [`VerifyStatus::Verified`] says that the request is the bot it claims to
/// be. Every other outcome means the request is not that bot.
///
/// The identity of the bot is only filled for a verified result: a request that
/// names a bot but is not verified keeps the path free of the copy of a name
/// that the host would throw away anyway.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct VerifyResult {
    /// The outcome of the verification.
    pub status: VerifyStatus,
    /// The id of the matched bot (`googlebot`), empty unless the request was
    /// verified.
    pub name: String,
    /// The organisation that owns the matched bot, empty unless the request was
    /// verified.
    pub vendor: String,
    /// The category of the matched bot.
    pub kind: BotKind,
}

impl VerifyResult {
    /// Whether the request is verified as the bot it claims to be.
    pub fn is_verified(&self) -> bool {
        self.status == VerifyStatus::Verified
    }

    /// The result of a request that names no bot of the registry.
    pub(crate) fn unknown() -> Self {
        // `String::new()` is heap-free; unknown is the common path.
        Self {
            status: VerifyStatus::Unknown,
            name: String::new(),
            vendor: String::new(),
            kind: BotKind::Unknown,
        }
    }

    /// The result of a request that named a bot but was not verified as it.
    pub(crate) fn non_verified(status: VerifyStatus, bot: &Bot) -> Self {
        // Failed / pending: the host discards the identity fields; skip clones.
        Self {
            status,
            name: String::new(),
            vendor: String::new(),
            kind: bot.kind,
        }
    }

    /// The result of a request that was verified as the bot it named.
    pub(crate) fn verified(bot: &Bot) -> Self {
        Self {
            status: VerifyStatus::Verified,
            name: bot.name.clone(),
            vendor: bot.vendor.clone(),
            kind: bot.kind,
        }
    }
}
