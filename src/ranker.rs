//! Card ranking — the 45s strength function.
//!
//! # Why 45s ranking is unusual
//!
//! In most trick-taking games, card strength is a fixed order (A > K > Q > J > 10 > ... > 2).
//! 45s has three layers of twist:
//!
//! 1. **Three "bowers" float to the top of trump:** the 5 of trump, J of trump,
//!    and Ace of Hearts (always). Ranked 5 > J > AH regardless of the trump suit.
//!
//! 2. **A of trump rank:** when trump is not Hearts, A-trump slots in at #4
//!    (below AH). When trump IS Hearts, AH is already #3 and serves both roles.
//!
//! 3. **Red/black asymmetry for number cards:** in red suits, number cards rank
//!    high-to-low (10 > 9 > ... > 2). In black suits, they rank low-to-high
//!    (2 > 3 > ... > 10). This is the oldest and weirdest inherited rule.
//!
//! # Strength encoding
//!
//! We return an `i32` where higher = stronger within a trick context. The
//! absolute numbers don't matter, only the ordering. We use distinct ranges:
//!
//! | Range     | Meaning                                                    |
//! |-----------|------------------------------------------------------------|
//! | 100–199   | Trump cards                                                |
//! | 1–49      | Non-trump (lead suit or otherwise — caller disambiguates)  |
//! | 0         | Shouldn't happen; means an unranked card                   |
//!
//! `TrickResolver` handles the lead-suit-vs-off-suit distinction above this
//! layer, so the strength function doesn't need to know which suit was led.

use crate::cards::{Card, Rank, Suit};

/// Is this card a trump, given the current trump suit?
///
/// Ace of Hearts is ALWAYS trump regardless of trump suit (it's one of the bowers).
pub fn is_trump(card: Card, trump: Suit) -> bool {
    if card.suit == Suit::Hearts && card.rank == Rank::Ace {
        return true;
    }
    card.suit == trump
}

/// Is this one of the three "top trumps" (bowers) that cannot be forced out
/// by a lower trump lead?
///
/// The top trumps, in descending order of strength:
/// - 5 of trump
/// - J of trump
/// - A of Hearts
///
/// When a player leads a *lower* trump, other players may withhold top trumps.
/// When a player leads a top trump, everyone must follow trump if they can.
pub fn is_top_trump(card: Card, trump: Suit) -> bool {
    if card.rank == Rank::Ace && card.suit == Suit::Hearts {
        return true;
    }
    if card.suit != trump {
        return false;
    }
    matches!(card.rank, Rank::Five | Rank::Jack)
}

/// Strength of a card given the trump suit. Higher = stronger.
///
/// Comparing strengths only makes sense within a single trick between cards
/// that share trump status or lead suit. `TrickResolver` handles that framing.
pub fn strength(card: Card, trump: Suit) -> i32 {
    if is_trump(card, trump) {
        trump_strength(card, trump)
    } else {
        non_trump_strength(card)
    }
}

// -----------------------------------------------------------------------------
// Private helpers
// -----------------------------------------------------------------------------

/// Trump strength values (100–199). The top four positions are hardcoded
/// bowers; everything else is indexed by suit-specific number order.
fn trump_strength(card: Card, trump: Suit) -> i32 {
    // 1st: 5 of trump
    if card.rank == Rank::Five && card.suit == trump {
        return 199;
    }
    // 2nd: J of trump
    if card.rank == Rank::Jack && card.suit == trump {
        return 198;
    }
    // 3rd: A of Hearts (always, regardless of trump suit)
    if card.rank == Rank::Ace && card.suit == Suit::Hearts {
        return 197;
    }
    // 4th: A of trump (only reached when trump != Hearts; otherwise AH took slot 3)
    if card.rank == Rank::Ace && card.suit == trump {
        return 196;
    }
    // King and Queen of trump get fixed high values above the number cards.
    if card.rank == Rank::King {
        return 113;
    }
    if card.rank == Rank::Queen {
        return 112;
    }
    // Remaining trump number cards (2, 3, 4, 6, 7, 8, 9, 10) ranked by suit order.
    100 + number_rank_order(card.rank, trump)
}

/// Non-trump strength (1–13). The lead-suit bonus is applied by TrickResolver,
/// not here, since this function doesn't know what was led.
fn non_trump_strength(card: Card) -> i32 {
    if card.suit.is_red() {
        red_non_trump_strength(card)
    } else {
        black_non_trump_strength(card)
    }
}

/// Red non-trump rank (Hearts/Diamonds): K Q J 10 9 ... 2 A.
/// - Hearts: A is always trump so never reaches here.
/// - Diamonds: A ranks lowest when Diamonds are not trump.
fn red_non_trump_strength(card: Card) -> i32 {
    match card.rank {
        Rank::King => 13,
        Rank::Queen => 12,
        Rank::Jack => 11,
        Rank::Ten => 10,
        Rank::Nine => 9,
        Rank::Eight => 8,
        Rank::Seven => 7,
        Rank::Six => 6,
        Rank::Five => 5,
        Rank::Four => 4,
        Rank::Three => 3,
        Rank::Two => 2,
        Rank::Ace => 1,
    }
}

/// Black non-trump rank (Clubs/Spades): K Q J A 2 3 4 5 6 7 8 9 10.
/// Numbers rank low-to-high — opposite of red. See the module comment for
/// historical context.
fn black_non_trump_strength(card: Card) -> i32 {
    match card.rank {
        Rank::King => 13,
        Rank::Queen => 12,
        Rank::Jack => 11,
        Rank::Ace => 10,
        Rank::Two => 1,
        Rank::Three => 2,
        Rank::Four => 3,
        Rank::Five => 4,
        Rank::Six => 5,
        Rank::Seven => 6,
        Rank::Eight => 7,
        Rank::Nine => 8,
        Rank::Ten => 9,
    }
}

/// Rank within trump for number cards (excluding 5, J, A which are handled above).
/// Returns 1–8 (8 highest).
///
/// - Red trump: 10 9 8 7 6 4 3 2  (high-to-low)
/// - Black trump: 2 3 4 6 7 8 9 10 (low-to-high)
fn number_rank_order(rank: Rank, trump: Suit) -> i32 {
    if trump.is_red() {
        match rank {
            Rank::Ten => 8, Rank::Nine => 7, Rank::Eight => 6, Rank::Seven => 5,
            Rank::Six => 4, Rank::Four => 3, Rank::Three => 2, Rank::Two => 1,
            _ => 0,
        }
    } else {
        match rank {
            Rank::Two => 8, Rank::Three => 7, Rank::Four => 6, Rank::Six => 5,
            Rank::Seven => 4, Rank::Eight => 3, Rank::Nine => 2, Rank::Ten => 1,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::{Card, Rank, Suit};

    fn c(suit: Suit, rank: Rank) -> Card {
        Card::new(suit, rank)
    }

    #[test]
    fn ace_of_hearts_is_always_trump() {
        for trump in Suit::ALL {
            assert!(is_trump(c(Suit::Hearts, Rank::Ace), trump),
                    "AH should be trump when trump = {:?}", trump);
        }
    }

    #[test]
    fn trump_suit_membership() {
        // When Spades are trump, Spades are trump; others (except AH) are not.
        let trump = Suit::Spades;
        assert!(is_trump(c(Suit::Spades, Rank::Two), trump));
        assert!(is_trump(c(Suit::Spades, Rank::King), trump));
        assert!(!is_trump(c(Suit::Clubs, Rank::Ace), trump));
        assert!(!is_trump(c(Suit::Diamonds, Rank::Ace), trump));
        assert!(is_trump(c(Suit::Hearts, Rank::Ace), trump)); // AH bower
        assert!(!is_trump(c(Suit::Hearts, Rank::King), trump));
    }

    #[test]
    fn top_trumps_identification() {
        let trump = Suit::Clubs;
        assert!(is_top_trump(c(Suit::Clubs, Rank::Five), trump));
        assert!(is_top_trump(c(Suit::Clubs, Rank::Jack), trump));
        assert!(is_top_trump(c(Suit::Hearts, Rank::Ace), trump));
        assert!(!is_top_trump(c(Suit::Clubs, Rank::Ace), trump));
        assert!(!is_top_trump(c(Suit::Clubs, Rank::King), trump));
        assert!(!is_top_trump(c(Suit::Hearts, Rank::Five), trump));
    }

    #[test]
    fn bower_ordering_when_trump_not_hearts() {
        // Trump = Spades. Ordering 5S > JS > AH > AS > KS > QS.
        let trump = Suit::Spades;
        let s_5 = strength(c(Suit::Spades, Rank::Five), trump);
        let s_j = strength(c(Suit::Spades, Rank::Jack), trump);
        let ah = strength(c(Suit::Hearts, Rank::Ace), trump);
        let as_ = strength(c(Suit::Spades, Rank::Ace), trump);
        let ks = strength(c(Suit::Spades, Rank::King), trump);
        let qs = strength(c(Suit::Spades, Rank::Queen), trump);
        assert!(s_5 > s_j && s_j > ah && ah > as_ && as_ > ks && ks > qs,
                "got 5S={s_5} JS={s_j} AH={ah} AS={as_} KS={ks} QS={qs}");
    }

    #[test]
    fn bower_ordering_when_trump_is_hearts() {
        // Trump = Hearts. AH is bower #3, and there is no separate "A of trump" slot.
        let trump = Suit::Hearts;
        let s_5 = strength(c(Suit::Hearts, Rank::Five), trump);
        let s_j = strength(c(Suit::Hearts, Rank::Jack), trump);
        let ah = strength(c(Suit::Hearts, Rank::Ace), trump);
        let kh = strength(c(Suit::Hearts, Rank::King), trump);
        assert!(s_5 > s_j && s_j > ah && ah > kh);
    }

    #[test]
    fn black_trump_numbers_rank_low_to_high() {
        // In Clubs trump, 2C should beat 10C.
        let trump = Suit::Clubs;
        let two = strength(c(Suit::Clubs, Rank::Two), trump);
        let ten = strength(c(Suit::Clubs, Rank::Ten), trump);
        assert!(two > ten, "2C={two} should beat 10C={ten}");
    }

    #[test]
    fn red_trump_numbers_rank_high_to_low() {
        // In Diamonds trump, 10D should beat 2D.
        let trump = Suit::Diamonds;
        let ten = strength(c(Suit::Diamonds, Rank::Ten), trump);
        let two = strength(c(Suit::Diamonds, Rank::Two), trump);
        assert!(ten > two, "10D={ten} should beat 2D={two}");
    }

    #[test]
    fn ace_of_diamonds_lowest_when_non_trump() {
        // Diamonds non-trump (trump = Clubs). AD should rank below 2D.
        let trump = Suit::Clubs;
        let ad = strength(c(Suit::Diamonds, Rank::Ace), trump);
        let d2 = strength(c(Suit::Diamonds, Rank::Two), trump);
        assert!(d2 > ad, "2D={d2} should beat AD={ad}");
    }

    #[test]
    fn black_non_trump_ace_ranks_fourth() {
        // Clubs non-trump (trump = Hearts). Order: KC > QC > JC > AC > numbers.
        let trump = Suit::Hearts;
        let kc = strength(c(Suit::Clubs, Rank::King), trump);
        let qc = strength(c(Suit::Clubs, Rank::Queen), trump);
        let jc = strength(c(Suit::Clubs, Rank::Jack), trump);
        let ac = strength(c(Suit::Clubs, Rank::Ace), trump);
        let c10 = strength(c(Suit::Clubs, Rank::Ten), trump);
        let c2 = strength(c(Suit::Clubs, Rank::Two), trump);
        assert!(kc > qc && qc > jc && jc > ac && ac > c10 && c10 > c2);
    }
}
