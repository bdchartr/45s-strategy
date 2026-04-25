"""Ablation: which L2 lever (partner-aware play vs strength-weighted trump)
is doing the work?

Builds two hybrids of L1/L2:
- L2_TrumpOnly: L1 + L2's strength-weighted trump declaration
- L2_PartnerOnly: L1 + L2's partner-aware play

Runs each vs L1 over n_games (default 5000), pooled across seat-flip.
"""

from __future__ import annotations

import sys
import time

from f45 import GameState, card_strength
from f45.strategies import L1Novice, L2Basic
from f45.tournament import run_tournament


class L2_TrumpOnly(L1Novice):
    """L1 in everything except trump declaration."""

    name = "L2_TrumpOnly"

    def _declare(self, state: GameState, seat: int) -> None:
        hand = state.hand(seat)
        scored = [(sum(card_strength(c, t) for c in hand), t) for t in "CDHS"]
        best = max(scored, key=lambda kv: (kv[0], -ord(kv[1])))[1]
        state.declare_trump(seat, best)


class L2_PartnerOnly(L1Novice):
    """L1 in everything except partner-aware trick play."""

    name = "L2_PartnerOnly"

    def _play(self, state: GameState, seat: int) -> None:
        legal = state.legal_plays(seat)
        if not legal:
            raise RuntimeError(f"no legal plays for seat {seat}")
        trump = state.trump()
        assert trump is not None

        partner = seat ^ 2
        winning = state.current_winning_seat()
        prefer_low = winning is not None and winning == partner

        scored = [(card_strength(c, trump), c) for c in legal]
        target = min(s for s, _ in scored) if prefer_low else max(s for s, _ in scored)
        candidates = [c for s, c in scored if s == target]
        choice = candidates[0] if len(candidates) == 1 else self._rng.choice(candidates)
        state.play(seat, choice)


def head_to_head(name: str, factory_a, factory_b, n_games: int) -> tuple[int, int]:
    bots = [factory_a(0), factory_b(1), factory_a(2), factory_b(3)]
    result = run_tournament(bots, n_games=n_games, base_seed=0)
    return result.team_wins[0], result.team_wins[1]


def pooled(name: str, factory: callable, n_games: int) -> None:
    """Run factory-vs-L1 in both seat configs, report pooled win-rate."""
    t0 = time.perf_counter()
    a_wins, b_wins = head_to_head(
        f"{name} vs L1",
        lambda s: factory(s),
        lambda s: L1Novice(seed=100 + s),
        n_games,
    )
    c_wins, d_wins = head_to_head(
        f"L1 vs {name}",
        lambda s: L1Novice(seed=200 + s),
        lambda s: factory(300 + s),
        n_games,
    )
    elapsed = time.perf_counter() - t0
    candidate_wins = a_wins + d_wins
    l1_wins = b_wins + c_wins
    total = candidate_wins + l1_wins
    pct = candidate_wins / total
    print(f"{name} vs L1 pooled ({total} games): {candidate_wins}/{l1_wins} = {pct:.2%} "
          f"[seat0,2: {a_wins/(a_wins+b_wins):.2%}, seat1,3: {d_wins/(c_wins+d_wins):.2%}] "
          f"({elapsed:.1f}s)")


def main() -> None:
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 5000

    print(f"# Ablation: which L2 component does the work?  ({n} games per direction)\n")

    pooled("L1_baseline", lambda s: L1Novice(seed=400 + s), n)
    pooled("L2_TrumpOnly", lambda s: L2_TrumpOnly(seed=s), n)
    pooled("L2_PartnerOnly", lambda s: L2_PartnerOnly(seed=s), n)
    pooled("L2_Basic", lambda s: L2Basic(seed=s), n)


if __name__ == "__main__":
    main()
