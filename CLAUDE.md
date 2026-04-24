# Claude Orientation

Read this first on any new session in this repo.

## What this repo is

Research project to develop strong AI strategies for the 45s card game via self-play reinforcement learning on Apple Silicon. Target architecture is AlphaZero-lite: Rust game engine, Python orchestration, MLX or PyTorch MPS for neural nets, actor-learner parallelism across M5 Pro cores + GPU.

This is a **research/learning project**, not production code. Prioritize experimentation, measurement, and understanding parallel compute patterns over polish.

## Relationship to sibling repo

The production 45s game lives at `/Users/chartb/excl/wkapp-45` (repo: `bdchartr/45s-2026`). That codebase is PHP + vanilla JS and is the **source of truth for game rules**. When in doubt about rule edge cases (bowers, reneging, Chartrand 6-player variant, discard mechanics), consult:

- `/Users/chartb/excl/wkapp-45/game_rules.md`
- `/Users/chartb/excl/wkapp-45/app/Application/Services/GameRuntimeService.php`
- `/Users/chartb/excl/wkapp-45/app/Infrastructure/AI/AlgorithmicMoveProvider.php` (current naive bot)

Any game engine we build here must property-test against PHP behavior to catch rule drift.

## Where to look

- `GOALS.md` — mission, non-goals, success criteria, stack rationale. Stable doc.
- `TODO.md` — staged work plan, current status, pending decisions. Living doc — update it.
- `README.md` — brief project description for humans.

## Current phase

Check `TODO.md` → "Current focus" at the top. Do not start work without reading it.

## Conventions

- Update `TODO.md` status markers as you work: `[ ]` pending, `[~]` in progress, `[x]` done, `[!]` blocked.
- When you finish a stage, write a short retrospective at the bottom of `TODO.md` under "Stage retrospectives" — what worked, what surprised, what to carry forward. These accumulate; don't delete old ones.
- Capture decisions with rationale in `GOALS.md` under "Decision log" when you make non-obvious choices. Future sessions need the *why*, not just the *what*.
- Benchmarks matter. Record numbers (games/sec, training throughput, win rates) in `TODO.md` as you hit them.

## Don'ts

- Don't port strategies to the PHP production repo until explicitly asked. That's a later, well-scoped engineering task — not part of the research.
- Don't add dependencies casually. This is a learning project; each tool choice should be a deliberate decision with rationale.
- Don't skip stages. Each stage's output is the next stage's baseline. Stage 4 self-play RL is meaningless without Stage 1 rule-based opponents to measure against.
