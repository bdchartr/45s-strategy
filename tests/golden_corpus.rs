//! Golden corpus: hand-authored 4-player scenarios that pin specific rule
//! behaviors end-to-end through the engine's public API.
//!
//! # Scope
//!
//! These tests use `GameState::from_state(...)` to bypass the deck shuffle
//! and stamp a known hand at every seat. That lets us exercise specific rule
//! interactions (top-trump exemption, reneging, scoring at game end) on a
//! *predictable* trick path rather than relying on random play.
//!
//! # What this is *not*
//!
//! This is **not** a Rust↔PHP rule-divergence test. The original Stage-0 plan
//! envisioned PHP serializing game traces and Rust replaying them. PHP's rule
//! logic turned out to be entangled with its DB layer in
//! `wkapp-45/app/Application/Services/GameRuntimeService.php`, so emitting
//! standalone traces from PHP would require non-trivial extraction work.
//!
//! Pragmatic call: deliver Rust-only authored scenarios for Stage-0 and defer
//! PHP-corpus comparison to a later stage. See `docs/stage-0.md` for the
//! scope-reduction rationale and the deferred work.
//!
//! # Coverage
//!
//! 1. `sanity_full_play_through` — confirms `from_state` produces a state
//!    that drives cleanly from Bidding → HandComplete with all-pass + dealer
//!    forced to 15. Card conservation must hold.
//! 2. `top_trump_exemption_against_lower_trump_lead` — when a low trump is
//!    led, a player holding only a top trump (here AH while trump = Spades)
//!    plus off-suit cards may play the off-suit card; the engine must accept
//!    it. Exercises the bower-exemption legality through `apply()`.
//! 3. `must_follow_trump_when_lower_trump_led_and_holding_mid_trump` — same
//!    setup, but the player holds a mid-trump (KS) and an off-suit card. The
//!    engine must reject the off-suit play and accept KS.
//! 4. `bid_made_team_sweeps_with_top_trumps` — bidder team wins all 5 tricks
//!    (25 trick points + 5 high-trump bonus = 30 hand points). Bid 20.
//!    Asserts `bid_made = true`, scores match, sets unchanged.
//! 5. `bid_set_bidder_team_loses_all_tricks` — bidder team wins 0 tricks,
//!    fails their 20-bid. Asserts `bid_made = false`, bidder team
//!    `scores -= 20`, `sets += 1`, opponent team gets their hand points.

use _engine::cards::{standard_deck, Card};
use _engine::state::{Action, GameConfig, GameState, Move, Phase};
use std::collections::HashSet;
use std::str::FromStr;

const FULL_DECK: usize = 52;

fn c(s: &str) -> Card {
    Card::from_str(s).unwrap_or_else(|e| panic!("bad card code {:?}: {:?}", s, e))
}

fn parse_hand(codes: &[&str]) -> Vec<Card> {
    codes.iter().map(|s| c(s)).collect()
}

/// Build the 29-card deck remainder for a 4-player scenario: every card not
/// dealt into a hand or the kitty.
fn remainder_excluding(used: &[Card]) -> Vec<Card> {
    let used: HashSet<Card> = used.iter().copied().collect();
    standard_deck().into_iter().filter(|c| !used.contains(c)).collect()
}

/// Multiset of every card the engine currently tracks. See proptest_invariants
/// for the same shape — this is the conservation-check helper.
fn all_cards(g: &GameState) -> Vec<Card> {
    let mut out: Vec<Card> = Vec::with_capacity(FULL_DECK);
    for seat in 0..g.num_players() {
        out.extend_from_slice(g.hand(seat));
    }
    out.extend_from_slice(g.kitty());
    out.extend_from_slice(g.deck_remainder());
    out.extend_from_slice(g.discarded());
    for (_, c) in g.current_trick() {
        out.push(*c);
    }
    for t in g.completed_tricks() {
        for (_, c) in &t.plays {
            out.push(*c);
        }
    }
    out.sort_by_key(|c| (c.suit as u8, c.rank as u8));
    out
}

fn assert_conservation(g: &GameState) {
    let cards = all_cards(g);
    assert_eq!(cards.len(), FULL_DECK, "expected 52 cards, got {}", cards.len());
    for w in cards.windows(2) {
        assert_ne!(w[0], w[1], "duplicate card {:?}", w[0]);
    }
}

fn four_player_config() -> GameConfig {
    GameConfig { num_players: 4, target_score: 120, enable_30_for_60: true }
}

// -----------------------------------------------------------------------------
// Scenario 1 — sanity check
// -----------------------------------------------------------------------------

/// Confirms `from_state` produces a state that drives cleanly through the
/// engine's apply() loop. Every seat passes; dealer is forced to bid 15;
/// dealer declares Diamonds; only bidder discards (3 cards); bidder leads
/// each trick with whatever they have. We don't pin specific scores — only
/// that we reach `HandComplete` with conservation intact.
#[test]
fn sanity_full_play_through() {
    let hands = vec![
        parse_hand(&["2D", "3D", "4D", "5D", "JD"]), // seat 0 (dealer + bidder after all-pass)
        parse_hand(&["2C", "3C", "4C", "5C", "6C"]), // seat 1
        parse_hand(&["2S", "3S", "4S", "5S", "6S"]), // seat 2
        parse_hand(&["7C", "8C", "9C", "10C", "AH"]), // seat 3
    ];
    let kitty = parse_hand(&["7S", "8S", "9S"]);
    let mut used: Vec<Card> = hands.iter().flatten().copied().collect();
    used.extend(kitty.iter().copied());
    let deck_remainder = remainder_excluding(&used);

    let mut g = GameState::from_state(hands, kitty, deck_remainder, four_player_config(), 0);
    assert_eq!(g.phase(), Phase::Bidding);
    assert_eq!(g.to_act(), Some(1));
    assert_conservation(&g);

    // Seats 1, 2, 3 pass. Dealer (seat 0) is then to_act with no current bid.
    for seat in [1u8, 2, 3] {
        g.apply(Move { seat, action: Action::Bid { amount: 0 } }).unwrap();
    }
    // Dealer passes — engine forces dealer to 15 internally.
    g.apply(Move { seat: 0, action: Action::Bid { amount: 0 } }).unwrap();
    assert_eq!(g.phase(), Phase::DeclareTrump);
    assert_eq!(g.bidder(), Some(0));

    // Declare Diamonds. AH (seat 3) becomes a trump bower for the rest of the hand.
    g.apply(Move { seat: 0, action: Action::DeclareTrump(_engine::cards::Suit::Diamonds) }).unwrap();
    assert_eq!(g.phase(), Phase::Discard);
    assert_eq!(g.hand(0).len(), 8); // 5 + 3 kitty

    // Bidder drops 3 weakest non-trump (kitty was all spades; bidder is all diamonds).
    let to_drop = parse_hand(&["7S", "8S", "9S"]);
    g.apply(Move { seat: 0, action: Action::Discard(to_drop) }).unwrap();
    // Non-bidders pass on discard.
    for seat in [1u8, 2, 3] {
        g.apply(Move { seat, action: Action::Discard(vec![]) }).unwrap();
    }
    assert_eq!(g.phase(), Phase::Play);
    assert_eq!(g.to_act(), Some(0)); // bidder leads first trick
    assert_conservation(&g);

    // Drive 5 tricks by always picking the first legal play.
    while g.phase() == Phase::Play {
        let seat = g.to_act().unwrap();
        let legal = g.legal_actions(seat);
        assert!(!legal.is_empty(), "no legal play for seat {}", seat);
        g.apply(Move { seat, action: legal[0].clone() }).unwrap();
    }
    assert_eq!(g.phase(), Phase::HandComplete);
    assert_eq!(g.completed_tricks().len(), 5);
    assert_conservation(&g);

    // Hand points sum to 30 (25 trick pts + 5 high-trump bonus).
    let hp = g.hand_points();
    assert_eq!(hp[0] + hp[1], 30, "hand points must sum to 30, got {:?}", hp);
}

// -----------------------------------------------------------------------------
// Scenarios 2 & 3 — top-trump exemption vs. forced trump follow
// -----------------------------------------------------------------------------

/// Scenario 2: trump = Spades, seat 0 leads 2S (the lowest trump). Seat 1
/// holds AH and a club. AH is the third-highest trump (a bower). Because the
/// lead is a *lower* trump (not a top trump), seat 1 may withhold AH and
/// play the off-suit club instead. The engine must list the off-suit as
/// legal and accept it.
#[test]
fn top_trump_exemption_against_lower_trump_lead() {
    // Seat 0 (dealer) bids 15 immediately (everyone else passes by default to
    // make this short — but easier: seat 1 bids first and we have seat 0 win
    // by holding). Use the simpler all-pass-then-dealer-15 path.
    let hands = vec![
        parse_hand(&["2S", "3S", "4S", "5S", "6S"]), // dealer + bidder
        parse_hand(&["AH", "2C", "3C", "4C", "5C"]), // holds AH + clubs
        parse_hand(&["7C", "8C", "9C", "10C", "JC"]),
        parse_hand(&["6C", "QC", "KC", "AC", "JS"]),
    ];
    let kitty = parse_hand(&["2D", "3D", "4D"]);
    let mut used: Vec<Card> = hands.iter().flatten().copied().collect();
    used.extend(kitty.iter().copied());
    let deck_remainder = remainder_excluding(&used);

    let mut g = GameState::from_state(hands, kitty, deck_remainder, four_player_config(), 0);

    // All pass → dealer forced to 15.
    for seat in [1u8, 2, 3, 0] {
        g.apply(Move { seat, action: Action::Bid { amount: 0 } }).unwrap();
    }
    g.apply(Move { seat: 0, action: Action::DeclareTrump(_engine::cards::Suit::Spades) }).unwrap();
    // Bidder absorbed kitty (3 diamonds, all non-trump). Drop them.
    g.apply(Move { seat: 0, action: Action::Discard(parse_hand(&["2D", "3D", "4D"])) }).unwrap();
    for seat in [1u8, 2, 3] {
        g.apply(Move { seat, action: Action::Discard(vec![]) }).unwrap();
    }
    assert_eq!(g.phase(), Phase::Play);

    // Trick 1: dealer leads 2S (the lowest trump in their hand).
    g.apply(Move { seat: 0, action: Action::Play(c("2S")) }).unwrap();
    assert_eq!(g.to_act(), Some(1));

    // Seat 1 holds AH (top trump #3 — exempt) plus clubs. legal_actions must
    // include both the off-suit clubs AND AH. The exemption means seat 1 is
    // *allowed* to play off-suit despite holding a trump.
    let legal = g.legal_actions(1);
    let legal_cards: Vec<Card> = legal
        .iter()
        .filter_map(|a| match a {
            Action::Play(c) => Some(*c),
            _ => None,
        })
        .collect();
    assert!(legal_cards.contains(&c("AH")), "AH must be in legal plays");
    assert!(
        legal_cards.contains(&c("2C")),
        "off-suit must be legal under top-trump exemption (got {:?})",
        legal_cards
    );

    // Play the off-suit. Engine must accept.
    g.apply(Move { seat: 1, action: Action::Play(c("2C")) }).unwrap();
    assert_eq!(g.current_trick().len(), 2);
}

/// Scenario 3: same setup as scenario 2, but the player at the spotlight
/// seat holds a *non-top* trump (KS) plus an off-suit card. The trump-follow
/// rule applies; the off-suit play must be rejected.
///
/// This pairs with scenario 2 to pin the exemption boundary: top trumps
/// exempt; non-top trumps do not.
#[test]
fn must_follow_trump_when_lower_trump_led_and_holding_mid_trump() {
    let hands = vec![
        parse_hand(&["2S", "3S", "4S", "5S", "6S"]),
        parse_hand(&["KS", "2C", "3C", "4C", "5C"]), // KS is mid-trump (not top), must follow
        parse_hand(&["7C", "8C", "9C", "10C", "JC"]),
        parse_hand(&["6C", "QC", "KC", "AC", "AH"]),
    ];
    let kitty = parse_hand(&["2D", "3D", "4D"]);
    let mut used: Vec<Card> = hands.iter().flatten().copied().collect();
    used.extend(kitty.iter().copied());
    let deck_remainder = remainder_excluding(&used);

    let mut g = GameState::from_state(hands, kitty, deck_remainder, four_player_config(), 0);

    for seat in [1u8, 2, 3, 0] {
        g.apply(Move { seat, action: Action::Bid { amount: 0 } }).unwrap();
    }
    g.apply(Move { seat: 0, action: Action::DeclareTrump(_engine::cards::Suit::Spades) }).unwrap();
    g.apply(Move { seat: 0, action: Action::Discard(parse_hand(&["2D", "3D", "4D"])) }).unwrap();
    for seat in [1u8, 2, 3] {
        g.apply(Move { seat, action: Action::Discard(vec![]) }).unwrap();
    }

    // Trick 1: dealer leads 2S.
    g.apply(Move { seat: 0, action: Action::Play(c("2S")) }).unwrap();

    // Seat 1 holds KS (non-top trump) + clubs. KS is NOT exempt. Must follow trump.
    // legal plays should be only spades (specifically KS, the only spade in hand).
    let legal_cards: Vec<Card> = g
        .legal_actions(1)
        .iter()
        .filter_map(|a| match a {
            Action::Play(c) => Some(*c),
            _ => None,
        })
        .collect();
    assert!(legal_cards.contains(&c("KS")), "KS must be legal");
    assert!(
        !legal_cards.contains(&c("2C")),
        "off-suit must NOT be legal when holding mid-trump (got {:?})",
        legal_cards
    );

    // Engine must reject an off-suit play.
    let bad = g.clone().apply(Move { seat: 1, action: Action::Play(c("2C")) });
    assert!(bad.is_err(), "off-suit while holding mid-trump must be rejected");

    // KS is accepted.
    g.apply(Move { seat: 1, action: Action::Play(c("KS")) }).unwrap();
}

// -----------------------------------------------------------------------------
// Scenarios 4 & 5 — scoring at hand-end (made vs. set)
// -----------------------------------------------------------------------------

/// Bidder team sweeps all 5 tricks: 25 trick points + 5 high-trump bonus
/// (5D played) = 30 hand points to the bidder team. Bid 20, made → bidder
/// team scores += 30 (full hand points), opponent += 0.
#[test]
fn bid_made_team_sweeps_with_top_trumps() {
    // Dealer = 0; bidder will be seat 1 (first to bid). Seat 1 holds the
    // five highest diamond trumps (5D, JD, KD, QD + 10D). AH is held by
    // seat 1 too — making it the absolute strongest hand once Diamonds
    // is declared. Other seats hold cards in suits that won't follow trump.
    let hands = vec![
        parse_hand(&["2C", "3C", "4C", "5C", "6C"]), // seat 0 (dealer, opponent)
        parse_hand(&["5D", "JD", "AH", "KD", "QD"]), // seat 1 (bidder, will absorb 3 spade-junk kitty)
        parse_hand(&["7C", "8C", "9C", "10C", "JC"]), // seat 2 (opponent)
        parse_hand(&["2S", "3S", "4S", "5S", "6S"]), // seat 3 (bidder partner)
    ];
    let kitty = parse_hand(&["7S", "8S", "9S"]);
    let mut used: Vec<Card> = hands.iter().flatten().copied().collect();
    used.extend(kitty.iter().copied());
    let deck_remainder = remainder_excluding(&used);

    let mut g = GameState::from_state(hands, kitty, deck_remainder, four_player_config(), 0);

    // Seat 1 bids 20; everyone else passes.
    g.apply(Move { seat: 1, action: Action::Bid { amount: 20 } }).unwrap();
    g.apply(Move { seat: 2, action: Action::Bid { amount: 0 } }).unwrap();
    g.apply(Move { seat: 3, action: Action::Bid { amount: 0 } }).unwrap();
    g.apply(Move { seat: 0, action: Action::Bid { amount: 0 } }).unwrap();
    assert_eq!(g.bidder(), Some(1));

    g.apply(Move { seat: 1, action: Action::DeclareTrump(_engine::cards::Suit::Diamonds) }).unwrap();
    // Bidder absorbed 3 spades. Drop them.
    g.apply(Move { seat: 1, action: Action::Discard(parse_hand(&["7S", "8S", "9S"])) }).unwrap();
    for seat in [0u8, 2, 3] {
        g.apply(Move { seat, action: Action::Discard(vec![]) }).unwrap();
    }
    assert_eq!(g.phase(), Phase::Play);
    assert_eq!(g.to_act(), Some(1));

    // Bidder leads each trick with a top trump — no other seat has a diamond
    // or AH, so all follow off-suit and bidder wins every trick.
    for lead in ["5D", "JD", "AH", "KD", "QD"] {
        // Bidder leads.
        g.apply(Move { seat: 1, action: Action::Play(c(lead)) }).unwrap();
        // Other seats follow with their first legal play.
        for _ in 0..3 {
            let seat = g.to_act().unwrap();
            let legal = g.legal_actions(seat);
            g.apply(Move { seat, action: legal[0].clone() }).unwrap();
        }
    }

    assert_eq!(g.phase(), Phase::HandComplete);
    assert_eq!(g.bid_made(), Some(true), "bid 20 with 30 hand pts must be made");
    let hp = g.hand_points();
    assert_eq!(hp, [0, 30], "team 1 sweeps 25 + 5 high-trump bonus = 30");
    let scores = g.scores();
    assert_eq!(scores, [0, 30], "bidder team scores their full hand points");
    assert_eq!(g.sets(), [0, 0]);
    assert_conservation(&g);
}

/// Bidder team wins 0 tricks. Bid was 20, hand points are [30, 0]. Bidder
/// team is set: scores -= 20, sets += 1. Opponent team scores their hand
/// points (30).
#[test]
fn bid_set_bidder_team_loses_all_tricks() {
    // Setup mirrors scenario 4 but with the strong hand on the *opponent*
    // side. Bidder (seat 1) holds weak trumps (2D, 3D, 4D) plus junk;
    // opponent seat 0 holds 5D, JD, AH, KD, QD.
    let hands = vec![
        parse_hand(&["5D", "JD", "AH", "KD", "QD"]), // seat 0 (dealer; will be opponent)
        parse_hand(&["2D", "3D", "4D", "2C", "3C"]), // seat 1 (bidder, weak trumps)
        parse_hand(&["4C", "5C", "6C", "7C", "8C"]), // seat 2 (opponent partner)
        parse_hand(&["9C", "10C", "JC", "2S", "3S"]), // seat 3 (bidder partner)
    ];
    let kitty = parse_hand(&["4S", "5S", "6S"]);
    let mut used: Vec<Card> = hands.iter().flatten().copied().collect();
    used.extend(kitty.iter().copied());
    let deck_remainder = remainder_excluding(&used);

    let mut g = GameState::from_state(hands, kitty, deck_remainder, four_player_config(), 0);

    // Seat 1 bids 20 (foolishly); opponents pass.
    g.apply(Move { seat: 1, action: Action::Bid { amount: 20 } }).unwrap();
    g.apply(Move { seat: 2, action: Action::Bid { amount: 0 } }).unwrap();
    g.apply(Move { seat: 3, action: Action::Bid { amount: 0 } }).unwrap();
    g.apply(Move { seat: 0, action: Action::Bid { amount: 0 } }).unwrap();
    assert_eq!(g.bidder(), Some(1));

    g.apply(Move { seat: 1, action: Action::DeclareTrump(_engine::cards::Suit::Diamonds) }).unwrap();
    // Bidder absorbed 3 spades (4S, 5S, 6S). Trump-keeper rule: must drop
    // non-trump. After absorption seat 1 has 2D, 3D, 4D, 2C, 3C, 4S, 5S, 6S.
    // Drop the spades.
    g.apply(Move { seat: 1, action: Action::Discard(parse_hand(&["4S", "5S", "6S"])) }).unwrap();
    for seat in [0u8, 2, 3] {
        g.apply(Move { seat, action: Action::Discard(vec![]) }).unwrap();
    }

    // Trick 1: bidder leads. Strongest available is 4D. Opponent seat 0
    // overtops with 5D and wins. Seats 2, 3 have no trump → play any club/spade.
    g.apply(Move { seat: 1, action: Action::Play(c("4D")) }).unwrap();
    g.apply(Move { seat: 2, action: Action::Play(c("4C")) }).unwrap();
    g.apply(Move { seat: 3, action: Action::Play(c("9C")) }).unwrap();
    g.apply(Move { seat: 0, action: Action::Play(c("5D")) }).unwrap();
    assert_eq!(g.completed_tricks().last().unwrap().winner, 0);

    // Trick 2: seat 0 leads JD. Seat 1 must follow trump (3D). Others off-suit.
    g.apply(Move { seat: 0, action: Action::Play(c("JD")) }).unwrap();
    g.apply(Move { seat: 1, action: Action::Play(c("3D")) }).unwrap();
    g.apply(Move { seat: 2, action: Action::Play(c("5C")) }).unwrap();
    g.apply(Move { seat: 3, action: Action::Play(c("10C")) }).unwrap();
    assert_eq!(g.completed_tricks().last().unwrap().winner, 0);

    // Trick 3: seat 0 leads AH. Seat 1 must follow trump (2D, last diamond).
    g.apply(Move { seat: 0, action: Action::Play(c("AH")) }).unwrap();
    g.apply(Move { seat: 1, action: Action::Play(c("2D")) }).unwrap();
    g.apply(Move { seat: 2, action: Action::Play(c("6C")) }).unwrap();
    g.apply(Move { seat: 3, action: Action::Play(c("JC")) }).unwrap();
    assert_eq!(g.completed_tricks().last().unwrap().winner, 0);

    // Tricks 4–5: seat 0 leads remaining trumps. Seat 1 has no trump left;
    // plays clubs. Seat 2 plays clubs. Seat 3 plays last cards.
    g.apply(Move { seat: 0, action: Action::Play(c("KD")) }).unwrap();
    g.apply(Move { seat: 1, action: Action::Play(c("2C")) }).unwrap();
    g.apply(Move { seat: 2, action: Action::Play(c("7C")) }).unwrap();
    g.apply(Move { seat: 3, action: Action::Play(c("2S")) }).unwrap();
    assert_eq!(g.completed_tricks().last().unwrap().winner, 0);

    g.apply(Move { seat: 0, action: Action::Play(c("QD")) }).unwrap();
    g.apply(Move { seat: 1, action: Action::Play(c("3C")) }).unwrap();
    g.apply(Move { seat: 2, action: Action::Play(c("8C")) }).unwrap();
    g.apply(Move { seat: 3, action: Action::Play(c("3S")) }).unwrap();

    assert_eq!(g.phase(), Phase::HandComplete);
    let hp = g.hand_points();
    assert_eq!(hp, [30, 0], "team 0 swept (25 trick + 5 high-trump bonus)");
    assert_eq!(g.bid_made(), Some(false), "bidder won 0 trump points; set");
    let scores = g.scores();
    assert_eq!(scores, [30, -20], "team 0 +30, team 1 -20 (set penalty = bid amount)");
    assert_eq!(g.sets(), [0, 1], "bidder team takes a set");
    assert_conservation(&g);
}
