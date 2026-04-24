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
