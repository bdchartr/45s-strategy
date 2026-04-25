# TODO

## Current focus

**Stage 0 complete.** All checkpoints C1–C5 done. Next up: **Stage 1** (rule-based baselines).

### Stage 0 checkpoints

- [x] **C1** — Scaffold + types + ranker + rules. `src/cards.rs`, `src/ranker.rs`, `src/rules.rs`, `src/error.rs` + unit tests. PyO3 hello-world verified.
- [x] **C2** — State machine + deal + score + 4-player play-through test. `src/state.rs`, 4p Bidding→DeclareTrump→Discard→Play→HandComplete flow with full scoring.
- [x] **C3** — 30-for-60, sets rule, game-over, 6-player Chartrand variant behind a config flag. Note: PHP has a latent bug in 30-for-60 (treats bid value as 60 but max hand pts is 30, so bid can never be "made"); Rust implements the correct ±60 sweep semantics instead and flags the divergence in `score_hand` comments.
- [x] **C4** — PyO3 bindings (`src/bindings.rs`: `GameState`, `bid`, `declare_trump`, `discard`, `play`, `next_hand`, `legal_bids`, `legal_plays`, all accessors), `scripts/smoke.py` Python smoke test, `tests/proptest_invariants.rs` with 3 properties (conservation, determinism, multi-hand). Also added `GameState::discarded()` so conservation counts are exact.
- [x] **C5** — Golden corpus (`tests/golden_corpus.rs`, 5 scenarios via `GameState::from_state`), benchmark run recorded, `docs/stage-0.md` written, retrospective appended below. Round-robin deal order adopted (matches PHP) for forward compatibility with a future deck-replay harness. PHP cross-engine corpus deferred with rationale — see `docs/stage-0.md → Deferred`.

### Current state of the repo (post-C5)

- `Cargo.toml`: pyo3 0.28, feature-gated `extension-module` so `cargo test` builds (dependencies section) — required because PyO3's `extension-module` defers Python symbols to runtime, which breaks Rust-only test binaries.
- `pyproject.toml`: maturin points at `python-source = "python"`, `features = ["extension-module"]`.
- `src/state.rs`: GameState with `discarded: Vec<Card>` field + `from_state` constructor for corpus/authored scenarios. Round-robin deal order matches PHP. `num_players()` exposed publicly.
- `src/bindings.rs`: stringly-typed Python API; `sets()` returns `(u8, u8)` tuple not `[u8; 2]` (PyO3 would encode the array as `bytes`).
- Tests: 49 unit + 3 proptest + 5 golden corpus = **57 total**, all green.
- `.venv/bin/python` — 3.14.4 — is the interpreter. `maturin develop --release` rebuilds.

## Benchmarks

Numbers from `cargo run --release --example bench_hands_per_sec` on M5 Pro.

- **2026-04-24** — Single-thread 4p random-legal driver: **~225k hands/sec** (stable range 212–234k across 4 runs), ~6.5M actions/sec. 29 actions/hand avg (4 bids + 1 declare + 4 discards + 20 plays = 29). Release mode, no neural-net inference.
- Implication for Stage 4: on all 12 P-cores with naive parallelism, ceiling is ~2.5M hands/sec pure-rules — comfortably >> the ≥100k all-core goal. Neural-net evaluation will dominate once wired in.

## Status legend

`[ ]` pending · `[~]` in progress · `[x]` done · `[!]` blocked

## Stage 0 — Headless game engine (Rust + PyO3)

Goal: deterministic, fast, rules-accurate engine callable from Python.

- [x] Scaffold Rust crate + Python package layout (`Cargo.toml`, `pyproject.toml`, `src/`, `python/f45/`)
- [x] Define core types: `Card`, `Suit`, `Rank` (C1)
- [x] Define core types: `Seat`, `Trick`, `GameState`, `Move` (C2)
- [x] Implement deal, bid, trump declare, discard, play phases (C2–C3)
- [x] Implement trick resolution with bowers + reneging rules (C1)
- [x] Implement Chartrand 6-player variant as a config flag (C3)
- [~] Define `GameView` (per-seat visible state including event log) — partial: per-seat `hand()` + full-table accessors exist; no dedicated typed view yet. Revisit when Stage 1 needs it.
- [x] PyO3 bindings: `GameState`, `bid`/`declare_trump`/`discard`/`play`, `legal_bids()`/`legal_plays()`, plus accessors (C4)
- [x] Rust unit tests for each phase + trick resolution edge cases (49 passing in `cargo test --lib`)
- [x] Property tests (`tests/proptest_invariants.rs`): card conservation, determinism, multi-hand (C4)
- [x] Golden-corpus test harness: 5 hand-authored 4p scenarios via `from_state` (C5). PHP cross-engine replay deferred — see `docs/stage-0.md → Deferred` for rationale.
- [x] Benchmark: single-threaded ~225k hands/sec (vs target ≥10k), multi-core naive ceiling ~2.5M hands/sec (vs target ≥100k) — both comfortably exceeded.

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

### Stage 0 — 2026-04-24

**What worked**

- **Checkpoint sequencing (C1→C5).** Each checkpoint's output was legitimately the next one's baseline. C2 was meaningless without C1's ranker; C4's proptest invariants would've been untestable without C3's multi-hand flow. Resist the urge to re-cut stages mid-work — the ordering held up.
- **Feature-gating `extension-module`.** Compiling the same crate both as a Python extension (with `maturin develop`) and as a regular Rust binary (with `cargo test`) requires that `pyo3/extension-module` be opt-in. The gate pattern (Cargo `[features]` + `pyproject.toml` enabling it) is reusable for any PyO3 + Rust-tested project; recommend lifting it into a personal template.
- **Tracking `discarded` in-state.** Initially felt like over-instrumentation, but it made the proptest conservation check fall out almost for free, and it's a head start for Stage-4 AI observability. Small cost (a Vec per hand, cleared on deal), outsized payoff.
- **`GameState::from_state(...)` for authored scenarios.** Writing the hand directly beats reverse-engineering a deck order. Advisor caught my initial `new_hand_from_deck` design as mis-targeted — hand-state is the natural shape for corpus tests, and the `from_state` constructor is ~60 lines including thorough validation.

**What surprised**

- **PHP's 6-player variant is broken in the rule engine, not just the bot.** Hardcoded `% 4` at `GameRuntimeService.php:524,528,747,748` means PHP's 6p Chartrand mode was never properly validated. This invalidated the original plan to cross-engine property-test both variants; Rust is authoritative for 6p going forward.
- **PHP's rule logic is entangled with the database layer.** Extracting standalone traces from PHP is non-trivial work — `GameRuntimeService` threads a `GameRepository` transaction through every rule evaluation. The Stage-0 plan assumed trace emission would be straightforward; reading the actual code showed otherwise. **Scope reduction:** 5 hand-authored Rust-only scenarios in lieu of 10k+ PHP-replay games. Rationale captured in `docs/stage-0.md → Deferred`.
- **Round-robin vs contiguous deal.** Hadn't realized PHP dealt round-robin (5 rounds × n seats via `array_shift($deck)`). Originally Rust was dealing contiguous blocks. Fixed mid-C5 — no existing test failed because none asserted on specific dealt cards. Forward-compatible with a future PHP deck-replay harness that doesn't compare shuffle outputs.
- **Throughput headroom.** Expected 10k hands/sec single-thread; got ~225k. The engine has plenty of runway — Stage-4 bottleneck will be neural-net inference, not rules. Won't feel the need to Rust-port the MCTS inner loop early.

**What to carry forward**

- **Advisor is highest-value before writing, not after.** The three directives that reshaped C5 (round-robin everywhere, `from_state` over `new_hand_from_deck`, document scope reduction explicitly) all landed before I wrote a line of corpus code. Post-hoc advisor calls are a much weaker gradient.
- **Document divergences at file:line granularity, not vaguely.** "PHP's 6p is buggy" evaporates quickly as memory. `GameRuntimeService.php:524,528,747,748` survives to be actionable later.
- **Name scope reductions explicitly.** Drifting from the original plan without flagging it is how Stage-3 ends up depending on an untested Stage-0 invariant. The "Deferred" section in `docs/stage-0.md` is load-bearing.
- **Benchmark early, even with dumb drivers.** The 225k number took 10 minutes to produce and changes how I'll reason about Stage-2 MCTS budget. Don't wait for a "representative" driver — any consistent baseline is better than none.

**Open threads for later stages**

- Event log enum (deferred to Stage 4 if feature engineering needs it).
- Typed `GameView` per seat (deferred to Stage 1 where the strategy trait will want it).
- PHP cross-engine golden corpus (deferred; pick up concurrently with Stage 1 if we're already modifying the PHP bot).
- Int-typed Python API alongside the stringly-typed one (add only if Stage-4 FFI profiling demands).
