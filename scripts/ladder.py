"""Ladder verification: each tier should beat the previous by ≥55%.

Stage 1 success criterion: L2 beats L1 ≥55%, L3 beats L2 ≥55% over 10k
games. This script runs the matchups and prints win-rates.

Layout: tier-A occupies team 0 (seats 0, 2); tier-B occupies team 1
(seats 1, 3). Two tiers per game, four bots.

Run:

    python scripts/ladder.py [num_games]

Default num_games=10000 (the success threshold). Set to 1000 for a
quick check during development.
"""

from __future__ import annotations

import sys
import time

from f45.strategies import L1Novice, L2Basic
from f45.tournament import run_tournament


def head_to_head(name: str, team0_factory, team1_factory, n_games: int) -> None:
    """Run team0 (seats 0,2) vs team1 (seats 1,3) for `n_games`. Print result."""
    bots = [team0_factory(0), team1_factory(1), team0_factory(2), team1_factory(3)]
    t0 = time.perf_counter()
    result = run_tournament(bots, n_games=n_games, base_seed=0)
    elapsed = time.perf_counter() - t0
    team0_label, team1_label = name.split(" vs ")
    print(f"{name} ({n_games} games):")
    print(f"  {team0_label}: {result.team_wins[0]} ({result.team0_winrate:.1%})")
    print(f"  {team1_label}: {result.team_wins[1]} ({result.team1_winrate:.1%})")
    print(f"  avg score {result.avg_score[0]:.1f}/{result.avg_score[1]:.1f}, "
          f"avg sets {result.avg_sets[0]:.2f}/{result.avg_sets[1]:.2f}, "
          f"avg hands/game {result.avg_hands_per_game:.1f}")
    print(f"  ({elapsed:.1f}s, {n_games / elapsed:.0f} games/sec)")


def main() -> None:
    n_games = int(sys.argv[1]) if len(sys.argv) > 1 else 10_000

    # L1 vs L1 sanity (should be ~50/50)
    head_to_head(
        "L1 vs L1",
        lambda s: L1Novice(seed=s),
        lambda s: L1Novice(seed=100 + s),
        n_games,
    )
    print()

    # L2 vs L1 — the headline matchup
    head_to_head(
        "L2 vs L1",
        lambda s: L2Basic(seed=s),
        lambda s: L1Novice(seed=100 + s),
        n_games,
    )
    print()

    # L1 vs L2 — flip seats to control for seat-position effects
    head_to_head(
        "L1 vs L2",
        lambda s: L1Novice(seed=s),
        lambda s: L2Basic(seed=100 + s),
        n_games,
    )


if __name__ == "__main__":
    main()
