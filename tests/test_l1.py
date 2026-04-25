"""Tests for L1 Novice and the tournament harness."""

from __future__ import annotations

import pytest

from f45 import GameState, card_strength, is_top_trump, is_trump
from f45.strategies import L1Novice
from f45.tournament import play_game, run_tournament


# -----------------------------------------------------------------------------
# Engine helpers (rule primitives exposed across the FFI boundary)
# -----------------------------------------------------------------------------

def test_card_strength_orders_top_trumps_correctly():
    # Trump = Spades. 5S > JS > AH > AS > KS.
    five_s = card_strength("5S", "S")
    j_s = card_strength("JS", "S")
    a_h = card_strength("AH", "S")
    a_s = card_strength("AS", "S")
    k_s = card_strength("KS", "S")
    assert five_s > j_s > a_h > a_s > k_s


def test_is_trump_recognizes_ah_under_any_trump():
    for trump in "CDHS":
        assert is_trump("AH", trump), f"AH should be trump under {trump}"


def test_is_top_trump_three_bowers():
    assert is_top_trump("5S", "S")
    assert is_top_trump("JS", "S")
    assert is_top_trump("AH", "S")
    assert not is_top_trump("AS", "S")  # A-of-trump is 4th, not a bower
    assert not is_top_trump("KS", "S")


# -----------------------------------------------------------------------------
# L1 Novice
# -----------------------------------------------------------------------------

def test_l1_drives_a_hand_to_completion():
    state = GameState(seed=42, num_players=4, dealer=0)
    bots = [L1Novice(seed=s) for s in range(4)]

    while state.phase() not in ("hand_complete", "game_over"):
        phase = state.phase()
        if phase == "discard":
            for seat in range(4):
                bots[seat].act(state, seat)
        else:
            bots[state.to_act()].act(state, seat=state.to_act())

    assert state.phase() == "hand_complete"
    hp = state.hand_points()
    assert sum(hp) == 30, f"hand_points must sum to 30, got {hp}"


def test_l1_passes_when_no_legal_bid_amount():
    """Any hand has exactly 5 cards; max suit count is 1..5 → bid in {5, 10, 15, 20, 25}.
    Hands with max_count ≤ 2 cannot legally bid (5 and 10 aren't legal amounts)."""
    # Drive several seeds; assert we observe at least one full-pass hand.
    saw_pass = False
    for seed in range(30):
        state = GameState(seed=seed, num_players=4, dealer=0)
        bots = [L1Novice(seed=0) for _ in range(4)]
        # Run only the bidding phase
        while state.phase() == "bidding":
            seat = state.to_act()
            bots[seat].act(state, seat)
        # If no bid was taken, the dealer was forced to 15.
        winning_bid = state.winning_bid()
        if winning_bid is not None and winning_bid[1] == 15 and winning_bid[0] == 0:
            # Verify it really came from "all passed → forced 15", not a real bid.
            # Easy proxy: bidder == dealer AND amount == 15.
            saw_pass = True
            break
    assert saw_pass, "expected at least one all-pass hand in 30 seeds"


def test_l1_discard_respects_trump_keeper_when_possible():
    """If the bidder has enough non-trump cards to cover the discard, no trump
    should appear in the discard pile. We don't try to construct a forced-
    trump-discard scenario here — proptest covers that via random play."""
    for seed in range(20):
        state = GameState(seed=seed, num_players=4, dealer=0)
        bots = [L1Novice(seed=0) for _ in range(4)]
        # Drive bidding + declare; if no real bid taken, dealer has 15.
        while state.phase() in ("bidding", "declare_trump"):
            if state.phase() == "discard":
                break
            seat = state.to_act()
            bots[seat].act(state, seat)
        if state.phase() != "discard":
            continue  # All passed → already in discard from the forced bid

        bidder = state.bidder()
        trump = state.trump()
        bidder_hand_pre = state.hand(bidder)
        non_trump_count = sum(1 for c in bidder_hand_pre if not is_trump(c, trump))
        required = len(bidder_hand_pre) - 5

        # Drive the discard phase
        for seat in range(4):
            bots[seat].act(state, seat)

        if non_trump_count >= required:
            discarded = state.discarded()
            # discarded contains everyone's discards; for non-bidders L1 drops 0
            assert all(
                not is_trump(c, trump) for c in discarded
            ), f"L1 dropped trump {discarded} despite having enough non-trump"


# -----------------------------------------------------------------------------
# Tournament harness
# -----------------------------------------------------------------------------

def test_play_game_produces_a_winner():
    bots = [L1Novice(seed=s) for s in range(4)]
    result = play_game(bots, game_seed=1)
    assert result.winner in (0, 1)
    assert result.hands_played > 0
    # One team must have at least one set or at least target_score.
    assert (
        result.final_scores[result.winner] >= 120
        or result.sets[1 - result.winner] >= 3
    )


def test_self_play_is_balanced():
    """L1 vs L1 in 4 identical seats. Across 200 games we expect a roughly
    even split — far enough from 50/50 would suggest a seat-ordering bug."""
    bots = [L1Novice(seed=s) for s in range(4)]
    result = run_tournament(bots, n_games=200, base_seed=0)
    # Loose bound — the test is flaky-resistant, not statistical-power-tuned.
    assert 0.30 <= result.team0_winrate <= 0.70, str(result)


def test_tournament_is_deterministic_under_same_seeds():
    """Same base_seed + same strategy seeds → identical aggregate."""
    bots1 = [L1Novice(seed=s) for s in range(4)]
    bots2 = [L1Novice(seed=s) for s in range(4)]
    r1 = run_tournament(bots1, n_games=20, base_seed=99)
    r2 = run_tournament(bots2, n_games=20, base_seed=99)
    assert r1.team_wins == r2.team_wins
    assert r1.avg_score == r2.avg_score


# -----------------------------------------------------------------------------
# Conftest: skip module if the extension didn't build
# -----------------------------------------------------------------------------

def test_module_imports():
    assert GameState is not None
    assert L1Novice is not None
