"""45s card game engine — Python-facing package.

The heavy lifting lives in the Rust extension module ``f45._engine``.
This Python layer keeps a stable import surface and provides small
conveniences where the Rust surface is awkward (see future helpers).
"""

from f45._engine import GameState, hello

__all__ = ["GameState", "hello"]
