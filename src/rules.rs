//! Trick resolution and legal-move validation.
//!
//! This is the rule-logic layer — no state, no mutation, just pure functions
//! that take a hand/trick/trump and return "who won" or "is this legal".
//! The state machine in `state.rs` (Checkpoint C2) calls into these.
//!
//! # The five legal-play rules
//!
//! 1. **Leading:** anything is legal.
//! 2. **Trump can always be played** instead of following suit ("reneging via trump").
//! 3. **If trump was led and you have trump, you must play trump** — UNLESS your only
//!    trump is a "top trump" (5, J, AH). Top trumps cannot be *forced out* by a lower
//!    trump lead.
//! 4. **If a non-trump suit was led and you have cards of that suit, you must follow.**
//!    (Trump is the only legal non-following play — rule 2.)
//! 5. **If the lead suit is not in your hand, anything is legal** (you're void).

use crate::cards::{Card, Suit};
use crate::ranker::{is_top_trump, is_trump, strength};

/// Determine which seat wins a completed trick.
///
/// `plays` is indexed by play order (`plays[0]` is the lead card). The winning
/// index corresponds to the position within `plays`, not to an absolute seat number.
/// The caller (state machine) translates index → seat.
///
/// Invariant: `plays` must be non-empty.
///
/// # Lead-suit semantics
///
/// If the lead card is trump (including AH when trump is not Hearts), the
/// "lead suit" for comparison purposes is the trump suit, NOT the lead card's
/// face suit. This matches the PHP engine: a trump-led trick is resolved with
/// trump as the lead.
pub fn winning_index(plays: &[Card], trump: Suit) -> usize {
    assert!(!plays.is_empty(), "winning_index called with empty trick");
    let lead_card = plays[0];
    let lead_suit = if is_trump(lead_card, trump) { trump } else { lead_card.suit };

    let mut best = 0usize;
    for i in 1..plays.len() {
        if beats(plays[i], plays[best], lead_suit, trump) {
            best = i;
        }
    }
    best
}

/// Does `candidate` beat `current_best` given the lead suit and trump?
///
/// Ranking layers, in order of dominance:
/// 1. A trump beats a non-trump (always).
/// 2. Among non-trumps, a lead-suit card beats an off-suit card.
/// 3. Within the same tier, higher `strength()` wins.
fn beats(candidate: Card, current_best: Card, lead_suit: Suit, trump: Suit) -> bool {
    let cand_trump = is_trump(candidate, trump);
    let best_trump = is_trump(current_best, trump);

    if cand_trump && !best_trump {
        return true;
    }
    if !cand_trump && best_trump {
        return false;
    }

    // Both trump OR both non-trump.
    if !cand_trump && !best_trump {
        let cand_lead = candidate.suit == lead_suit;
        let best_lead = current_best.suit == lead_suit;
        if cand_lead && !best_lead {
            return true;
        }
        if !cand_lead && best_lead {
            return false;
        }
        // Both off-suit non-trump: neither can win a trick, but we still need a
        // deterministic comparison; fall through to strength comparison (matches PHP).
    }

    strength(candidate, trump) > strength(current_best, trump)
}

/// Check whether `play` is a legal move given the current trick context.
///
/// `lead` is `None` when the player is leading the trick (first play).
/// Otherwise `lead` holds the actual lead card (suit + rank both matter, because
/// the AH-as-lead case and "lead was a top trump" case both need the full card).
pub fn can_play_card(hand: &[Card], play: Card, lead: Option<Card>, trump: Suit) -> bool {
    // Rule 1: Leading is unconstrained.
    let Some(lead_card) = lead else {
        return true;
    };

    let play_is_trump = is_trump(play, trump);
    let lead_is_trump = is_trump(lead_card, trump);

    // Rule 2: Trump is always a legal play.
    //
    // (The top-trump-exemption "can't be forced" rule only prevents being *required*
    // to play a top trump. Voluntarily playing one is always fine.)
    if play_is_trump {
        return true;
    }

    // Non-trump play paths:
    if lead_is_trump {
        // Rule 3: trump was led. Non-trump is legal only if player has no
        // trump they're obligated to play (i.e., only top trumps, against a lower lead).
        return !has_playable_trump(hand, lead_card, trump);
    }

    // Non-trump was led. Rule 4: must follow suit if you have it.
    let lead_suit = lead_card.suit;
    let has_lead_suit = hand.iter().any(|c| c.suit == lead_suit && !is_trump(*c, trump));
    if has_lead_suit {
        return play.suit == lead_suit;
    }

    // Rule 5: void in lead suit — anything goes.
    true
}

/// List all legal moves from `hand` given the current trick context.
/// Convenience wrapper around `can_play_card`.
pub fn legal_moves(hand: &[Card], lead: Option<Card>, trump: Suit) -> Vec<Card> {
    hand.iter()
        .copied()
        .filter(|&card| can_play_card(hand, card, lead, trump))
        .collect()
}

/// Does the hand contain a trump that the player *must* play?
///
/// "Must play" = trump in hand, accounting for the top-trump exemption.
/// - Lead is a *top* trump: any trump must be played (top trumps can only
///   withhold against a *lower* lead).
/// - Lead is a *lower* trump: player can withhold top trumps, so "must play"
///   requires a non-top trump in hand.
fn has_playable_trump(hand: &[Card], lead_card: Card, trump: Suit) -> bool {
    let lead_is_top_trump = is_top_trump(lead_card, trump);

    for &card in hand {
        if !is_trump(card, trump) {
            continue;
        }
        if lead_is_top_trump {
            return true; // any trump is playable against a top-trump lead
        }
        if !is_top_trump(card, trump) {
            return true; // a non-top trump must be played
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::{Card, Rank, Suit};

    fn c(suit: Suit, rank: Rank) -> Card {
        Card::new(suit, rank)
    }

    // --- winning_index ---

    #[test]
    fn highest_trump_wins_simple_trick() {
        // Trump = Spades. Plays: 4S, 7S, 2S, KS. KS is highest trump (K=113 > numbers).
        let plays = [
            c(Suit::Spades, Rank::Four),
            c(Suit::Spades, Rank::Seven),
            c(Suit::Spades, Rank::Two),
            c(Suit::Spades, Rank::King),
        ];
        assert_eq!(winning_index(&plays, Suit::Spades), 3);
    }

    #[test]
    fn five_of_trump_is_highest() {
        // Trump = Clubs. Plays: AH, JC, KC, 5C. 5C wins (199 > 198 > 197 > ...).
        let plays = [
            c(Suit::Hearts, Rank::Ace),    // AH bower (197)
            c(Suit::Clubs, Rank::Jack),    // J-trump (198)
            c(Suit::Clubs, Rank::King),    // K-trump (113)
            c(Suit::Clubs, Rank::Five),    // 5-trump (199) — wins
        ];
        assert_eq!(winning_index(&plays, Suit::Clubs), 3);
    }

    #[test]
    fn trump_beats_non_trump() {
        // Hearts led (non-trump when trump = Spades). Trump 2S beats AH... wait,
        // AH is always trump, so pick a different example.
        // Lead: KH. Plays: KH, 9S (trump), 10H, QH. 9S wins (trump).
        let plays = [
            c(Suit::Hearts, Rank::King),
            c(Suit::Spades, Rank::Nine),
            c(Suit::Hearts, Rank::Ten),
            c(Suit::Hearts, Rank::Queen),
        ];
        assert_eq!(winning_index(&plays, Suit::Spades), 1);
    }

    #[test]
    fn lead_suit_beats_off_suit_non_trump() {
        // Lead: 7D. Trump = Clubs. Plays: 7D, 2S, 6D, KH. 6D wins (lead suit, high).
        // Wait — 6D has strength 6, 7D has strength 7. So 7D (index 0) beats 6D (index 2).
        // Let me redo: plays 2D, 2S, 6D, KH => 6D wins (lead + highest lead-suit).
        let plays = [
            c(Suit::Diamonds, Rank::Two),
            c(Suit::Spades, Rank::Two),    // off-suit non-trump: can't win
            c(Suit::Diamonds, Rank::Six),  // lead suit, higher
            c(Suit::Hearts, Rank::King),   // off-suit non-trump
        ];
        assert_eq!(winning_index(&plays, Suit::Clubs), 2);
    }

    #[test]
    fn ah_led_makes_trump_the_lead_suit_not_hearts() {
        // Trump = Spades. Lead: AH (trump). Plays: AH, KH (non-trump Hearts), 2C, 9H.
        // Even though the "face suit" of the lead is Hearts, the trick is a trump
        // trick — so Hearts doesn't promote KH/9H over 2C. AH wins as the only trump.
        let plays = [
            c(Suit::Hearts, Rank::Ace),
            c(Suit::Hearts, Rank::King),
            c(Suit::Clubs, Rank::Two),
            c(Suit::Hearts, Rank::Nine),
        ];
        assert_eq!(winning_index(&plays, Suit::Spades), 0);
    }

    #[test]
    fn ace_of_hearts_as_lead_is_trump() {
        // Trump = Spades. Plays: AH (trump), 3S (trump), KH (non-trump), QH (non-trump).
        // 3S strength = 100 + ? (small). AH = 197. AH wins.
        let plays = [
            c(Suit::Hearts, Rank::Ace),
            c(Suit::Spades, Rank::Three),
            c(Suit::Hearts, Rank::King),
            c(Suit::Hearts, Rank::Queen),
        ];
        assert_eq!(winning_index(&plays, Suit::Spades), 0);
    }

    // --- can_play_card: rule 1 (leading) ---

    #[test]
    fn leading_allows_any_card() {
        let hand = vec![c(Suit::Spades, Rank::Two), c(Suit::Hearts, Rank::King)];
        assert!(can_play_card(&hand, hand[0], None, Suit::Diamonds));
        assert!(can_play_card(&hand, hand[1], None, Suit::Diamonds));
    }

    // --- can_play_card: rule 2 (trump always OK) ---

    #[test]
    fn trump_always_playable_even_when_holding_lead_suit() {
        // Lead: KD. Trump = Spades. Hand: QD (lead suit), 7S (trump).
        // Playing 7S (trump) is legal even though QD is in hand.
        let hand = vec![c(Suit::Diamonds, Rank::Queen), c(Suit::Spades, Rank::Seven)];
        let lead = Some(c(Suit::Diamonds, Rank::King));
        assert!(can_play_card(&hand, c(Suit::Spades, Rank::Seven), lead, Suit::Spades));
    }

    // --- can_play_card: rule 3 (trump led, must follow trump) ---

    #[test]
    fn must_follow_trump_when_trump_led() {
        // Lead: 6S (trump). Hand: QS (trump, non-top), KH (non-trump).
        // Playing KH is illegal; player has a non-top trump to follow with.
        let hand = vec![c(Suit::Spades, Rank::Queen), c(Suit::Hearts, Rank::King)];
        let lead = Some(c(Suit::Spades, Rank::Six));
        assert!(!can_play_card(&hand, c(Suit::Hearts, Rank::King), lead, Suit::Spades));
        assert!(can_play_card(&hand, c(Suit::Spades, Rank::Queen), lead, Suit::Spades));
    }

    #[test]
    fn top_trump_exempt_from_lower_trump_lead() {
        // Lead: 6S (non-top trump). Hand: 5S (top trump), KH.
        // Player may withhold 5S and play KH.
        let hand = vec![c(Suit::Spades, Rank::Five), c(Suit::Hearts, Rank::King)];
        let lead = Some(c(Suit::Spades, Rank::Six));
        assert!(can_play_card(&hand, c(Suit::Hearts, Rank::King), lead, Suit::Spades));
    }

    #[test]
    fn top_trump_exemption_does_not_apply_against_top_trump_lead() {
        // Lead: JS (top trump). Hand: 5S (top trump), KH.
        // Player must play 5S (the exemption protects only against LOWER trump leads).
        let hand = vec![c(Suit::Spades, Rank::Five), c(Suit::Hearts, Rank::King)];
        let lead = Some(c(Suit::Spades, Rank::Jack));
        assert!(!can_play_card(&hand, c(Suit::Hearts, Rank::King), lead, Suit::Spades));
    }

    #[test]
    fn ah_exempt_from_lower_trump_lead() {
        // Lead: 6S. Hand: AH (top trump), KH. Player may play KH.
        let hand = vec![c(Suit::Hearts, Rank::Ace), c(Suit::Hearts, Rank::King)];
        let lead = Some(c(Suit::Spades, Rank::Six));
        assert!(can_play_card(&hand, c(Suit::Hearts, Rank::King), lead, Suit::Spades));
    }

    // --- can_play_card: rule 4 (follow suit if able) ---

    #[test]
    fn must_follow_non_trump_lead_when_able() {
        // Lead: 7D (non-trump). Hand: QD, 4C, KH. Playing 4C or KH is illegal.
        let hand = vec![
            c(Suit::Diamonds, Rank::Queen),
            c(Suit::Clubs, Rank::Four),
            c(Suit::Hearts, Rank::King),
        ];
        let lead = Some(c(Suit::Diamonds, Rank::Seven));
        let trump = Suit::Spades;
        assert!(can_play_card(&hand, c(Suit::Diamonds, Rank::Queen), lead, trump));
        assert!(!can_play_card(&hand, c(Suit::Clubs, Rank::Four), lead, trump));
        assert!(!can_play_card(&hand, c(Suit::Hearts, Rank::King), lead, trump));
    }

    #[test]
    fn hearts_in_hand_doesnt_count_as_lead_when_trump_is_hearts() {
        // Trump = Hearts. Lead: 7D. "Has hearts" is irrelevant; player must follow diamonds.
        let hand = vec![c(Suit::Hearts, Rank::King), c(Suit::Spades, Rank::Four)];
        let lead = Some(c(Suit::Diamonds, Rank::Seven));
        // Player is void in diamonds → may play anything.
        assert!(can_play_card(&hand, c(Suit::Spades, Rank::Four), lead, Suit::Hearts));
    }

    #[test]
    fn ah_held_does_not_count_as_hearts_when_trump_not_hearts() {
        // Trump = Spades. Lead: 7H. Hand: AH (trump, not "hearts" for follow-suit),
        // 4C. Player is void in *non-trump* hearts, so 4C is legal.
        let hand = vec![c(Suit::Hearts, Rank::Ace), c(Suit::Clubs, Rank::Four)];
        let lead = Some(c(Suit::Hearts, Rank::Seven));
        assert!(can_play_card(&hand, c(Suit::Clubs, Rank::Four), lead, Suit::Spades));
    }

    // --- can_play_card: rule 5 (void → anything goes) ---

    #[test]
    fn void_in_lead_suit_allows_any() {
        // Lead: 7D. Hand: 2S, KH. Void in diamonds → both plays legal.
        let hand = vec![c(Suit::Spades, Rank::Two), c(Suit::Hearts, Rank::King)];
        let lead = Some(c(Suit::Diamonds, Rank::Seven));
        let trump = Suit::Clubs;
        assert!(can_play_card(&hand, c(Suit::Spades, Rank::Two), lead, trump));
        assert!(can_play_card(&hand, c(Suit::Hearts, Rank::King), lead, trump));
    }

    // --- legal_moves ---

    #[test]
    fn legal_moves_returns_all_playable() {
        // Lead: 7D. Hand: QD, 4C, KH. Only QD is legal.
        let hand = vec![
            c(Suit::Diamonds, Rank::Queen),
            c(Suit::Clubs, Rank::Four),
            c(Suit::Hearts, Rank::King),
        ];
        let lead = Some(c(Suit::Diamonds, Rank::Seven));
        let legal = legal_moves(&hand, lead, Suit::Spades);
        assert_eq!(legal, vec![c(Suit::Diamonds, Rank::Queen)]);
    }

    #[test]
    fn legal_moves_void_returns_everything() {
        let hand = vec![c(Suit::Spades, Rank::Two), c(Suit::Hearts, Rank::King)];
        let lead = Some(c(Suit::Diamonds, Rank::Seven));
        let legal = legal_moves(&hand, lead, Suit::Clubs);
        assert_eq!(legal.len(), 2);
    }
}
