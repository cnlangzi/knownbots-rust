//! The health of the address space of a bot.

use crate::error::RefreshFailure;
use crate::model::bot::BotKind;

/// The state of the address space of a bot.
///
/// The state is derived from the state the validator already keeps — no extra
/// bookkeeping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotRefreshState {
    /// The bot can be verified against a loaded address space.
    Ready,
    /// A bot that is backed by a document whose list was never downloaded.
    Cold,
    /// The last refresh failed.
    Failed,
}

/// The health of the address space of a bot.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BotRefreshStatus {
    /// The id of the bot (`googlebot`).
    pub name: String,
    /// The organisation that owns the bot.
    pub vendor: String,
    /// The category of the bot.
    pub kind: BotKind,
    /// The prefixes currently loaded (downloaded and stated).
    pub prefixes: usize,
    /// The derived state.
    pub state: BotRefreshState,
    /// The failure of the last refresh, when the state is
    /// [`BotRefreshState::Failed`].
    pub error: Option<RefreshFailure>,
}
