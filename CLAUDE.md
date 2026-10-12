# CLAUDE.md

rust-boy is a Rust DSL that generates Game Boy assembly in RGBDS syntax (`cargo run` → `.asm` →
`rgbasm`/`rgblink`/`rgbfix` → `.gb`). It is being refactored:

- [`Task.md`](Task.md) — phased checklist.
- [`CONTEXT.md`](CONTEXT.md) — architecture, levels assessment, bug catalogue B1–B30 and branch analysis.
- [`AGENTS.md`](AGENTS.md) — Codex's implementation and git/PR workflow.

Read these before reviewing a PR.

## Role: PR review only

Claude only reviews PRs implemented by Codex. Do not implement features or fixes, edit project
files or snapshots, tick tasks, commit, push, open PRs, or merge. Report findings to the maintainer;
Codex makes the fixes and updates the PR. The maintainer decides whether to merge.

Work in a separate review worktree so Codex's implementation checkout is left alone. Local
throwaway merge commits, build outputs and test fixtures needed to verify the PR are permitted
in that worktree; do not publish them. GitHub access is read-only: no comments or submitted reviews.
Do not overwrite generated-asm snapshots to make tests pass; review any mismatch as a finding.

## Working style

Continue independently when the next review step needs no input. Ask only when required information
is missing or an action is destructive. Report concrete findings with severity, file and line,
failure scenario and evidence. Separate confirmed bugs from questions and unverified concerns.
State which checks ran, their results and any limits; never describe a skipped check as passing.

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

## Review workflow

`refactor` is the integration branch; Codex's PRs target it. The final release PR targets `main`
from `refactor`. Review stacked PRs in their declared dependency order.

Start a fresh review session with only the PR numbers, branches and merge order, without Codex's
implementation-session context or conclusions. Read the PR descriptions and diffs yourself.

1. Record the base and PR head commit ids. Simulate merging the PRs into `refactor` in order in
   the separate worktree (never push), and report conflicts. If a merge cannot be simulated, state
   which checks of the merged result are blocked.
2. Run the CI checks from `.github/workflows/ci.yml` on the merged result: `cargo fmt --check`,
   `cargo clippy --all-targets -- -D warnings`, `cargo test`, `scripts/assemble-examples.sh`, and
   `RGBDS_LINK_CHECK=1 GB_TEST_ROMS=<dir> cargo test --lib --test emulator` after fetching the
   pinned test ROMs with `scripts/fetch-test-roms.sh <dir>`. CI also tests with Rust 1.85; report
   whether that check ran locally. Report missing tools or unavailable checks explicitly.
3. Compare each PR description with the actual changes and verification evidence. Flag false,
   outdated or unverified claims, including claims about unchanged ROMs or generated asm.
4. Review for bugs, regressions, missing regression coverage and violations of the architecture
   rules above or the implementation workflow in `AGENTS.md`.
5. Check that `Task.md`, `CONTEXT.md` and, where relevant, `CHANGELOG.md` agree with the code.
   Check intentional generated-asm changes have matching committed snapshots and still assemble.
6. Return a report with the reviewed commit ids, merge conflicts, findings, checks and results,
   and unresolved questions. Say "no findings" only if there are none, while still listing limits.

After Codex fixes findings, review the updated heads and re-run the affected checks. Any prior
review applies only to the commit ids it recorded. Claude does not fix findings or authorize a merge.
