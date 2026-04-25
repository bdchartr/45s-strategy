"""Strategy protocol — the interface all 45s players implement.

# Why a Protocol, not an ABC

We use `typing.Protocol` rather than `abc.ABC` because we want *structural*
typing: anything that has the right `name` and `act` attributes IS a
strategy, no inheritance required. This lets researchers prototype
strategies as plain functions wrapped in a tiny adapter, or as full
classes with state — both work without touching the protocol definition.

# Why `act(state, seat)` mutates state

Two API shapes were on the table:

  1. `act(state, seat) -> Action`  → harness applies the action.
  2. `act(state, seat) -> None`    → strategy applies it directly.

We picked (2) for Stage 1 because it's the smallest viable surface — no
Action type to design, no dispatch table in the harness. The downside is
that strategies must know which `state.bid()` / `state.play()` method to
call, which couples them to the engine's mutation API. If we ever want
to record decisions (for replay, training data, opponent modeling), we'll
move to (1) and have the harness record the (state, action) pair.

# Discard phase

During `Phase::Discard`, multiple seats act in parallel — there is no
`to_act`. The harness calls `act(state, seat)` once per seat in seat
order; each strategy is responsible for calling `state.discard(seat, ...)`
exactly once. Non-bidder strategies that don't care can call
`state.discard(seat, [])` — a legal no-op that advances the seat's
discard-done flag.
"""

from __future__ import annotations

from typing import Protocol, runtime_checkable

from f45 import GameState


@runtime_checkable
class Strategy(Protocol):
    """A 45s player. Must declare a name (used in tournament reports) and
    an `act` method that mutates `state` to reflect this seat's chosen
    action for the current phase."""

    name: str

    def act(self, state: GameState, seat: int) -> None:
        ...
