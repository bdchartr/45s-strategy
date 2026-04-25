"""L1 Novice — naive heuristics, ported from the PHP AlgorithmicMoveProvider.

This is the **floor** of the strategy ladder: every later tier must beat
this one to justify its existence. It's a port (not a copy) of the bot
shipped with the production wkapp-45 PHP codebase, adjusted only where
PHP took a buggy or impossible path under Rust's stricter validation.

# Heuristics

- **Bid:** count cards per suit; bid `max_suit_count * 5` if that lands
  on a legal value (15/20/25) and beats the current bid (dealer can
  match to steal; non-dealer must strictly exceed). Otherwise pass.
  Never bids 30 or 60.

- **Trump:** declare the suit with the most cards in hand; ties broken
  C < D < H < S (PHP's `arsort` is stable in this order on the count
  dict we build).

- **Discard:** keep the 5 strongest cards by `card_strength`. Trump-keeper
  rule: when possible, prefer non-trump cards in the discard pile (don't
  weaken the bid).

- **Play:** play the legal card with the highest strength; random
  tiebreak among equally-strong cards. (For Stage 1 reproducibility,
  the tiebreak RNG is per-instance and seedable.)

# What L1 does *not* do (and L2/L3 will)

- L1 has no concept of partner. It will trump its partner's winning
  trick (a classic novice mistake).
- L1 ignores the play history. It will lead an Ace into a trump void
  even if everyone has shown out of the suit.
- L1's bidding is purely about hand composition — it doesn't bluff,
  bid for control, or model opponents.
"""

from __future__ import annotations

import random
from typing import Iterable

from f45 import GameState
from f45._engine import card_strength, is_trump


def _suit_of(card_code: str) -> str:
    """Return the suit character of a card code. AH is Hearts."""
    return card_code[-1].upper()


def _count_suits(hand: Iterable[str]) -> dict[str, int]:
    counts = {"C": 0, "D": 0, "H": 0, "S": 0}
    for code in hand:
        s = _suit_of(code)
        if s in counts:
            counts[s] += 1
    return counts


class L1Novice:
    """Naive 45s bot — see module docstring for heuristics."""

    name = "L1_Novice"

    def __init__(self, seed: int | None = None) -> None:
        # Per-instance RNG for play tiebreaks. Seeding keeps tournaments
        # reproducible; None falls back to OS entropy.
        self._rng = random.Random(seed)

    # -------------------------------------------------------------------------
    # Phase dispatch
    # -------------------------------------------------------------------------

    def act(self, state: GameState, seat: int) -> None:
        phase = state.phase()
        if phase == "bidding":
            self._bid(state, seat)
        elif phase == "declare_trump":
            self._declare(state, seat)
        elif phase == "discard":
            self._discard(state, seat)
        elif phase == "play":
            self._play(state, seat)
        else:
            raise RuntimeError(f"L1Novice asked to act in phase {phase!r}")

    # -------------------------------------------------------------------------
    # Bidding
    # -------------------------------------------------------------------------

    def _bid(self, state: GameState, seat: int) -> None:
        hand = state.hand(seat)
        counts = _count_suits(hand)
        max_count = max(counts.values())
        candidate = max_count * 5

        # Legal bid amounts at the rules layer: 15, 20, 25, 30, (60).
        # max_count of 1..5 yields 5/10/15/20/25 — only the last three legal.
        legal_amounts = {15, 20, 25}
        if candidate not in legal_amounts:
            state.bid(seat, 0)  # pass
            return

        current = state.current_bid()
        is_dealer = seat == state.dealer()
        # Non-dealer must strictly exceed; dealer may match (the "hold").
        if current is None:
            ok = True
        else:
            _, amount = current
            ok = candidate >= amount if is_dealer else candidate > amount

        state.bid(seat, candidate if ok else 0)

    # -------------------------------------------------------------------------
    # Trump declaration
    # -------------------------------------------------------------------------

    def _declare(self, state: GameState, seat: int) -> None:
        counts = _count_suits(state.hand(seat))
        # `max(items, key=...)` is stable: with equal counts, the suit that
        # appears first in iteration order wins. Python dict insertion order
        # is C, D, H, S — matching PHP's tiebreak.
        best = max(counts.items(), key=lambda kv: kv[1])[0]
        state.declare_trump(seat, best)

    # -------------------------------------------------------------------------
    # Discard
    # -------------------------------------------------------------------------

    def _discard(self, state: GameState, seat: int) -> None:
        hand = state.hand(seat)
        if len(hand) <= 5:
            # Non-bidder in 4p, or 6p non-bidder who chose nothing. No-op.
            state.discard(seat, [])
            return

        trump = state.trump()
        assert trump is not None, "trump must be declared before discard"

        # Sort weakest first. Drop the first three weakest, but prefer
        # non-trump cards (the trump-keeper rule applied as a preference).
        scored = sorted(hand, key=lambda c: card_strength(c, trump))
        required = len(hand) - 5
        non_trump = [c for c in scored if not is_trump(c, trump)]
        trump_cards = [c for c in scored if is_trump(c, trump)]

        if len(non_trump) >= required:
            to_drop = non_trump[:required]
        else:
            # Have to drop some trump too. Drop all non-trump first, then
            # the weakest trump cards to fill the quota.
            to_drop = non_trump + trump_cards[: required - len(non_trump)]

        state.discard(seat, to_drop)

    # -------------------------------------------------------------------------
    # Trick play
    # -------------------------------------------------------------------------

    def _play(self, state: GameState, seat: int) -> None:
        legal = state.legal_plays(seat)
        if not legal:
            raise RuntimeError(f"no legal plays for seat {seat}")
        trump = state.trump()
        assert trump is not None

        scored = [(card_strength(c, trump), c) for c in legal]
        max_strength = max(s for s, _ in scored)
        best = [c for s, c in scored if s == max_strength]
        choice = best[0] if len(best) == 1 else self._rng.choice(best)
        state.play(seat, choice)
