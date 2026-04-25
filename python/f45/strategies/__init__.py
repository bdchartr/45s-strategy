"""45s strategies — rule-based baselines and (later) learned policies.

Each strategy implements the `Strategy` protocol from `base`. Strategies
mutate a `GameState` directly via the PyO3 boundary; the tournament
harness (`f45.tournament`) drives a game by asking each seat's strategy
to act when it's that seat's turn.
"""

from f45.strategies.base import Strategy
from f45.strategies.l1_novice import L1Novice
from f45.strategies.l2_basic import L2Basic

__all__ = ["Strategy", "L1Novice", "L2Basic"]
