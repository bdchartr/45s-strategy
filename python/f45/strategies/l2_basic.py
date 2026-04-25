"""L2 Basic — partner-aware, cost-minimizing trick play.

L2 inherits L1's bidding (suit-count threshold) and L1's discard logic
(keep top-5 by strength, prefer non-trump in the discard pile). It
diverges in two trick-play behaviors and one trump-declaration tweak:

# Partner-winning detection

When it's your turn to follow a trick and your partner is already
winning it, dumping your highest-strength card is wasteful — partner
wins anyway, so you'd rather save the strong card for a future trick.
L2 plays the **weakest** legal card when partner is winning.

# Play minimum to win

When an opponent is winning the trick, L1 plays its highest-strength
card unconditionally. That throws bowers at trash trumps: leading 2T
followed by JT (185) followed by 5T (199) burns the 5T to win a 2-trick.
L2 instead plays the **cheapest legal card that beats the current
winner**, falling back to the cheapest legal card if no card can win.
Combined with partner-awareness, this is the dominant lever in L2 —
ablation shows it carries the win-rate over the L1+5% bar.

# Strength-weighted trump selection

L1 declares the suit with the most cards. L2 declares the suit that
**maximizes the bidder's expected hand strength** if it were trump.
`card_strength(c, T)` encodes this — trumps score in the 100s, top
trumps in the 190s. Summing strengths picks suits where the bidder
holds bowers (5/J/AH), even when those suits are short.

Example: a hand of `[5C, JC, 2D, 3D, 4D]` calls Diamonds under L1 (3
cards) but Clubs under L2 — the strength sum is ~393 in Clubs (5C=199,
JC=185, plus tiny off-suit-red diamond ranks) vs ~315 in Diamonds (the
three diamonds become low trumps ~100 each, but 5C and JC are reduced
to off-suit-black ranks summing to ~15). The ~80-point gap matches
intuition: 5C+JC are two of the three top trumps and dominate any
3-card non-bower suit.

Empirically this contributes <1% on its own, but it's a building block
for strength-aware bidding in a later tier.

# Leading

On a lead L2 plays its strongest legal card (matching L1). Smart leads
(low-leads to fish for partner's high cards, drawing trump after a
sweep) are L3's job.

# What L2 still doesn't do

- L2 ignores the play history. No tracking of who's shown out, who's
  void in trump, or which top trumps are still live.
- L2's bidding doesn't use the strength signal — only its trump
  declaration does. (Bidding precedes trump declaration, so the bidder
  doesn't yet know which suit will be trump.) A future tier could
  estimate "best-case trump" during bidding.
"""

from __future__ import annotations

import random
from typing import Iterable

from f45 import GameState, card_strength, is_trump
from f45.strategies.l1_novice import _count_suits, _suit_of


class L2Basic:
    """Partner-aware bot — see module docstring."""

    name = "L2_Basic"

    def __init__(self, seed: int | None = None) -> None:
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
            raise RuntimeError(f"L2Basic asked to act in phase {phase!r}")

    # -------------------------------------------------------------------------
    # Bidding (unchanged from L1 — see l1_novice.py for rationale)
    # -------------------------------------------------------------------------

    def _bid(self, state: GameState, seat: int) -> None:
        hand = state.hand(seat)
        counts = _count_suits(hand)
        candidate = max(counts.values()) * 5
        if candidate not in (15, 20, 25):
            state.bid(seat, 0)
            return
        current = state.current_bid()
        is_dealer = seat == state.dealer()
        if current is None:
            ok = True
        else:
            _, amount = current
            ok = candidate >= amount if is_dealer else candidate > amount
        state.bid(seat, candidate if ok else 0)

    # -------------------------------------------------------------------------
    # Trump declaration — strength-weighted
    # -------------------------------------------------------------------------

    def _declare(self, state: GameState, seat: int) -> None:
        hand = state.hand(seat)
        # For each candidate trump, compute total hand strength under that
        # trump. Higher = better. Tie-break by C<D<H<S so this remains
        # deterministic and matches L1's tiebreak shape.
        scored: list[tuple[int, str]] = []
        for trump in "CDHS":
            total = sum(card_strength(c, trump) for c in hand)
            scored.append((total, trump))
        # `max` picks the first item among ties, so the C<D<H<S order
        # matters only for tied scores.
        best = max(scored, key=lambda kv: (kv[0], -ord(kv[1])))[1]
        # The negative ord trick reverses tie order: with equal scores,
        # smaller suit char wins (C < D < H < S).
        state.declare_trump(seat, best)

    # -------------------------------------------------------------------------
    # Discard (unchanged from L1)
    # -------------------------------------------------------------------------

    def _discard(self, state: GameState, seat: int) -> None:
        hand = state.hand(seat)
        if len(hand) <= 5:
            state.discard(seat, [])
            return
        trump = state.trump()
        assert trump is not None
        scored = sorted(hand, key=lambda c: card_strength(c, trump))
        required = len(hand) - 5
        non_trump = [c for c in scored if not is_trump(c, trump)]
        trump_cards = [c for c in scored if is_trump(c, trump)]
        if len(non_trump) >= required:
            to_drop = non_trump[:required]
        else:
            to_drop = non_trump + trump_cards[: required - len(non_trump)]
        state.discard(seat, to_drop)

    # -------------------------------------------------------------------------
    # Trick play — partner-aware, cost-minimizing
    # -------------------------------------------------------------------------

    def _play(self, state: GameState, seat: int) -> None:
        legal = state.legal_plays(seat)
        if not legal:
            raise RuntimeError(f"no legal plays for seat {seat}")
        trump = state.trump()
        assert trump is not None

        current_trick = state.current_trick()
        if not current_trick:
            # Leading — play strongest legal (L1 behavior; L3's job to lead smart).
            choice = self._pick(legal, trump, prefer_low=False)
            state.play(seat, choice)
            return

        partner = seat ^ 2  # 4-player: seats 0/2 partner, 1/3 partner
        winning_seat = state.current_winning_seat()
        assert winning_seat is not None  # current_trick is non-empty

        if winning_seat == partner:
            # Partner is winning → save strong cards; play weakest.
            choice = self._pick(legal, trump, prefer_low=True)
        else:
            # Opponent is winning → cheapest card that still beats them; else cheapest.
            winning_card = next(c for s, c in current_trick if s == winning_seat)
            lead_card = current_trick[0][1]
            winners = self._beating_cards(legal, winning_card, lead_card, trump)
            if winners:
                choice = self._pick(winners, trump, prefer_low=True)
            else:
                choice = self._pick(legal, trump, prefer_low=True)

        state.play(seat, choice)

    def _pick(self, cards: Iterable[str], trump: str, *, prefer_low: bool) -> str:
        scored = [(card_strength(c, trump), c) for c in cards]
        target = min(s for s, _ in scored) if prefer_low else max(s for s, _ in scored)
        candidates = [c for s, c in scored if s == target]
        return candidates[0] if len(candidates) == 1 else self._rng.choice(candidates)

    def _beating_cards(
        self,
        legal: Iterable[str],
        winning_card: str,
        lead_card: str,
        trump: str,
    ) -> list[str]:
        """Return cards from `legal` that would top the current winning card.

        45s trick logic: trump beats non-trump; among trumps, higher strength
        wins; among non-trumps both following lead, higher strength wins;
        a non-trump that doesn't follow lead can't win. AH is always trump
        (`is_trump` handles that).
        """
        win_is_trump = is_trump(winning_card, trump)
        win_strength = card_strength(winning_card, trump)
        lead_is_trump = is_trump(lead_card, trump)
        # If the lead is non-trump, "follow lead" means matching the natural
        # suit of the lead card. (If lead is trump, the winner is necessarily
        # trump too, and we go through the win_is_trump branch only.)
        lead_natural_suit = _suit_of(lead_card)

        out: list[str] = []
        for c in legal:
            c_is_trump = is_trump(c, trump)
            if win_is_trump:
                # Only a higher trump beats a trump.
                if c_is_trump and card_strength(c, trump) > win_strength:
                    out.append(c)
            else:
                # Winner is a non-trump following the (non-trump) lead.
                if c_is_trump:
                    out.append(c)  # any trump beats any non-trump
                elif (
                    not lead_is_trump
                    and _suit_of(c) == lead_natural_suit
                    and card_strength(c, trump) > win_strength
                ):
                    out.append(c)
        return out
