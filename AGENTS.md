# AGENTS.md

rust-boy is a Rust DSL that generates Game Boy assembly in RGBDS syntax (`cargo run` → `.asm` →
`rgbasm`/`rgblink`/`rgbfix` → `.gb`). It is being refactored:

- [`Task.md`](Task.md) — what to do next (phased checklist).
- [`CONTEXT.md`](CONTEXT.md) — why: architecture, levels assessment, bug catalogue B1–B30, branch analysis.

Read both before starting a task, and keep them up to date in the same branch as the change.

## Role: first implementation with Codex

Codex makes the **first try** of a task: implementation, tests, documentation, snapshots, a branch,
commits and a PR into `refactor`. Then Codex hands the PR to Claude and stops working on it.
Claude, following [`CLAUDE.md`](CLAUDE.md), has the PR reviewed, fixes the findings on the PR
branch, and merges it into `refactor` when everything is OK. Only the final `refactor` → `main` PR
needs the maintainer. Do not treat a Codex self-review as Claude's independent review.

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
cargo test                                # unit tests, doctests and the snapshot tests
UPDATE_SNAPSHOTS=1 cargo test --test snapshots   # rewrite examples/<bin>/main.asm after an intended
                                          # change of the generated asm; commit them with it
cargo clippy --all-targets                # lint
cargo fmt                                 # format
cargo run --bin <name> > main.asm         # bins: basic_usage, unbricked, unbricked_std,
                                          #       unbricked_rustboy, fosdem, coin-anim
scripts/run.sh <bin>                      # one bin -> target/examples/<bin>/main.{gb,sym,map,asm} via RGBDS
scripts/run.sh --run <bin>                # ... and open it in an emulator ($GB_EMULATOR, sameboy, mgba, gambatte,
                                          # bgb (also through wine), pyboy); none found: prints where the files are
scripts/assemble-examples.sh              # every example -> target/examples/<bin>/ with scripts/run.sh (needs RGBDS;
                                          # under $CARGO_TARGET_DIR when it is set)
RGBDS_LINK_CHECK=1 cargo test --lib --test emulator   # with RGBDS: link the test programs, and build ROMs
                                          # and run them on the test emulator (tests/emulator.rs)
scripts/fetch-test-roms.sh <dir>          # Blargg's test ROMs (pinned, checksummed); with GB_TEST_ROMS=<dir>,
                                          # tests/emulator.rs also checks the emulator against them
```

Headless-emulator tests (`tests/emulator.rs`): `support::rom::Rom::build(asm, include_dirs)` (or
`Rom::example(bin)`) assembles and links a program with RGBDS; `rom.boot()` gives a `support::gameboy::GameBoy`
(zero-dependency DMG emulator in `tests/support/gameboy.rs`) to run frames with scripted input
(`run_script(&[(frames, &[Button::Left])])`) and read WRAM/HRAM/VRAM/OAM (`read_symbol("wScore")`, `oam_entry(0)`),
the registers, and every OAM write with its PPU mode (`oam_writes()`, for [B12](CONTEXT.md#b12)). Without
`RGBDS_LINK_CHECK` these tests return early; CI runs them.

Each example's generated asm is committed as `examples/<bin>/main.asm`, next to its assets: the snapshot that
`tests/snapshots.rs` compares with the binary's output, which fails with a diff when they differ.

Assembling generated output (RGBDS ≥ 0.9). `include/hardware.inc` (v4.x) and the example's directory, for its
`.2bpp` assets, go on the include path with `-I`:

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
- A change to an example's generated asm updates its snapshot `examples/<bin>/main.asm` in the same commit
  (`UPDATE_SNAPSHOTS=1 cargo test --test snapshots`); the PR shows the asm change in that diff.
- Never commit build artifacts (`*.gb`, `*.o`, `target/`).

## Git workflow

- `refactor` is the integration branch. Never commit directly to `main` or `refactor`.
- For each phase, or each major edit, create a branch from an up-to-date `refactor`:
  `refactor-<phase>-<topic>`, e.g. `refactor-p1-if-semantics`, `refactor-p2-typed-operands`.
  (Use a dash, not a slash: git cannot have both a `refactor` branch and `refactor/...` branches.)
- If the work needs a PR that is not merged yet, branch from that PR's branch instead (stacked PR),
  still target `refactor`, and start the PR description with "Depends on #N".
- Small, focused commits with clear messages. Tick the matching boxes in `Task.md` in the same branch, and
  record user-visible and breaking changes in `CHANGELOG.md` (with the migration notes).
- Before pushing, run what CI runs (`.github/workflows/ci.yml`): `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`, `scripts/assemble-examples.sh`, and
  `RGBDS_LINK_CHECK=1 GB_TEST_ROMS=<dir> cargo test --lib --test emulator` (after `scripts/fetch-test-roms.sh <dir>`),
  and `cargo +1.85 test` too when Rust 1.85 is installed (CI's MSRV job).
- Push the branch and open a **PR into `refactor`** (never straight into `main`), then hand it to Claude
  (below). Claude reviews, fixes and merges it. When everything is done, `refactor` → `main` is the
  final PR, which only the maintainer merges.
- Never force-push, rewrite published history or delete branches without asking first.
- Do not mention or link AI sessions in commit messages or PR descriptions.

### Handoff to Claude

When the PR is open and its CI checks pass locally (or the ones that could not run are listed as not run),
hand it to Claude: give the maintainer the PR
number, the branch and, for stacked PRs, the merge order, and leave the PR's *Independent review*
section as "pending" (Claude also picks up open PRs marked so). Do not push to the branch after the handoff, unless
the maintainer asks you to (Claude's requests reach you through the maintainer). Questions go to the maintainer. If something is unfinished or uncertain, say so in the PR's
*Risks and follow-ups* section rather than in the handoff, so the reviewer reads it independently.
Claude fills in the *Independent review* section.

In *How it was verified*, list each command you ran and its result. Never describe a skipped check as
passing: name the checks that did not run and why.

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
- "Pending" when the PR is opened; Claude records what each review found, the reviewed commit ids,
  and which commit fixed each finding (or why not).

## How to review
- Suggested reading order / what to focus on.
```
