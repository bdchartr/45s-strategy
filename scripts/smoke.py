"""End-to-end smoke test for the f45 PyO3 bindings.

Exercises the stringly-typed Python API: creates a 4-player game,
drives through bidding, trump, discard, and all 5 tricks, and asserts
the bookkeeping invariants that the hand preserved.

Run with `python scripts/smoke.py` after `maturin develop`.
"""

from __future__ import annotations

import sys

from f45 import GameState, hello


def all_cards(g: GameState, num_players: int) -> list[str]:
    """Every card the engine is tracking. Must equal a 52-card deck."""
    out: list[str] = []
    for s in range(num_players):
        out.extend(g.hand(s))
    out.extend(g.kitty())
    out.extend(g.deck_remainder())
    out.extend(g.discarded())
    out.extend(c for _, c in g.current_trick())
    for _, _, plays in g.completed_tricks():
        out.extend(c for _, c in plays)
    return out


def assert_conservation(g: GameState, num_players: int, tag: str) -> None:
    cards = all_cards(g, num_players)
    assert len(cards) == 52, f"{tag}: expected 52 tracked cards, got {len(cards)}"
    assert len(set(cards)) == 52, f"{tag}: duplicate cards detected"


def drive_bidding_dealer_takes_15(g: GameState) -> None:
    """Seats 1-3 pass, dealer (seat 0) takes 15."""
    for seat in (1, 2, 3, 0):
        to_act = g.to_act()
        assert to_act == seat, f"expected seat {seat} to act, got {to_act}"
        if seat == 0:
            g.bid(seat, 15)
        else:
            g.bid(seat, 0)  # pass
    assert g.phase() == "declare_trump"
    assert g.bidder() == 0
    wb = g.winning_bid()
    assert wb == (0, 15), f"winning_bid: {wb}"


def pick_declare_suit(g: GameState, bidder: int) -> str:
    """Pick the suit with the longest holding in the bidder's hand."""
    hand = g.hand(bidder)
    counts: dict[str, int] = {}
    for code in hand:
        suit = code[-1]
        counts[suit] = counts.get(suit, 0) + 1
    # Break ties by suit order for determinism.
    return max(counts, key=lambda s: (counts[s], s))


def bidder_discards_non_trump(g: GameState, bidder: int, trump: str) -> None:
    """4-player: bidder dumps all non-trump cards (can be any 0-4 here)."""
    hand = g.hand(bidder)
    drops = [c for c in hand if c[-1] != trump and c != "AH"]
    # 4-player: bidder can discard up to 4 non-trumps.
    drops = drops[:4]
    g.discard(bidder, drops)


def non_bidder_pass_discard(g: GameState, seat: int) -> None:
    g.discard(seat, [])


def play_all_tricks(g: GameState) -> None:
    """Each seat plays the first legal card; this is a smoke test, not strategy."""
    while g.phase() == "play":
        seat = g.to_act()
        assert seat is not None
        legal = g.legal_plays(seat)
        assert legal, f"no legal plays for seat {seat} in play phase"
        g.play(seat, legal[0])


def main() -> int:
    print("hello() =", hello())

    g = GameState(seed=42, num_players=4, dealer=0, target_score=120, enable_30_for_60=True)
    print("initial:", g)
    assert_conservation(g, 4, "after deal")

    # --- Invariant: 52 cards total (4*5 hand + 3 kitty + rest as deck_remainder) ---
    all_hand_cards = sum(len(g.hand(s)) for s in range(4))
    assert all_hand_cards == 4 * 5, f"expected 20 hand cards, got {all_hand_cards}"
    assert len(g.kitty()) == 3, f"expected 3 kitty cards, got {len(g.kitty())}"
    # 20 + 3 = 23, so deck_remainder should hold the other 29.
    assert len(g.deck_remainder()) == 29, (
        f"expected 29 cards remaining in deck, got {len(g.deck_remainder())}"
    )

    # --- Bidding ---
    drive_bidding_dealer_takes_15(g)
    print("after bidding:", g)

    # --- Declare trump ---
    bidder = g.bidder()
    assert bidder == 0
    trump = pick_declare_suit(g, bidder)
    g.declare_trump(bidder, trump)
    assert g.trump() == trump
    assert g.phase() == "discard"
    print(f"trump declared: {trump}")

    # --- Discard phase ---
    # In Discard, `to_act` returns None — every seat submits independently.
    # Bidder must drop enough non-trumps to get back to 5; others can pass.
    # Do the bidder first so the "non-bidder passes" rule holds cleanly.
    bidder_discards_non_trump(g, bidder, trump)
    for seat in range(4):
        if seat == bidder:
            continue
        non_bidder_pass_discard(g, seat)
    assert g.phase() == "play", f"expected play, got {g.phase()}"
    assert_conservation(g, 4, "after discard")
    print("after discard:", g)

    # --- Play ---
    play_all_tricks(g)
    print("after play:", g)

    # --- Post-hand invariants ---
    assert g.phase() in ("hand_complete", "game_over"), f"unexpected phase: {g.phase()}"
    tricks = g.completed_tricks()
    assert len(tricks) == 5, f"expected 5 tricks, got {len(tricks)}"

    # Each trick has exactly 4 plays and one winner.
    for i, (leader, winner, plays) in enumerate(tricks):
        assert len(plays) == 4, f"trick {i}: expected 4 plays, got {len(plays)}"
        assert 0 <= leader < 4
        assert 0 <= winner < 4

    # Hand points sum to 30: 5-per-trick + 5 for high trump in play = 25 + 5.
    pts = g.hand_points()
    assert sum(pts) == 30, f"hand points must sum to 30, got {pts}"
    print(f"hand_points: team0={pts[0]}, team1={pts[1]} (sum=30 ✓)")

    # Scores updated, bid_made decided.
    print(f"scores: {g.scores()}, sets: {g.sets()}, bid_made: {g.bid_made()}")

    # Card conservation: every card from every trick is unique; 20 cards total.
    all_played = [c for _, _, plays in tricks for _, c in plays]
    assert len(all_played) == 20
    assert len(set(all_played)) == 20, "duplicate cards across tricks!"

    # Whole-deck conservation: hands + kitty + deck_remainder + discarded
    #  + in-flight trick + completed tricks must equal 52 unique cards.
    assert_conservation(g, 4, "after hand complete")

    print("\n✅ smoke test passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
