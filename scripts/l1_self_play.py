"""Stage 1 sanity check: L1 vs L1 self-play.

In a fair fight (same strategy at all four seats) we expect:

- Team win-rate close to 50/50 across many games.
- Games terminate (we don't hit `max_hands`).
- Scores accumulate sensibly — neither team should average wildly negative
  (which would suggest both sides are setting themselves on every hand).

Run:

    python scripts/l1_self_play.py [num_games]
"""

from __future__ import annotations

import sys
import time

from f45.strategies import L1Novice
from f45.tournament import run_tournament


def main() -> None:
    n_games = int(sys.argv[1]) if len(sys.argv) > 1 else 200

    # Distinct seeds per seat keep the four bots from making identical
    # tiebreak choices, which would correlate them.
    strategies = [L1Novice(seed=s) for s in range(4)]

    t0 = time.perf_counter()
    result = run_tournament(strategies, n_games=n_games, base_seed=42)
    elapsed = time.perf_counter() - t0

    print(result)
    print(
        f"  ({elapsed:.2f}s, {n_games / elapsed:.1f} games/sec, "
        f"{result.avg_hands_per_game * n_games / elapsed:.0f} hands/sec)"
    )

    # Sanity: in self-play, one team consistently winning > 60% would suggest
    # a seat-position advantage we should investigate.
    wr = result.team0_winrate
    if wr < 0.30 or wr > 0.70:
        print(
            f"  ⚠  team0 win-rate {wr:.1%} — outside expected 30–70% band; "
            f"investigate seat asymmetry."
        )
    else:
        print(f"  ✅ team0 win-rate {wr:.1%} within self-play sanity band.")


if __name__ == "__main__":
    main()
