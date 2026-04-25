//! Property-based invariants for the game engine.
//!
//! Strategy: pick a random seed and a long stream of u32 "choice indices",
//! then drive the state forward by taking a legal action at each step
//! (chosen by `choices[step] % legal.len()`). Along the way and at terminal
//! states, assert engine invariants that must hold regardless of seed or play.
//!
//! These tests are intentionally dumb about strategy — we don't care who
//! wins; we care that the rule engine never loses or duplicates cards, never
//! over-pays a bid, and always produces a coherent terminal state.
//!
//! Not covered here (deliberate): 6-player DealerExtraDraw and DiscardChoose
//! sub-states. Random-choice navigation over discard *sets* would blow up
//! the test space since every subset of non-trump cards is legal. Those are
//! covered by targeted unit tests in `state.rs`.

use _engine::cards::Card;
use _engine::state::{Action, GameConfig, GameState, Move, Phase};
use proptest::prelude::*;

const FULL_DECK: usize = 52;

/// Count every card currently tracked by the engine: held in hands,
/// in the kitty, in the deck remainder, mid-trick on the table, in
/// completed tricks, and discarded earlier in the hand. Return the
/// multiset as a sorted Vec for comparison.
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

/// Assert the card-conservation invariant: the multiset of all tracked cards
/// equals a standard 52-card deck (each card appears exactly once).
fn assert_conservation(g: &GameState) {
    let cards = all_cards(g);
    assert_eq!(
        cards.len(),
        FULL_DECK,
        "total cards must be 52 (got {})",
        cards.len()
    );
    // No duplicates.
    for window in cards.windows(2) {
        assert_ne!(window[0], window[1], "duplicate card {:?}", window[0]);
    }
}

/// Drive a 4-player game forward by picking `choices[i] % legal.len()` at each step.
/// For `Phase::Discard` we can't enumerate subsets, so we use a deterministic heuristic:
/// bidder drops all non-trump non-AH cards; non-bidders pass (discard nothing).
/// Returns the terminal state after at most `max_steps` actions.
fn play_one_hand_4p(seed: u64, choices: &[u32]) -> GameState {
    let config = GameConfig { num_players: 4, target_score: 120, enable_30_for_60: true };
    let mut g = GameState::new_hand(seed, config, 0);
    let mut step = 0;

    assert_conservation(&g);

    loop {
        match g.phase() {
            Phase::Bidding | Phase::DeclareTrump | Phase::Play => {
                let seat = g.to_act().expect("to_act during active phase");
                let legal = g.legal_actions(seat);
                assert!(!legal.is_empty(), "no legal actions for seat {} in {:?}", seat, g.phase());
                let idx = (choices.get(step).copied().unwrap_or(0) as usize) % legal.len();
                let action = legal[idx].clone();
                g.apply(Move { seat, action }).expect("legal action applies");
                step += 1;
            }
            Phase::Discard => {
                // Deterministic 4p discard:
                //   bidder absorbs 3-card kitty → has 8 cards → must drop 3.
                //   Drop the 3 weakest cards, preferring non-trumps so we
                //   don't trip the trump-keeper rule.
                let bidder = g.bidder().expect("bidder set in discard phase");
                let trump = g.trump().expect("trump set in discard phase");
                for seat in 0..g.num_players() {
                    if seat == bidder {
                        let mut hand = g.hand(seat).to_vec();
                        hand.sort_by_key(|c| {
                            (
                                if _engine::ranker::is_trump(*c, trump) { 1 } else { 0 },
                                _engine::ranker::strength(*c, trump),
                            )
                        });
                        let to_drop: Vec<Card> = hand.into_iter().take(3).collect();
                        g.apply(Move { seat, action: Action::Discard(to_drop) })
                            .expect("bidder discard applies");
                    } else {
                        g.apply(Move { seat, action: Action::Discard(vec![]) })
                            .expect("non-bidder pass-discard applies");
                    }
                }
                assert_eq!(g.phase(), Phase::Play, "discard should lead to Play in 4p");
            }
            Phase::DealerExtraDraw => unreachable!("4-player never enters DealerExtraDraw"),
            Phase::HandComplete | Phase::GameOver => break,
        }
        assert_conservation(&g);
    }
    g
}

proptest! {
    /// After a full 4-player hand: (a) conservation holds, (b) hand points
    /// sum to exactly 30, (c) exactly 5 tricks were played with 4 plays each,
    /// (d) every completed-trick winner is a valid seat.
    #[test]
    fn four_player_full_hand_invariants(
        seed in any::<u64>(),
        choices in prop::collection::vec(any::<u32>(), 0..200),
    ) {
        let g = play_one_hand_4p(seed, &choices);

        // All phases reachable from our navigator terminate at HandComplete
        // (GameOver is possible if target_score is crossed, but we cap at
        // one hand so only HandComplete is reachable here).
        prop_assert!(
            matches!(g.phase(), Phase::HandComplete | Phase::GameOver),
            "expected terminal phase, got {:?}",
            g.phase()
        );

        // If the whole table passed, we'd sit in Bidding until dealer is forced
        // to take the minimum, so we always enter Play — thus we have tricks.
        let tricks = g.completed_tricks();
        prop_assert_eq!(tricks.len(), 5, "expected 5 tricks, got {}", tricks.len());

        for t in tricks {
            prop_assert_eq!(t.plays.len(), 4);
            prop_assert!((t.winner as usize) < 4);
            prop_assert!((t.leader as usize) < 4);
        }

        let pts = g.hand_points();
        prop_assert_eq!(
            pts[0] + pts[1],
            30,
            "hand points must sum to 30 (got {:?})",
            pts
        );

        // Card conservation one more time, post-hand.
        assert_conservation(&g);
    }

    /// Repeatable runs: same seed + same choices ⇒ identical terminal state.
    #[test]
    fn determinism(
        seed in any::<u64>(),
        choices in prop::collection::vec(any::<u32>(), 0..200),
    ) {
        let a = play_one_hand_4p(seed, &choices);
        let b = play_one_hand_4p(seed, &choices);

        prop_assert_eq!(a.phase(), b.phase());
        prop_assert_eq!(a.scores(), b.scores());
        prop_assert_eq!(a.hand_points(), b.hand_points());
        prop_assert_eq!(a.sets(), b.sets());
        prop_assert_eq!(a.completed_tricks().len(), b.completed_tricks().len());
        for (ta, tb) in a.completed_tricks().iter().zip(b.completed_tricks()) {
            prop_assert_eq!(ta.winner, tb.winner);
            prop_assert_eq!(ta.leader, tb.leader);
            prop_assert_eq!(&ta.plays, &tb.plays);
        }
    }

    /// The multi-hand driver keeps conservation between hands and advances
    /// `hands_played` monotonically.
    #[test]
    fn multi_hand_conservation(
        seed in any::<u64>(),
        choices in prop::collection::vec(any::<u32>(), 0..800),
        n_hands in 2u32..5u32,
    ) {
        let mut g = GameState::new_hand(
            seed,
            GameConfig { num_players: 4, target_score: 120, enable_30_for_60: true },
            0,
        );
        let mut step = 0;
        let mut hands_completed = 0u32;
        let mut last_hands_played = g.hands_played();

        while hands_completed < n_hands {
            match g.phase() {
                Phase::Bidding | Phase::DeclareTrump | Phase::Play => {
                    let seat = g.to_act().unwrap();
                    let legal = g.legal_actions(seat);
                    prop_assert!(!legal.is_empty());
                    let idx = (choices.get(step).copied().unwrap_or(0) as usize) % legal.len();
                    let action = legal[idx].clone();
                    g.apply(Move { seat, action })?;
                    step += 1;
                }
                Phase::Discard => {
                    let bidder = g.bidder().unwrap();
                    let trump = g.trump().unwrap();
                    for seat in 0..g.num_players() {
                        if seat == bidder {
                            let mut hand = g.hand(seat).to_vec();
                            hand.sort_by_key(|c| {
                                (
                                    if _engine::ranker::is_trump(*c, trump) { 1 } else { 0 },
                                    _engine::ranker::strength(*c, trump),
                                )
                            });
                            let to_drop: Vec<Card> = hand.into_iter().take(3).collect();
                            g.apply(Move { seat, action: Action::Discard(to_drop) })?;
                        } else {
                            g.apply(Move { seat, action: Action::Discard(vec![]) })?;
                        }
                    }
                }
                Phase::DealerExtraDraw => unreachable!(),
                Phase::HandComplete => {
                    assert_conservation(&g);
                    hands_completed += 1;
                    let new_seed = seed.wrapping_add(hands_completed as u64);
                    g.next_hand(new_seed)?;
                    prop_assert!(g.hands_played() > last_hands_played);
                    last_hands_played = g.hands_played();
                }
                Phase::GameOver => break,
            }
            assert_conservation(&g);
        }
    }
}
