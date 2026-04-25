//! Card, Suit, Rank — the primitive types of a 45s game.
//!
//! # Design notes
//!
//! `Card` is `Copy` and 2 bytes total. Games deal out 52 cards thousands of times
//! per second during self-play; copying is much cheaper than reference-counting.
//!
//! Suits and ranks are explicit enums rather than integers or strings. Strings
//! would be slow (hash lookups on a hot path) and integers would let us write
//! nonsense like `Rank::Seven * 2`. Enums force the type checker to help us.
//!
//! The numeric `#[repr(u8)]` discriminants are chosen so that `rank as u8` gives
//! the natural "face value" (Ace is the ambiguous one — 14 here, because in 45s
//! it matters which trump-adjacent role it plays, not its raw numeric value).

use std::fmt;
use std::str::FromStr;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[repr(u8)]
pub enum Suit {
    Clubs = 0,
    Diamonds = 1,
    Hearts = 2,
    Spades = 3,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];

    /// Red suits rank their non-trump number cards high-to-low (normal).
    /// Black suits rank their non-trump number cards low-to-high (a 45s quirk
    /// inherited from the 25/45/110 family of Irish games).
    pub fn is_red(self) -> bool {
        matches!(self, Suit::Hearts | Suit::Diamonds)
    }

    pub fn is_black(self) -> bool {
        !self.is_red()
    }

    pub fn code(self) -> char {
        match self {
            Suit::Clubs => 'C',
            Suit::Diamonds => 'D',
            Suit::Hearts => 'H',
            Suit::Spades => 'S',
        }
    }
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[repr(u8)]
pub enum Rank {
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
    Nine = 9,
    Ten = 10,
    Jack = 11,
    Queen = 12,
    King = 13,
    Ace = 14,
}

impl Rank {
    pub const ALL: [Rank; 13] = [
        Rank::Two, Rank::Three, Rank::Four, Rank::Five, Rank::Six, Rank::Seven,
        Rank::Eight, Rank::Nine, Rank::Ten, Rank::Jack, Rank::Queen, Rank::King, Rank::Ace,
    ];

    /// Two-character code for display: "2".."10", "J", "Q", "K", "A".
    pub fn code(self) -> &'static str {
        match self {
            Rank::Two => "2", Rank::Three => "3", Rank::Four => "4",
            Rank::Five => "5", Rank::Six => "6", Rank::Seven => "7",
            Rank::Eight => "8", Rank::Nine => "9", Rank::Ten => "10",
            Rank::Jack => "J", Rank::Queen => "Q", Rank::King => "K",
            Rank::Ace => "A",
        }
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

impl Card {
    pub const fn new(suit: Suit, rank: Rank) -> Self {
        Card { suit, rank }
    }

    /// Human code: "AH" (Ace of Hearts), "10D" (Ten of Diamonds), "5S" (Five of Spades).
    /// Matches the wire format used by the PHP game engine.
    pub fn code(self) -> String {
        format!("{}{}", self.rank.code(), self.suit.code())
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

/// Parse errors mirror the three ways a code like "AH" can be malformed.
#[derive(Debug, PartialEq, Eq)]
pub enum CardParseError {
    TooShort,
    BadRank,
    BadSuit,
}

impl fmt::Display for CardParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CardParseError::TooShort => f.write_str("card code too short (need rank+suit)"),
            CardParseError::BadRank => f.write_str("unknown rank"),
            CardParseError::BadSuit => f.write_str("unknown suit"),
        }
    }
}

impl std::error::Error for CardParseError {}

impl FromStr for Card {
    type Err = CardParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_uppercase();
        if s.len() < 2 {
            return Err(CardParseError::TooShort);
        }
        let (rank_part, suit_part) = s.split_at(s.len() - 1);
        let rank = match rank_part {
            "2" => Rank::Two, "3" => Rank::Three, "4" => Rank::Four,
            "5" => Rank::Five, "6" => Rank::Six, "7" => Rank::Seven,
            "8" => Rank::Eight, "9" => Rank::Nine, "10" => Rank::Ten,
            "J" => Rank::Jack, "Q" => Rank::Queen, "K" => Rank::King,
            "A" => Rank::Ace,
            _ => return Err(CardParseError::BadRank),
        };
        let suit = match suit_part {
            "C" => Suit::Clubs, "D" => Suit::Diamonds,
            "H" => Suit::Hearts, "S" => Suit::Spades,
            _ => return Err(CardParseError::BadSuit),
        };
        Ok(Card { suit, rank })
    }
}

/// The 52-card standard deck in a stable canonical order
/// (Clubs 2..A, Diamonds 2..A, Hearts 2..A, Spades 2..A).
///
/// Callers that want randomized deals should shuffle the result with a seeded RNG.
/// The canonical order is itself useful for reproducible tests and for hashing.
pub fn standard_deck() -> Vec<Card> {
    let mut deck = Vec::with_capacity(52);
    for suit in Suit::ALL {
        for rank in Rank::ALL {
            deck.push(Card::new(suit, rank));
        }
    }
    deck
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deck_has_52_unique_cards() {
        let deck = standard_deck();
        assert_eq!(deck.len(), 52);
        let unique: std::collections::HashSet<_> = deck.iter().collect();
        assert_eq!(unique.len(), 52);
    }

    #[test]
    fn card_roundtrip() {
        for card in standard_deck() {
            let code = card.code();
            let parsed: Card = code.parse().unwrap();
            assert_eq!(parsed, card, "roundtrip failed for {}", code);
        }
    }

    #[test]
    fn parse_rejects_bad_input() {
        assert_eq!("".parse::<Card>(), Err(CardParseError::TooShort));
        assert_eq!("X".parse::<Card>(), Err(CardParseError::TooShort));
        assert_eq!("1H".parse::<Card>(), Err(CardParseError::BadRank));
        assert_eq!("AZ".parse::<Card>(), Err(CardParseError::BadSuit));
    }

    #[test]
    fn parse_is_case_insensitive_and_trims() {
        let c: Card = "  ah  ".parse().unwrap();
        assert_eq!(c, Card::new(Suit::Hearts, Rank::Ace));
        let c: Card = "10d".parse().unwrap();
        assert_eq!(c, Card::new(Suit::Diamonds, Rank::Ten));
    }

    #[test]
    fn suit_colour_classification() {
        assert!(Suit::Hearts.is_red());
        assert!(Suit::Diamonds.is_red());
        assert!(Suit::Clubs.is_black());
        assert!(Suit::Spades.is_black());
    }
}
