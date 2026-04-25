//! Typed engine errors.
//!
//! The PHP runtime rejects invalid moves with string codes ("not_your_turn",
//! "illegal_card", etc.). Mirroring them here means:
//! - Tests can assert on specific reasons without string-matching.
//! - When we eventually cross-check against PHP traces (golden corpus in C5),
//!   we have a clean 1:1 mapping.
//!
//! Add a variant here when the state machine needs a new rejection reason,
//! and only then.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// A move was submitted by a seat that isn't on turn.
    NotYourTurn,
    /// The move type doesn't match the current phase (e.g., bid during play).
    WrongPhase,
    /// A played card isn't legal under the 45s follow-suit/trump rules.
    IllegalCard,
    /// A played card isn't actually in the player's hand.
    CardNotInHand,
    /// Bid value isn't one of {15, 20, 25, 30, 60} or doesn't exceed current bid.
    InvalidBid,
    /// Discard count violates the rules (e.g., >3 in 6-player, or non-kept trump).
    InvalidDiscard,
    /// Trump declaration happened in a seat/phase where it isn't allowed.
    InvalidTrumpDeclare,
    /// Catch-all for internal invariant violations that should never happen.
    /// Use sparingly; prefer a typed variant.
    Invariant(&'static str),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::NotYourTurn => f.write_str("not your turn"),
            EngineError::WrongPhase => f.write_str("wrong phase for this move"),
            EngineError::IllegalCard => f.write_str("illegal card under 45s rules"),
            EngineError::CardNotInHand => f.write_str("card not in hand"),
            EngineError::InvalidBid => f.write_str("invalid bid"),
            EngineError::InvalidDiscard => f.write_str("invalid discard"),
            EngineError::InvalidTrumpDeclare => f.write_str("invalid trump declaration"),
            EngineError::Invariant(s) => write!(f, "engine invariant violated: {}", s),
        }
    }
}

impl std::error::Error for EngineError {}
