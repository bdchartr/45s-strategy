# Learning This Stuff

A companion doc to the project. Each project stage uses several technologies
you might not have deep experience with — this doc walks you through *what
they are*, *why we picked them here*, and *how to learn more*. It grows
stage by stage; old sections stay so you can flip back.

## How to read this doc

Each technology section is structured the same way:

1. **What it is** — one-paragraph plain-language definition.
2. **The concept that matters here** — the *one or two ideas* that make it
   click for this project. Not a tutorial, not a reference. Just enough to
   read the code without confusion.
3. **How to learn more** — a suggested order if you want to go deeper.
   Curated, not exhaustive: I'd rather point you at one good thing than
   five mediocre ones.

If something in the codebase confuses you, search this doc for the
filename or concept first. If it's not here, it's worth adding — flag it.

---

# Stage 0 — Rust + PyO3 game engine

## Rust

### What it is

A systems language with C-level performance and a strict compile-time
type system that catches whole classes of bugs (memory errors, data
races, null derefs) before the program runs. Think "C++ designed in
2010 with the benefit of hindsight."

### The concept that matters here

**Ownership.** Every value has exactly one owner; passing it elsewhere
either *moves* ownership or *borrows* a reference. The compiler tracks
who owns what across every line of code; if your code can't be analyzed
into a sound ownership graph, it doesn't compile. Once you internalize
ownership, most of Rust's other complexity falls out of it.

For this project specifically, ownership is *why* the engine is fast and
deterministic: we never accidentally share mutable state, and copying a
`Card` (2 bytes, `Copy`) is cheaper than reference-counting it.

You'll see `&self` (immutable borrow), `&mut self` (mutable borrow), and
`self` (move) on most methods in `src/state.rs`. Read those as "this
method needs to look at me / mutate me / consume me."

### How to learn more

1. **The Rust Book** (free, official). Skim chapters 1–4, read 4 carefully
   (ownership), skim the rest. This is enough to read everything in
   `src/`. https://doc.rust-lang.org/book/
2. **Rust by Example** for hands-on practice with the syntax.
   https://doc.rust-lang.org/rust-by-example/
3. **Programming Rust, 2nd ed.** (Blandy/Orendorff/Tindall, O'Reilly) is
   the best reference book if you want depth. Lifetime chapter alone is
   worth the price.

Skip "advanced" Rust topics until they bite you: macros, async, unsafe,
lifetime variance. None of them appear in our Stage 0 code.

## PyO3 — calling Rust from Python

### What it is

A Rust crate that exposes Rust code as a Python extension module. You
write Rust functions decorated with `#[pyfunction]`, Rust structs
decorated with `#[pyclass]`, and PyO3 generates the C ABI glue that
Python's import system speaks.

### The concept that matters here

**There are two builds of the same crate.** When you run `cargo test`,
the crate compiles as a normal Rust binary that links against
`libpython` and uses real Python symbols. When you run `maturin develop`,
the crate compiles with `pyo3/extension-module` enabled, which *defers*
all Python symbol resolution to runtime — needed for the produced `.so`
to load into Python.

This matters because if you enable `extension-module` unconditionally,
`cargo test`'s integration-test binaries fail to link. The fix is to
make `extension-module` opt-in via a Cargo feature, enabled in
`pyproject.toml` for the wheel build but not for `cargo test`. See
`Cargo.toml`'s `[features]` section and `pyproject.toml`'s
`features = ["extension-module"]` line.

The other PyO3 wart you'll meet is **type encoding**. Some Rust types
encode "obviously" in Python (e.g. `Vec<String>` → `list[str]`), some
don't (`[u8; 2]` becomes `bytes`, not a 2-element list — that's why
`sets()` returns a tuple in `bindings.rs`).

### How to learn more

1. **PyO3 user guide.** Read the "Class" and "Function" pages first —
   that's 90% of what we use. https://pyo3.rs/
2. **The PyO3 0.20+ migration notes** if a future version is needed
   (the API changed shape around 0.21). We pin 0.28 in `Cargo.toml`.
3. Skim `bindings.rs` next to the PyO3 docs — it's a complete worked
   example of a stringly-typed wrapper around a stateful Rust object.

## maturin — building the Python wheel

### What it is

A build tool that runs `cargo build` and packages the resulting `.so`
into a Python wheel installable by pip. `maturin develop` installs the
package in your active virtualenv in editable mode.

### The concept that matters here

`maturin develop --release` is a *rebuild* command. After any Rust
change, run it before re-running Python — otherwise Python keeps using
the old `.so`. Symptoms of forgetting: a function exists in
`bindings.rs` but `ImportError` says it's not in `f45._engine`.

### How to learn more

The maturin user guide is short and complete. Read the "Project layout"
and "Build options" pages and you've got it. https://www.maturin.rs/

## ChaCha8 — the deterministic shuffle

### What it is

A cryptographically-secure pseudorandom number generator from the
ChaCha family (Daniel Bernstein, 2008). The "8" is the round count —
ChaCha8 is faster than ChaCha20 with cryptographic strength sufficient
for non-adversarial use (which is us — we're not protecting secrets,
we're seeding deterministic shuffles).

### The concept that matters here

**Deterministic from a seed, portable across machines.** Given the same
`u64` seed, ChaCha8 produces the same byte stream on any platform with
any compiler. That's the property we need: every game is reproducible
from `(seed, dealer, num_players)`.

We picked it over Rust's default `SmallRng` (which is non-portable) and
PHP's Mersenne Twister (non-portable in a different way). Tradeoff:
ChaCha8's stream is *not* the same as PHP's, so we cannot replay PHP
games bit-for-bit — see `docs/stage-0.md → Known divergences`.

### How to learn more

You probably don't need to. PRNG choice is a one-line decision in
`Cargo.toml`. If you're curious: Bernstein's "ChaCha, a variant of
Salsa20" is a 6-page paper; the rand-chacha crate docs are also fine.

## Property-based testing (proptest)

### What it is

A testing framework that generates *random* inputs to your function and
checks that some invariant holds for all of them. If proptest finds a
failing input, it *shrinks* it to the minimal counter-example.

You define a "strategy" (proptest's word — confusingly overloaded with
ours) that produces inputs, plus a property that should always hold.
Proptest runs the property against thousands of generated inputs.

### The concept that matters here

Property tests are good at catching the kind of bug you didn't think to
test for. Our `tests/proptest_invariants.rs` checks **card conservation**
— that across any random sequence of legal moves on any seed, the engine
always tracks exactly 52 distinct cards. If the engine ever loses or
duplicates a card, proptest will find a seed that exhibits it and shrink
to the minimum reproducer.

The shrinking is the magic. A 200-step bug becomes a 4-step bug.

### How to learn more

1. **proptest book** — short, with worked examples.
   https://proptest-rs.github.io/proptest/
2. **John Hughes — "Testing the Hard Stuff and Staying Sane"** is the
   classic talk on property testing in industry. 30 minutes; worth it.
3. The Hypothesis project (Python) is the same idea; its docs are
   excellent reading even when you're working in Rust.

## Cargo features for conditional compilation

### What it is

Named flags in `Cargo.toml` that enable optional dependencies or code
paths. `cargo build` compiles a default set; `cargo build --features X`
adds X.

### The concept that matters here

**A feature is just a string with `cfg!`-gated code behind it.** Our
`extension-module` feature gates the `pyo3/extension-module` flag. When
maturin builds the wheel, it passes the feature; when `cargo test` runs,
it doesn't. The same crate compiles to two different binaries depending
on context.

This pattern (`extension-module = []` in `[features]`, then
`pyo3 = { version = "0.28", features = [], optional = false }`, then
maturin enabling it via `pyproject.toml`) is reusable for any
PyO3+Rust-tested project.

### How to learn more

The Cargo book's "Features" page is short and complete.
https://doc.rust-lang.org/cargo/reference/features.html

## Where to go next

By the end of Stage 0 you can:

- Read and modify `src/state.rs` (the state machine — the biggest file,
  the densest Rust).
- Add a new method to the PyO3 surface and rebuild via `maturin develop`.
- Read a proptest failure report and turn it into a unit test.
- Explain why the Rust↔PHP shuffles diverge.

If any of those still feel shaky, that's the right thing to fix before
Stage 1.

---

# Stage 1 — Python strategies + tournament harness

The Stage-0 code stays as the engine; everything new lives in
`python/f45/`. The big shift: we're now *consuming* the Rust API rather
than building it. Most of the learning here is about Python idioms and
reproducibility patterns.

## typing.Protocol — structural interfaces

### What it is

A way to define an interface based on *what methods/attributes a type
has*, not on *what it inherits from*. Any class that has a `name`
attribute and an `act` method (matching the right signature) IS a
`Strategy` — no `class L1Novice(Strategy)` declaration needed.

This is duck typing made explicit. Static type checkers (mypy, pyright)
verify the conformance; at runtime, `isinstance(x, MyProtocol)` works
if you decorate it with `@runtime_checkable`.

### The concept that matters here

For a research codebase with many strategies of wildly different shapes
(hand-written rules now, MCTS later, neural nets later), structural
typing means you don't have to inherit from a common base or import a
shared module just to declare conformance. Strategies can be developed
in total isolation and slot in via the `Strategy` protocol.

The contrast with `abc.ABC` (nominal typing): an ABC requires
`class L1Novice(Strategy):` and forces an import dependency. Protocols
don't.

### How to learn more

- **PEP 544** (Protocols) is the spec. Skim the "Use cases" section —
  the rest is for type-checker authors.
- **Real Python — Python Protocols** is a friendly intro.
  https://realpython.com/python-protocol/

## random.Random — per-instance reproducible RNG

### What it is

Python's `random` module has a global state used by `random.choice()`,
`random.shuffle()`, etc. `random.Random()` creates an *independent*
RNG instance with its own state, seedable independently of the global
one and other instances.

### The concept that matters here

**Don't use the module-level `random.choice` in code that needs
reproducibility.** It shares state with every other call in the
process; one untracked import can change your "deterministic" seed
behavior.

In `l1_novice.py`, each `L1Novice(seed=N)` instance creates its own
`random.Random(N)`. Two L1 bots with the same seed make identical
tiebreak choices; bots with different seeds don't. The tournament
harness exploits this by giving each seat a different seed, breaking
correlation that would otherwise make the four bots play in lockstep.

### How to learn more

The stdlib docs page for `random` is sufficient — you just need to know
that the class exists and that you should prefer it over module-level
functions for any non-trivial code.
https://docs.python.org/3/library/random.html

## Seed derivation — making a tournament reproducible

### The concept

A "tournament of 1000 games" needs 1000 different shuffle seeds. The
naive approach is `seed = base_seed + game_index`, which works but
correlates adjacent games (seeds 0 and 1 produce hands with similar
RNG bytes for a few rounds — usually fine, but feels gross).

The cleaner approach is **mixing**: derive each game's seed via a
high-quality hash of the base seed and the game index. We use
splitmix64's golden-ratio constant (`0x9E3779B97F4A7C15`) for cheap
mixing — multiply, then xor:

```python
def hand_seed(game_seed: int, hand_index: int) -> int:
    return (game_seed * 0x9E3779B97F4A7C15 ^ hand_index) & 0xFFFF_FFFF_FFFF_FFFF
```

Two adjacent game seeds now produce uncorrelated hand seeds. See
`python/f45/tournament.py`.

### How to learn more

If you ever care: read the splitmix64 paper (Steele/Lea/Flood, 2014)
and the ChaCha papers above. For day-to-day work this is a "pick one
constant, never think about it again" decision.

## Where to go next

By the end of Stage 1 you can:

- Write a new `Strategy` that conforms to the protocol (no inheritance
  needed; just match the shape).
- Run a tournament between any two strategies via `run_tournament`.
- Reason about why a self-play tournament should converge to ~50/50
  and what asymmetries in the harness would push it elsewhere.

The next stage (MCTS) introduces the algorithmic content that none of
this prepares you for. We'll add:

- Monte Carlo Tree Search and its Information-Set variant for hidden
  hands.
- Multiprocessing patterns for embarrassingly-parallel game simulation.
- Profiling tools for finding the actual bottleneck (it's almost never
  where you think).

This file will grow.
