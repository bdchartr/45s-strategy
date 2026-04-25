# Stage 0 — Headless 45s engine

A Rust game engine for 45s callable from Python via PyO3. This is the
foundation for everything downstream: rule-based baselines (Stage 1), MCTS
(Stage 2), and self-play RL (Stage 4) all consume positions and moves from
this engine.

The goal of Stage 0 was to produce a **deterministic, fast, rules-accurate
engine** that could drive millions of games per core per second — enough
headroom that neural-net inference (not the rules engine) becomes the Stage-4
bottleneck.

## Quickstart

```bash
# Rust-only: unit tests, property tests, golden corpus.
cargo test

# One-off benchmark (release mode required; debug is ~15× slower).
cargo run --release --example bench_hands_per_sec

# Python smoke test. First rebuild the extension:
source .venv/bin/activate
maturin develop --release
python scripts/smoke.py
```

## What the engine does

A single `GameState` value holds everything about a hand: the dealer, all
four hands, the kitty, the deck remainder, the phase (Bidding, DeclareTrump,
Discard, Play, HandComplete, GameOver), the current bid, and running scores
/sets. `GameState::apply(Move)` is the only mutation entry point. `Move` is
a seat-attributed `Action` (`Bid`, `DeclareTrump`, `Discard`, `Play`). The
engine rejects mis-routed moves at the type boundary (e.g. `Play` during
bidding), which means strategies above it never have to re-check phase.

Full game loop: `new_hand` → play to `HandComplete` → `next_hand(seed)` →
repeat until `GameOver`. Scores and sets persist across hands; per-hand
state is reset by `deal_hand`.

## Layout

```
src/
  cards.rs          # Card, Suit, Rank primitives; standard_deck(); parsing
  ranker.rs         # is_trump, is_top_trump, strength — the 45s quirks live here
  rules.rs          # legal-move validation, trick resolution
  state.rs          # GameState machine, bid/discard/play/score, GameConfig
  bindings.rs       # PyO3 stringly-typed Python API
  error.rs          # EngineError enum
  lib.rs            # crate root + PyO3 module registration

tests/
  proptest_invariants.rs   # randomized drivers, conservation/determinism
  golden_corpus.rs         # hand-authored scenarios pinning specific rules

examples/
  bench_hands_per_sec.rs   # single-thread throughput baseline

scripts/
  smoke.py                 # end-to-end Python driver across the PyO3 API

python/f45/                # thin Python wrapper + pyi stubs
```

## C1–C5 learning path

Stage 0 was split into five checkpoints. Each stage's output is the next
stage's baseline — skipping any one would leave downstream stages unable to
measure what they just built.

### C1 — Primitives + ranker + rules

- `Card` is `#[repr(u8)]` in both fields → 2 bytes total, `Copy`. Self-play
  deals 52 cards thousands of times per second; `Copy` semantics are much
  cheaper than `Arc`/`Rc`.
- `Suit` and `Rank` are enums, not ints — the type checker forbids
  nonsensical expressions like `Rank::Five * 2`.
- The 45s **bower hierarchy** (5-of-trump > J-of-trump > A-of-hearts) is
  implemented in `ranker::strength`. AH is always trump, regardless of
  declared suit — the single subtlety that trips every implementation. The
  `rules` module composes `ranker` with lead-suit logic to produce
  `can_play_card` and `winning_index`.
- Red/black number-card asymmetry (red high-to-low, black low-to-high) is
  a quirk inherited from the 25/45/110 Irish family.

### C2 — State machine + deal + score + full 4p play-through

- Phases as an enum; moves as an enum; `apply` is a phase×action dispatch.
  This lets us write `(Phase::Bidding, Action::Bid { .. }) => …` and let
  the compiler enforce exhaustiveness.
- Scoring model: 5 points per trick won + 5 high-trump bonus, with the
  bidder team's adjustment applied at hand-end. Either they made their bid
  (scores += hand_points) or they were set (scores -= bid_amount; sets += 1).
- Unit test: hand-authored trump + discard + 5-trick play-through with
  hand_points summing to 30 and bid_made flag set appropriately.

### C3 — 30-for-60, sets rule, game-over, 6-player Chartrand variant

- 30-for-60: a bid of 60 that requires sweeping all 30 hand points.
  Implementation notes: a ±60 delta either way and a dedicated
  `is_30_for_60` flag to keep scoring branches readable.
- Sets rule: any team with 3 sets loses immediately.
- Game-over rule: bidder team hits target score via a *made* bid.
  Non-bidder reaching target does *not* end the game ("bid out" rule).
- 6-player Chartrand variant: `GameConfig { num_players: 6, .. }` switches
  in a `DealerExtraDraw` sub-phase where the dealer gets the entire deck
  remainder after normal discards and must discard back to 5.

### C4 — PyO3 bindings + Python smoke test + property tests

- Python API is **stringly-typed**: card codes are strings (`"AH"`, `"10D"`),
  suits are single chars (`"S"`). Rationale: while the engine shape is
  still iterating, clarity at the boundary matters more than the ~30% FFI
  overhead. If Stage-4 profiling shows the boundary is the bottleneck, add
  an integer-typed API *alongside* — don't replace.
- `sets()` returns a `(u8, u8)` tuple instead of a `[u8; 2]` array because
  PyO3 encodes `[u8; 2]` as Python `bytes`, which is unintuitive.
- Feature gate: `extension-module` is only enabled for wheel builds (via
  `pyproject.toml`). Unconditional enablement defers Python symbols to
  runtime, which breaks Rust integration-test binaries (they compile as
  standalone executables and fail to link). `cargo test` builds without
  the feature; `maturin develop` builds with it.
- Property tests: card conservation (52 cards tracked across hand/kitty/
  deck/discard/tricks), determinism (same seed ⇒ same outcome), multi-hand
  conservation across `next_hand()`. Discard phases use a deterministic
  heuristic rather than random subsets — the subset space is exponential
  and already covered by targeted unit tests.

### C5 — Golden corpus + benchmarks + this doc

- Golden corpus (`tests/golden_corpus.rs`): 5 hand-authored 4-player
  scenarios that pin specific rule interactions (top-trump exemption,
  required trump follow, bid made, bid set, full play-through sanity).
  Uses `GameState::from_state(...)` to bypass the deck shuffle and pin
  exact hands.
- Benchmark (`examples/bench_hands_per_sec.rs`): ~225k hands/sec
  single-threaded on an M5 Pro, ~6.5M actions/sec. Implication for
  Stage 4: on all 12 P-cores with naive parallelism the ceiling is ~2.5M
  hands/sec of pure rules evaluation — >> the ≥100k all-core goal. Neural
  inference will dominate once wired in.
- Decision: track `discarded: Vec<Card>` in-state. Engine does not read it
  back, but the proptest conservation check needs it, and future AI
  feature extraction will want it for free.

## Key design decisions

These are recorded in full with rationale in `GOALS.md → Decision log`;
summaries here.

| Decision | Rationale |
|---|---|
| ChaCha8 shuffle (not PHP's Mersenne Twister) | Portable, deterministic from a u64 seed, fast. Consequence: Rust↔PHP cannot be compared bit-for-bit on shuffle outputs. |
| Round-robin deal (matches PHP order) | `deal_hand` deals 5 rounds × n seats, then 3 kitty, then remainder — mirroring `GameRuntimeService::dealNewHand()`. Forward-compatible with a future deck-replay harness. |
| `from_state` constructor (not `new_hand_from_deck`) | Authored scenarios want to write the *hand state* directly, not reverse-engineer a deck order. `from_state(hands, kitty, deck_remainder, config, dealer)` is the corpus ergonomic. |
| Stringly-typed Python API | Clarity > 30% FFI overhead at Stage 0. Add int-typed API alongside if profiling demands. |
| 30-for-60 implements the documented rule, not the PHP bug | PHP checks `bidder_pts >= 60` (unmakeable — max hand is 30). Rust checks `bidder_pts >= 30` and applies a ±60 delta. Flag divergence in scoring comments. |
| `extension-module` feature opt-in | Required so `cargo test` can link. Maturin enables it for wheel builds; `cargo test` compiles without it. |

## Known divergences from the PHP engine

The PHP engine at `wkapp-45/app/Application/Services/GameRuntimeService.php`
is the source of truth for game rules, *except* for the following bugs or
design differences identified during Stage-0 work:

1. **30-for-60 unmakeable in PHP.** PHP's `score_hand` checks
   `bidder_pts >= bid.amount` with `bid.amount = 60`, but max hand points
   is 30 — the bid cannot structurally be made. Rust implements the
   documented rule. If a golden-corpus comparison is added later and PHP's
   behavior is genuinely what the user expects, revisit.
2. **6-player variant has hardcoded `% 4` / `=== 4` in the rule engine.**
   Locations in `GameRuntimeService.php`:
   - line 524: `if (count($newPlays) === 4)` — trick-completion check
     hardcoded to 4 plays regardless of variant
   - line 528: `$nextSeat = ($command->seat + 1) % 4;` — next-seat wraps
     at 4 even in 6-player games
   - lines 747–748: `$newDealer = ((int) $prevHand['dealer_seat'] + 1) % 4;`
     and `$firstBidder = ($newDealer + 1) % 4;` — dealer rotation wraps at 4
   These aren't isolated bot-side bugs; they live in the core rule engine.
   Rust's 6-player variant correctly wraps at `config.num_players`.
3. **Shuffle RNG.** PHP uses Mersenne Twister via `shuffle()`; Rust uses
   `ChaCha8Rng`. Given the same seed, the dealt hands differ. Cross-engine
   comparison must start from a pre-specified deck.

## Stage-0 scope: what's in, what's out, what's deferred

### In scope (delivered)

- Full 4-player game loop (bid → declare → discard → play → score → next hand → game over).
- 6-player Chartrand variant behind `GameConfig { num_players: 6 }`.
- PyO3 bindings for the full public API (`GameState`, all phase transitions,
  legal-move enumeration, accessors).
- 49 Rust unit tests covering ranker, rules, state machine, scoring edge cases.
- 3 proptest invariants (conservation, determinism, multi-hand conservation).
- 5 golden-corpus scenarios pinning specific rule interactions.
- Python smoke test (`scripts/smoke.py`).
- Benchmark harness with recorded baseline numbers.

### Deferred — scope reduction from the original plan

- **PHP-corpus cross-engine property tests.** The original Stage-0 success
  criterion was "Rust engine matches PHP behavior on 10k+ property-test
  games." Delivered instead: 5 hand-authored Rust-only scenarios.

  *Rationale:* PHP's rule logic is entangled with its database layer in
  `GameRuntimeService.php` — standalone trace emission from PHP would
  require non-trivial extraction work (stubbing `GameRepository`, pulling
  the rule classes out of the transaction boundary). The pragmatic call
  for Stage 0 was to defer this and flag it here explicitly. A later
  engineering task (probably concurrent with Stage 1 when we're already
  modifying the PHP bot) can either:

    - extract PHP's rule functions into a pure-function module and wire
      a small trace-emission CLI, then run the cross-engine replay, or
    - accept that Rust is authoritative going forward and drop the PHP
      comparison entirely.

- **Typed `GameView` per seat.** The current API exposes per-seat `hand()`
  plus full-table accessors (`completed_tricks()`, `current_bid()`, etc.).
  A dedicated typed view that hides hidden state is deferred to Stage 1
  where it'll be needed by the strategy trait.

- **Event log enum.** PHP emits a detailed `event_type`/`payload` log; Rust
  does not. Self-play RL doesn't currently need one. If Stage 4 feature
  engineering needs it, revisit.

## Benchmark numbers

Recorded by `cargo run --release --example bench_hands_per_sec` on an M5 Pro.

- **Single-thread 4p random-legal driver:** ~212–234k hands/sec, ~6.5M
  actions/sec. 29 actions/hand average (4 bids + 1 declare + 4 discards
  + 20 plays).
- **All-P-cores ceiling (12 cores, naive parallelism):** ~2.5M hands/sec of
  pure rules. Well above the Stage-0 success criterion of ≥100k all-core.

Neural-net inference will dominate these numbers once wired in. Treat this
baseline as "rules-engine headroom," not a Stage-4 throughput prediction.
