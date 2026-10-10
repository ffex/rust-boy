# CLAUDE.md

rust-boy is a Rust DSL that generates Game Boy assembly in RGBDS syntax (`cargo run` → `.asm` →
`rgbasm`/`rgblink`/`rgbfix` → `.gb`). It is being refactored:

- [`Task.md`](Task.md) — what to do next (phased checklist).
- [`CONTEXT.md`](CONTEXT.md) — why: architecture, levels assessment, bug catalogue B1–B30, branch analysis.

Read both before starting a task, and keep them up to date in the same branch as the change.

## Working style

When a step doesn't need my input, keep going. Put status notes in the
same message as your next action.
Stop and ask only when you can't continue without me, or before anything
destructive: deleting data, force-pushing, or changing anything outside
this repository.

Once you have answered something, treat that answer as done. Focus on
what I'm asking now, and don't go back over an earlier answer unless I
ask about it or point out a problem with it.

## Commands

```bash
cargo build                               # library + all bins
cargo test                                # unit tests
cargo clippy --all-targets                # lint
cargo fmt                                 # format
cargo run --bin <name> > main.asm         # bins: basic_usage, unbricked, unbricked_std,
                                          #       unbricked_rustboy, fosdem, coin-anim
scripts/assemble-examples.sh              # every example -> target/examples/<bin>/main.gb (needs RGBDS;
                                          # under $CARGO_TARGET_DIR when it is set)
```

Assembling generated output (RGBDS ≥ 0.9). `include/hardware.inc` (v4.x) and the example's `.2bpp` assets
go on the include path with `-I`:

```bash
rgbasm -I include -I examples/fosdem -o main.o main.asm   # no `-L`: it was removed in RGBDS 0.8
rgblink -o main.gb main.o
rgbfix -v -p 0xFF main.gb
```

No RGBDS installed (e.g. in a cloud session)? Build it from the official source:
`git clone --depth 1 --branch v1.0.4 https://github.com/gbdev/rgbds` and run
`make rgbasm rgblink rgbfix` in that directory (needs a C++ compiler, bison and libpng).

## Architecture rules

- Layers: `asm` (instructions, `src/asm/`) → `stdlib` (stateless routines, `If`/flow control, `src/stdlib/`) →
  `engine` (`RustBoy` and its managers, `src/engine/`), with `hw` (pure data, `src/hw.rs`) beside them and
  `prelude` (re-exports). A layer depends only on the layers below it; `asm` does not use `hw` (see `CONTEXT.md`
  §3). Until Phase 2 they were `gb_asm`, `gb_std` and `rust_boy` (`rust_boy::rust_boy`).
- Every routine (Memcopy, WaitVBlank, UpdateKeys, …) exists **once**. Do not copy a routine into another layer.
  The exceptions, decided by the maintainer: the two examples written by hand with raw `asm` never link with the
  library's routines and keep their own: `src/bin/unbricked.rs`, the tutorial (WaitVBlank, Memcopy, UpdateKeys,
  GetTileByPixel), and `src/bin/basic_usage.rs`, the minimal example (WaitVBlank, which waits for `rLY` = 144).
- Generated output must be **deterministic**: never let `HashMap`/`HashSet` iteration order reach the
  output; use `BTreeMap` or an ordered `Vec`. Sprites, tiles, variables and functions are emitted in the
  order they were created.
- Generated labels must be unique and must not break RGBDS local-label scope (a global label inside an
  `If` body breaks `.end_if_N`). Prefer local labels or the label allocator.
- Do not hardcode hardware addresses/flags as strings in new code; use (or add to) the `hw` constants.
- Every bug fix gets a test; when it changes generated asm, the asm must still assemble.
- Never commit build artifacts (`*.gb`, `*.o`, `target/`).

## Git workflow

- `refactor` is the integration branch. Never commit directly to `main` or `refactor`.
- For each phase, or each major edit, create a branch from an up-to-date `refactor`:
  `refactor-<phase>-<topic>`, e.g. `refactor-p1-if-semantics`, `refactor-p2-typed-operands`.
  (Use a dash, not a slash: git cannot have both a `refactor` branch and `refactor/...` branches.)
- If the work needs a PR that is not merged yet, branch from that PR's branch instead (stacked PR),
  still target `refactor`, and start the PR description with "Depends on #N".
- Small, focused commits with clear messages. Tick the matching boxes in `Task.md` in the same branch.
- Before pushing, run what CI runs (`.github/workflows/ci.yml`): `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `scripts/assemble-examples.sh`.
- Push the branch and open a **PR into `refactor`** (never straight into `main`). When everything is done,
  `refactor` → `main` is a final PR.
- Never force-push, rewrite published history or delete branches without asking first.
- Do not mention or link the Claude session in commit messages or PR descriptions.

### Independent review before asking for a merge

Before telling the maintainer that a PR (or a chain of stacked PRs) is ready, have an agent with **no
context** from the work review it. Give it only the PR numbers, their branches and the merge order, not
your own conclusions. It must:

1. Simulate the merge into `refactor` locally (in order, never push) and report conflicts.
2. Run the same checks as CI on the merged result (fmt, clippy `-D warnings`, tests,
   `scripts/assemble-examples.sh`).
3. Compare each PR description with the code it really changes, and flag false or unverified claims.
4. Review the changes for bugs, regressions and violations of the rules in this file.
5. Check that `Task.md` and `CONTEXT.md` agree with the code.

The reviewer is read-only towards GitHub (no pushes, comments or reviews) and works in its own worktree.
Fix what it finds, or say in the PR why not, before asking for the merge; mention in the PR that the
independent review was done and what it found.

### PR description template

```markdown
## Summary
Why this change: the problem and the outcome, in 2–4 sentences.

## What changed
- Per file or area, what was changed and why.

## Tasks closed
- Task.md items ticked, bug ids (e.g. B5, B7).

## How it was verified
- Commands run and their results (build, test, clippy, rgbasm/rgblink of affected examples).

## Generated-asm impact
- Which examples' output changes and how (attach a short diff excerpt if relevant), or "none".

## Risks and follow-ups
- Breaking changes, open questions, what is left for a later PR.

## Independent review
- What the context-free reviewer found and which commit fixed each finding (or why not), or "no findings".

## How to review
- Suggested reading order / what to focus on.
```
