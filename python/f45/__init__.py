"""45s card game engine — Python-facing package.

The heavy lifting lives in the Rust extension module ``f45._engine``.
This Python layer keeps a stable import surface and provides:

- the engine surface (`GameState`, `hello`)
- rule-helper functions (`card_strength`, `is_trump`, `is_top_trump`)
- strategies (`f45.strategies`) and the tournament harness (`f45.tournament`)
"""

from f45._engine import GameState, card_strength, hello, is_top_trump, is_trump

__all__ = [
    "GameState",
    "card_strength",
    "hello",
    "is_top_trump",
    "is_trump",
]
