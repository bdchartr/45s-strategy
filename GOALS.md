# Project Goals

## Mission

Develop progressively stronger AI strategies for 45s via self-play reinforcement learning, using the project as a hands-on research vehicle for massive parallel compute on Apple Silicon (M5 Pro).

Two equal-weight outcomes:
1. **Artifacts**: a ladder of 45s strategies from naive rule-based to learned, plus the infrastructure to keep improving them.
2. **Learning**: direct experience with Rust/Python interop, actor-learner RL architectures, MLX vs PyTorch MPS, MCTS parallelism, and profiling real workloads on Apple Silicon.

## Non-goals

- **Production deployment.** Strategies will eventually port back to `wkapp-45`, but that's a later engineering task, not part of this research.
- **Generalization to other card games.** 45s only. Hardcode rules where it simplifies.
- **Multi-machine distribution.** Everything runs on one M5 Pro. No Kubernetes, no cloud.
- **Human-readable learned strategies.** Neural policies don't need to be explainable. Only hand-crafted tiers need to be.
- **Beating humans is not the bar.** Beating the previous tier is the bar.

## Success criteria

Graded by stage (see `TODO.md` for the staged plan):

- **Stage 0 (engine)**: Rust engine matches PHP behavior on 10k+ property-test games. Throughput ≥10k games/sec single-threaded, ≥100k games/sec across all P-cores.
- **Stage 1 (baselines)**: 3 rule-based tiers implemented. L2 beats L1 ≥55%, L3 beats L2 ≥55%, in 10k-game tournaments. These become permanent evaluation opponents.
- **Stage 2 (MCTS)**: Pure MCTS with random rollouts beats L3 ≥60%. Benchmark tree-parallel vs root-parallel scaling curves.
- **Stage 3 (supervised policy)**: Small NN trained on MCTS games approximates MCTS at ≥80% move agreement, at ≥100x inference speedup.
- **Stage 4 (self-play RL)**: AlphaZero-lite loop produces a policy that beats Stage 3 from cold start in <48 hours of self-play.
- **Stage 5 (research questions)**: at least two well-documented experiments beyond baseline (e.g., partner-signaling emergence, level-conditioned policy, opponent modeling).

## Stack decisions

| Layer | Choice | Rationale |
|---|---|---|
| Game engine | Rust + PyO3 | 50-500x speed over PHP for simulation; PyO3 is itself a valuable learning target; clean separation from Python orchestration. |
| Orchestration | Python 3.12+ | ML ecosystem lives here. Standard for RL research. |
| Parallel sim | `multiprocessing` first, Ray if needed | Start simple. Ray adds value only when we hit IPC/state-sharing limits. |
| Neural nets | MLX primary, PyTorch MPS fallback | MLX uses unified memory natively (no CPU↔GPU copies), designed for Apple Silicon. Benchmark both in Stage 3. |
| MCTS | Custom Python, with hot-loop candidates for Rust port | Start readable, move to Rust only when profiling demands it. |
| Testing | Rust `cargo test` + Python `pytest` + cross-engine property tests | Property tests against PHP engine catch rule drift. |

## Constraints

- **Hardware**: single M5 Pro Mac. All design decisions assume unified memory, Apple Silicon, no external GPUs.
- **Solo developer**. No team conventions needed; optimize for fast iteration and clear context for future Claude sessions.
- **No budget pressure but no cloud.** Experiments run overnight locally; can't scale to 1000 machines.

## Relationship to `wkapp-45`

- `wkapp-45` is the source of truth for game rules.
- This repo produces strategies that may later be ported back (rule-based → PHP port is trivial; neural nets → ONNX + inference microservice).
- No runtime dependency between the two repos. Rules are replicated, not linked.

## Decision log

Record non-obvious decisions here with date and rationale. Format:

`YYYY-MM-DD — Decision. Why.`

- 2026-04-24 — **Greenfield research repo, not subdir of `wkapp-45`.** Keeps production code path clean; allows research repo to have its own stack (Rust + Python) without polluting the PHP project.
- 2026-04-24 — **Rust engine over pure Python.** Speed matters at Stage 4 (millions of self-play games). Rust + PyO3 also serves the learning goal.
- 2026-04-24 — **AlphaZero-lite target, not pure rule-based evolution.** User framed this as a research project with M5 Pro compute available. Rule-based evolution via LLM would not exercise the hardware and would not produce strong strategies.
- 2026-04-24 — **MLX primary over PyTorch MPS.** MLX's unified-memory design matches the hardware; benchmark comparison is itself a Stage 3 deliverable.
- 2026-04-24 — **PyO3 0.28 pinned (not 0.23).** Python 3.14 is the system interpreter; PyO3 0.23 caps at 3.13 and fails the build with a clear error. 0.28 supports 3.14. If a future session hits a build failure citing Python version, check the PyO3 cap first.
- 2026-04-24 — **ChaCha8 for shuffle, not PHP Mersenne Twister.** Portable across platforms, deterministic from a u64 seed, fast. Consequence: Rust↔PHP golden-corpus tests cannot compare shuffle outputs directly — they must start from a pre-specified deck via a `new_hand_from_deck`-style constructor (to be added in C5). PHP compatibility was never a goal; deterministic self-play was.
- 2026-04-24 — **Stringly-typed Python API for Stage 0** (`"AH"`, `"10D"` card codes and suit chars like `"S"`). Slower than integer encoding but clearer while the engine shape is still iterating. If Stage-4 profiling shows the FFI boundary is the bottleneck, add an int-typed API alongside — don't replace. Rationale recorded in `src/bindings.rs` module docs.
- 2026-04-24 — **30-for-60 implements the correct rule, not the PHP bug.** PHP stores `bid_value = 60` for a 30-for-60 bid and then checks `bidder_pts >= 60` to decide "made", but max hand points is 30 — so the bid is structurally unmakeable in PHP. Rust's `score_hand` checks `bidder_pts >= 30` (sweep = made) and applies a ±60 delta. Divergence is documented inline in `score_hand`. Flag during golden-corpus work (C5).
- 2026-04-24 — **`extension-module` PyO3 feature is opt-in via Cargo feature.** Unconditional `features = ["extension-module"]` defers all Python symbols to runtime, which breaks `cargo test` integration-test binaries (they compile as regular executables and fail to link). Maturin enables the feature for wheel builds via `pyproject.toml`; `cargo test` compiles without it and links against `libpython`. Pattern: reusable for any PyO3 + Rust-tested project.
- 2026-04-24 — **Engine tracks `discarded: Vec<Card>`.** Strictly for invariant checking and future AI observability (the engine does not read it back). Needed for clean card-conservation proptest. Costs a Vec per hand; cleared on `deal_hand`.
- 2026-04-24 — **Round-robin deal order in Rust, matching PHP.** `deal_hand` deals 5 rounds × n seats, then 3-card kitty, then remainder — same order as `wkapp-45/app/Application/Services/GameRuntimeService.php::dealNewHand()`. Originally Rust dealt contiguous blocks; no existing test depended on specific dealt cards, so the switch was clean. Rationale: keeps a future deck-replay corpus (Rust replays a PHP-pinned deck and expects identical hands post-deal) forward-compatible.
- 2026-04-24 — **`GameState::from_state(hands, kitty, deck_remainder, config, dealer)` over `new_hand_from_deck([Card; 52], ...)` for authored scenarios.** Corpus tests want to write the hand *state* directly, not reverse-engineer a deck order. `from_state` validates 52-card conservation + no duplicates + per-hand sizes, then produces a fresh `Phase::Bidding` state. Shape is a test/scenario constructor and asserts on invalid input — not a production entry point.
- 2026-04-24 — **Stage-0 scope reduction: hand-authored corpus instead of PHP-replay corpus.** Original success criterion: "Rust engine matches PHP behavior on 10k+ property-test games." Delivered: 5 hand-authored Rust-only scenarios in `tests/golden_corpus.rs`. Rationale: PHP's rule logic is entangled with its DB layer (`GameRuntimeService` threads a `GameRepository` transaction through every evaluation); standalone trace emission is non-trivial extraction work. Deferred to a later engineering task, flagged in `docs/stage-0.md → Deferred` so downstream stages don't assume cross-engine-verified rules.
