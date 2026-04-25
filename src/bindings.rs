//! Python bindings (PyO3) — Stage 0 surface.
//!
//! Design choice: the Python-facing API uses **stringly-typed cards** ("AH",
//! "10D") and **string phase/suit labels**. This keeps the Python side simple
//! and avoids exposing Rust enums across the FFI boundary while we're still
//! iterating on the engine shape.
//!
//! Downside: parsing strings on every move is slower than an integer-encoded
//! API. If Stage 4 self-play profiling shows the boundary is the bottleneck,
//! we'll add an `int`-based API alongside. Until then, clarity wins.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::cards::{Card, Suit};
use crate::error::EngineError;
use crate::state::{Action, GameConfig, GameState, Move, Phase, Seat};

// --- EngineError → PyErr ---

impl From<EngineError> for PyErr {
    fn from(err: EngineError) -> PyErr {
        PyValueError::new_err(err.to_string())
    }
}

fn parse_suit(s: &str) -> PyResult<Suit> {
    let t = s.trim().to_uppercase();
    match t.as_str() {
        "C" | "CLUBS" => Ok(Suit::Clubs),
        "D" | "DIAMONDS" => Ok(Suit::Diamonds),
        "H" | "HEARTS" => Ok(Suit::Hearts),
        "S" | "SPADES" => Ok(Suit::Spades),
        _ => Err(PyValueError::new_err(format!("unknown suit: {}", s))),
    }
}

fn parse_card(s: &str) -> PyResult<Card> {
    s.parse::<Card>()
        .map_err(|e| PyValueError::new_err(format!("bad card '{}': {}", s, e)))
}

fn parse_cards(codes: &[String]) -> PyResult<Vec<Card>> {
    codes.iter().map(|s| parse_card(s)).collect()
}

fn cards_to_codes(cards: &[Card]) -> Vec<String> {
    cards.iter().map(|c| c.code()).collect()
}

fn phase_label(p: Phase) -> &'static str {
    match p {
        Phase::Bidding => "bidding",
        Phase::DeclareTrump => "declare_trump",
        Phase::Discard => "discard",
        Phase::DealerExtraDraw => "dealer_extra_draw",
        Phase::Play => "play",
        Phase::HandComplete => "hand_complete",
        Phase::GameOver => "game_over",
    }
}

// --- PyGameState ---

#[pyclass(name = "GameState")]
pub struct PyGameState {
    inner: GameState,
}

#[pymethods]
impl PyGameState {
    /// Create a new game with the first hand dealt.
    ///
    /// - `seed` — seeds `ChaCha8Rng` for the shuffle (reproducible).
    /// - `num_players` — 4 or 6.
    /// - `dealer` — starting dealer seat.
    /// - `target_score` — Chartrand default 120; classic 45.
    /// - `enable_30_for_60` — Newfoundland "30-for-60" bid availability.
    #[new]
    #[pyo3(signature = (seed, num_players=4, dealer=0, target_score=120, enable_30_for_60=true))]
    fn new(
        seed: u64,
        num_players: u8,
        dealer: u8,
        target_score: i32,
        enable_30_for_60: bool,
    ) -> PyResult<Self> {
        if num_players != 4 && num_players != 6 {
            return Err(PyValueError::new_err("num_players must be 4 or 6"));
        }
        if dealer >= num_players {
            return Err(PyValueError::new_err("dealer out of range"));
        }
        let config = GameConfig { num_players, target_score, enable_30_for_60 };
        Ok(PyGameState {
            inner: GameState::new_hand(seed, config, dealer),
        })
    }

    // --- Accessors ---

    fn phase(&self) -> &'static str {
        phase_label(self.inner.phase())
    }

    fn to_act(&self) -> Option<Seat> {
        self.inner.to_act()
    }

    fn dealer(&self) -> Seat {
        self.inner.dealer()
    }

    fn hand(&self, seat: Seat) -> PyResult<Vec<String>> {
        if seat >= self.inner.num_players() {
            return Err(PyValueError::new_err("seat out of range"));
        }
        Ok(cards_to_codes(self.inner.hand(seat)))
    }

    fn kitty(&self) -> Vec<String> {
        cards_to_codes(self.inner.kitty())
    }

    fn deck_remainder(&self) -> Vec<String> {
        cards_to_codes(self.inner.deck_remainder())
    }

    fn discarded(&self) -> Vec<String> {
        cards_to_codes(self.inner.discarded())
    }

    fn trump(&self) -> Option<char> {
        self.inner.trump().map(|s| s.code())
    }

    fn current_bid(&self) -> Option<(Seat, u8)> {
        self.inner.current_bid().map(|b| (b.seat, b.amount))
    }

    fn winning_bid(&self) -> Option<(Seat, u8)> {
        self.inner.winning_bid().map(|b| (b.seat, b.amount))
    }

    fn is_30_for_60(&self) -> bool {
        self.inner.is_30_for_60()
    }

    fn bidder(&self) -> Option<Seat> {
        self.inner.bidder()
    }

    fn scores(&self) -> [i32; 2] {
        self.inner.scores()
    }

    /// Sets tracked for the "3 sets ⇒ lose" rule. Returned as a (team0, team1)
    /// tuple (not `[u8; 2]`, which PyO3 would encode as `bytes`).
    fn sets(&self) -> (u8, u8) {
        let s = self.inner.sets();
        (s[0], s[1])
    }

    fn hand_points(&self) -> [i32; 2] {
        self.inner.hand_points()
    }

    fn bid_made(&self) -> Option<bool> {
        self.inner.bid_made()
    }

    fn winner(&self) -> Option<u8> {
        self.inner.winner()
    }

    fn hands_played(&self) -> u32 {
        self.inner.hands_played()
    }

    fn current_trick(&self) -> Vec<(Seat, String)> {
        self.inner
            .current_trick()
            .iter()
            .map(|(s, c)| (*s, c.code()))
            .collect()
    }

    /// Completed tricks as `[(leader_seat, winner_seat, [(seat, card_code), ...]), ...]`.
    fn completed_tricks(&self) -> Vec<(Seat, Seat, Vec<(Seat, String)>)> {
        self.inner
            .completed_tricks()
            .iter()
            .map(|t| {
                let plays: Vec<(Seat, String)> =
                    t.plays.iter().map(|(s, c)| (*s, c.code())).collect();
                (t.leader, t.winner, plays)
            })
            .collect()
    }

    // --- Moves ---

    fn bid(&mut self, seat: Seat, amount: u8) -> PyResult<()> {
        self.inner.apply(Move { seat, action: Action::Bid { amount } })?;
        Ok(())
    }

    fn declare_trump(&mut self, seat: Seat, suit: &str) -> PyResult<()> {
        let s = parse_suit(suit)?;
        self.inner.apply(Move { seat, action: Action::DeclareTrump(s) })?;
        Ok(())
    }

    fn discard(&mut self, seat: Seat, cards: Vec<String>) -> PyResult<()> {
        let parsed = parse_cards(&cards)?;
        self.inner.apply(Move { seat, action: Action::Discard(parsed) })?;
        Ok(())
    }

    fn play(&mut self, seat: Seat, card: &str) -> PyResult<()> {
        let c = parse_card(card)?;
        self.inner.apply(Move { seat, action: Action::Play(c) })?;
        Ok(())
    }

    fn next_hand(&mut self, seed: u64) -> PyResult<()> {
        self.inner.next_hand(seed)?;
        Ok(())
    }

    // --- Legal-move helpers ---

    /// Bid amounts (0 means pass) that are legal for `seat` in the current state.
    /// Returns empty list if seat is not to-act in the Bidding phase.
    fn legal_bids(&self, seat: Seat) -> Vec<u8> {
        self.inner
            .legal_actions(seat)
            .into_iter()
            .filter_map(|a| if let Action::Bid { amount } = a { Some(amount) } else { None })
            .collect()
    }

    /// Legal card codes to play for `seat` in the current state.
    /// Returns empty list if seat is not to-act in the Play phase.
    fn legal_plays(&self, seat: Seat) -> Vec<String> {
        self.inner
            .legal_actions(seat)
            .into_iter()
            .filter_map(|a| if let Action::Play(c) = a { Some(c.code()) } else { None })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "GameState(phase={}, to_act={:?}, scores={:?}, sets={:?}, hands_played={})",
            phase_label(self.inner.phase()),
            self.inner.to_act(),
            self.inner.scores(),
            self.inner.sets(),
            self.inner.hands_played(),
        )
    }
}

/// Register the PyGameState class on the Python module. Called from lib.rs.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyGameState>()?;
    Ok(())
}
