# TODO

## Current focus

**Stage 0 — Rust game engine with PyO3 bindings.** Nothing is implemented yet. Next concrete action is sketching the `GameState` struct and deciding on the `GameView` (per-player visible state) representation.

## Status legend

`[ ]` pending · `[~]` in progress · `[x]` done · `[!]` blocked

## Stage 0 — Headless game engine (Rust + PyO3)

Goal: deterministic, fast, rules-accurate engine callable from Python.

- [ ] Scaffold Rust crate (`engine/`) and Python package (`strategy/`) layout
- [ ] Define core types: `Card`, `Suit`, `Rank`, `Seat`, `Trick`, `GameState`, `Move`
- [ ] Implement deal, bid, trump declare, discard, play phases
- [ ] Implement trick resolution with bowers + reneging rules
- [ ] Implement Chartrand 6-player variant as a config flag
- [ ] Define `GameView` (per-seat visible state including event log)
- [ ] PyO3 bindings: `GameState`, `step(move)`, `legal_moves()`, `view_for(seat)`
- [ ] Rust unit tests for each phase + trick resolution edge cases
- [ ] Property test harness: run 10k random-play games through both PHP and Rust, compare final state hashes
- [ ] Benchmark: single-threaded games/sec (target ≥10k), multi-core throughput (target ≥100k)

**Pending decisions:**
- Event log format: port PHP's `event_type` + `payload` JSON, or design fresh typed Rust enum? *Leaning: typed enum, convertible to PHP format for property tests.*
- Random state: per-game `SmallRng` seed, or shared? *Leaning: per-game for reproducibility.*

## Stage 1 — Rule-based baselines

Goal: 3 hand-crafted strategy tiers + tournament harness. These are permanent evaluation opponents for all later stages.

- [ ] `Strategy` trait/protocol defined in Python (Rust-side optional)
- [ ] `L1_Novice`: highest legal card, threshold-based bidding (port from `AlgorithmicMoveProvider.php`)
- [ ] `L2_Basic`: + partner-winning detection → dump low; smarter trump selection
- [ ] `L3_Counter`: + tracks trump/aces played; avoids wasting high cards
- [ ] Tournament harness: round-robin N games, output win-rate matrix + score differentials
- [ ] Parallel harness via `multiprocessing.Pool`
- [ ] Verify ladder: L2 beats L1 ≥55%, L3 beats L2 ≥55% over 10k games

## Stage 2 — MCTS baseline

Goal: pure MCTS with random rollouts, parallelized. No neural net.

- [ ] Information-set MCTS (IS-MCTS) for imperfect information — review algorithm, document choice
- [ ] Determinization strategy for hidden hands (uniform sample consistent with observations?)
- [ ] Single-threaded MCTS player, tuned iteration budget
- [ ] Leaf parallelism implementation
- [ ] Root parallelism implementation
- [ ] Benchmark: scaling curves for both vs P-core count
- [ ] Verify: MCTS beats L3 ≥60%

## Stage 3 — Supervised policy network

Goal: small NN that imitates MCTS, at ≥100x inference speed.

- [ ] Position featurization: design input tensor (own hand, visible play history, bids, trump, seat)
- [ ] Generate MCTS game dataset (1M positions?)
- [ ] MLX model + training loop
- [ ] PyTorch MPS model + training loop (same architecture)
- [ ] Benchmark: training throughput, inference latency, memory, on both
- [ ] Document findings — this is the MLX vs PyTorch writeup
- [ ] Verify: policy move-agreement with MCTS ≥80%

## Stage 4 — Self-play RL (AlphaZero-lite)

Goal: policy-guided MCTS + self-play learning loop.

- [ ] Actor-learner architecture sketch (workers + replay buffer + learner)
- [ ] Shared memory replay buffer (Ray or `multiprocessing.shared_memory`)
- [ ] Weight-broadcast mechanism (how frequently, how atomically)
- [ ] Batched inference server for MCTS leaf evaluation
- [ ] Checkpoint format + resumption
- [ ] Training run: ≥48h self-play, monitor win rate vs Stage 3 snapshot
- [ ] Verify: cold-start trained policy beats Stage 3 ≥55%

## Stage 5 — Research questions

Pick at least 2. Write up each with methodology, results, plots.

- [ ] Does the policy discover partner-signaling conventions? (Analyze correlations between partner's lead and own subsequent play.)
- [ ] Can we train a "level slider" — one net conditioned on a difficulty parameter that plays at configurable strength?
- [ ] Opponent modeling: does conditioning on opponent play-history (not just current game) improve win rate?
- [ ] Transfer: does a 4-player trained policy play the 6-player Chartrand variant competently?
- [ ] Search budget tradeoff: win rate as a function of MCTS iterations at inference time.

## Known risks

- **Rule drift between Rust and PHP.** Mitigation: property-test harness every CI run (once we have CI).
- **MCTS determinization is subtle** for imperfect information. Getting this wrong produces a bot that plays as if hands are public. Budget time for reading IS-MCTS and Perfect Information Monte Carlo literature.
- **Stage 4 debugging is notoriously hard.** Diverging losses, stale weights, reward hacking. Invest in monitoring/logging infrastructure before the long training run.
- **MLX is young.** May hit undocumented bugs or missing ops. PyTorch MPS is the fallback.

## Stage retrospectives

(Append a short retro after each stage completes. These accumulate — do not delete.)

---

_Nothing yet — project just initialized 2026-04-24._
