"""Tournament harness — run games between strategies and aggregate results.

# Why drive games here, not in Rust

Game *driving* (asking each strategy what to do) is a Python-side concern:
strategies live in Python, and we want the mental model "harness asks
strategy, strategy mutates state" to be inspectable from a Python REPL.
Stage 4's self-play loop will probably push the inner loop down to Rust
once strategies are NN-evaluations rather than handwritten heuristics.

# Game vs hand

A *game* runs until one team reaches `target_score` (default 120) via a
made bid OR another team takes 3 sets. A *hand* is one deal+play cycle.
Each hand has a separate seed so a tournament can be deterministic.

# Why per-hand seeds

`GameState.next_hand(seed)` re-deals from a fresh ChaCha8 stream. We
derive each hand's seed from the game seed + hand index. A tournament
of N games uses game_seed = base_seed + game_index; within a game, hand
seed = (game_seed * MULT) ^ hand_index. The mixing keeps two games with
seeds 1 and 2 from sharing any hand seed.
"""

from __future__ import annotations

import dataclasses

from f45 import GameState
from f45.strategies import Strategy


# Mixing constant for derived hand seeds. Anything large and odd works;
# this is splitmix64's golden ratio constant. Avoids correlation between
# adjacent game seeds while keeping the derivation deterministic.
_HAND_SEED_MIX = 0x9E3779B97F4A7C15


def _hand_seed(game_seed: int, hand_index: int) -> int:
    """Derive a per-hand seed from a game seed + hand index. Output fits in 64 bits."""
    return (game_seed * _HAND_SEED_MIX ^ hand_index) & 0xFFFF_FFFF_FFFF_FFFF


@dataclasses.dataclass
class GameResult:
    """Outcome of a single game."""

    winner: int  # 0 or 1 — winning team
    final_scores: tuple[int, int]
    sets: tuple[int, int]
    hands_played: int


@dataclasses.dataclass
class TournamentResult:
    """Aggregate of N games between the same four-strategy lineup."""

    n_games: int
    team_wins: tuple[int, int]
    avg_score: tuple[float, float]
    avg_sets: tuple[float, float]
    avg_hands_per_game: float

    @property
    def team0_winrate(self) -> float:
        return self.team_wins[0] / self.n_games if self.n_games else 0.0

    @property
    def team1_winrate(self) -> float:
        return self.team_wins[1] / self.n_games if self.n_games else 0.0

    def __str__(self) -> str:
        t0, t1 = self.team_wins
        return (
            f"Tournament({self.n_games} games): "
            f"team0 {t0} ({self.team0_winrate:.1%}), "
            f"team1 {t1} ({self.team1_winrate:.1%}), "
            f"avg score {self.avg_score[0]:.1f}/{self.avg_score[1]:.1f}, "
            f"avg sets {self.avg_sets[0]:.2f}/{self.avg_sets[1]:.2f}, "
            f"avg hands/game {self.avg_hands_per_game:.1f}"
        )


def play_hand(state: GameState, strategies: list[Strategy]) -> None:
    """Drive a single hand from current state to HandComplete.

    Caller is responsible for advancing to the next hand via
    `state.next_hand(seed)` — `play_hand` only consumes the current deal.
    """
    n = state.num_players()
    while state.phase() not in ("hand_complete", "game_over"):
        phase = state.phase()
        if phase == "discard":
            # No to_act — every seat decides in seat order.
            for seat in range(n):
                strategies[seat].act(state, seat)
        elif phase == "dealer_extra_draw":
            # 6p only. Dealer picks up the deck remainder and discards back to 5.
            seat = state.to_act()
            strategies[seat].act(state, seat)
        else:
            seat = state.to_act()
            if seat is None:
                raise RuntimeError(f"no to_act in phase {phase!r}")
            strategies[seat].act(state, seat)


def play_game(
    strategies: list[Strategy],
    game_seed: int,
    *,
    num_players: int = 4,
    target_score: int = 120,
    enable_30_for_60: bool = True,
    max_hands: int = 200,
) -> GameResult:
    """Play one full game between the given strategies (length = num_players).

    Returns a `GameResult` once a team wins. `max_hands` is a runaway-game
    safety belt — under the rules, games terminate; this guards against
    bugs that could loop forever.
    """
    if len(strategies) != num_players:
        raise ValueError(
            f"need {num_players} strategies, got {len(strategies)}"
        )

    state = GameState(
        seed=_hand_seed(game_seed, 0),
        num_players=num_players,
        dealer=0,
        target_score=target_score,
        enable_30_for_60=enable_30_for_60,
    )

    hand_index = 0
    while state.phase() != "game_over" and hand_index < max_hands:
        play_hand(state, strategies)
        if state.phase() == "game_over":
            break
        hand_index += 1
        state.next_hand(_hand_seed(game_seed, hand_index))

    if state.phase() != "game_over":
        raise RuntimeError(f"game did not terminate within {max_hands} hands")

    winner = state.winner()
    assert winner is not None, "winner must be set when phase=game_over"

    return GameResult(
        winner=winner,
        final_scores=tuple(state.scores()),
        sets=state.sets(),
        hands_played=state.hands_played(),
    )


def run_tournament(
    strategies: list[Strategy],
    n_games: int,
    *,
    base_seed: int = 0,
    num_players: int = 4,
    target_score: int = 120,
    enable_30_for_60: bool = True,
) -> TournamentResult:
    """Play `n_games` between the same lineup; aggregate wins by team."""
    wins = [0, 0]
    score_sum = [0, 0]
    set_sum = [0, 0]
    hand_sum = 0

    for i in range(n_games):
        result = play_game(
            strategies,
            game_seed=base_seed + i,
            num_players=num_players,
            target_score=target_score,
            enable_30_for_60=enable_30_for_60,
        )
        wins[result.winner] += 1
        score_sum[0] += result.final_scores[0]
        score_sum[1] += result.final_scores[1]
        set_sum[0] += result.sets[0]
        set_sum[1] += result.sets[1]
        hand_sum += result.hands_played

    return TournamentResult(
        n_games=n_games,
        team_wins=(wins[0], wins[1]),
        avg_score=(score_sum[0] / n_games, score_sum[1] / n_games),
        avg_sets=(set_sum[0] / n_games, set_sum[1] / n_games),
        avg_hands_per_game=hand_sum / n_games,
    )
