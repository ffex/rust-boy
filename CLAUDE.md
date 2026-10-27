# CLAUDE.md

rust-boy is a Rust DSL that generates Game Boy (DMG) assembly in RGBDS syntax (`cargo run` → `.asm` →
`rgbasm`/`rgblink`/`rgbfix` → `.gb`). On this branch we use it to **make games**: each game is an example
binary written with the public API (`rust_boy::prelude`, the `RustBoy` engine). The current game is
**olive-empire** (`src/bin/olive-empire/`, art in `examples/olive-empire/`).

- [`Task.md`](Task.md) — checklist of the library work, done and open.
- [`CONTEXT.md`](CONTEXT.md) — architecture, levels assessment, bug catalogue B1–B30.
- [`CHANGELOG.md`](CHANGELOG.md) — user-visible changes.

Read the relevant parts before a task, and keep them up to date in the same branch as the change.

## How we work

We work together live, pair-programming style.

- **I decide what the game does**; you write the code, explain the Game Boy side when it matters (VBlank/OAM
  timing, tile maps, the +8/+16 sprite offsets, the cost in cycles and ROM bytes) and suggest better ways
  to do things.
- **Small steps that build and run.** After each step run `cargo build`, then `scripts/run.sh --run <bin>` so
  I can see it in the emulator, and say in one line what changed.
- **Ask me about game design** (controls, rules, art, sizes, level layout). Do not ask about anything the
  code or this file already settles.
- When a step doesn't need my input, keep going; put status notes in the same message as the next action.
  Stop and ask only when you can't continue without me, or before anything destructive: deleting data,
  force-pushing, or changing anything outside this repository.
- Once you have answered something, treat that answer as done; don't go back over it unless I ask.
- State which checks ran and their results; never describe a skipped check as passing.

### Game code vs library

- Game code uses the public API. Raw `asm` only where the API has no way to do it yet, and say so when you
  use it.
- When the game hits a **bug or a missing feature in the library**, stop and tell me: what is missing,
  where (`file:line`), and a proposal. Do not quietly work around it in the game or patch the library.
  A library fix we agree on follows the [architecture rules](#architecture-rules), gets a test, and is
  noted in `Task.md` (and `CHANGELOG.md`). Things we postpone go in `Task.md` as open items.
- Art (`.png` in `examples/<game>/`): use the `gb-pixel-art` skill to draw, fix or check it.

## Game Boy cheat sheet (DMG)

| | |
|---|---|
| Screen | 160 × 144 px = 20 × 18 tiles of 8 × 8 |
| Tilemaps | two, 32 × 32 tiles (256 × 256 px), at `$9800` (default) and `$9C00`; scroll with SCX/SCY |
| Background tiles | 128 in the library (`$9000`–`$97FF`), 16 bytes each; the tilemap holds 1 byte per cell |
| Sprite tiles | 256 (`$8000`–`$8FFF`); sprites are 8 × 8 or 8 × 16 (one size for the whole game) |
| Sprites | 40 in OAM, at most 10 per scanline (the rest are not drawn); OAM X = screen X + 8, Y = screen Y + 16 |
| Colours | 4 per palette: PNG index 0–3 = `#FFFFFF`, `#AAAAAA`, `#555555`, `#000000` (`PngOptions::default()`); sprite colour 0 is transparent, background colour 0 is not |
| Timing | ~59.7 frames/s; VRAM and OAM are written in VBlank (the engine's shadow OAM and VRAM queue do it) |

A background made of a few repeated tiles fits easily; a full-screen picture of unique tiles (360) does not
(PNG → unique tiles + tilemap is an open item in `Task.md`).

## Commands

```bash
cargo build                               # library + all bins
cargo test                                # unit tests, doctests and the snapshot tests
UPDATE_SNAPSHOTS=1 cargo test --test snapshots   # rewrite examples/<bin>/main.asm after an intended
                                          # change of the generated asm; commit them with it
cargo clippy --all-targets -- -D warnings # lint
cargo fmt                                 # format
cargo run --bin <name> > main.asm         # bins: basic_usage, unbricked, unbricked_std,
                                          #       unbricked_rustboy, fosdem, coin-anim, music,
                                          #       olive-empire
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

Before saying a step is done, run `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test`,
and report the result of each.

Headless-emulator tests (`tests/emulator.rs`): `support::rom::Rom::build(asm, include_dirs)` (or
`Rom::example(bin)`) assembles and links a program with RGBDS; `rom.boot()` gives a `support::gameboy::GameBoy`
(zero-dependency DMG emulator in `tests/support/gameboy.rs`) to run frames with scripted input
(`run_script(&[(frames, &[Button::Left])])`) and read WRAM/HRAM/VRAM/OAM (`read_symbol("wScore")`, `oam_entry(0)`),
the registers, and every OAM write with its PPU mode (`oam_writes()`). Without `RGBDS_LINK_CHECK` these tests
return early. Useful for game logic too: script the input and check where the player ended up.

Assembling generated output by hand (RGBDS ≥ 0.9). `include/hardware.inc` (v4.x) and the example's directory,
for its assets, go on the include path with `-I`:

```bash
rgbasm -I include -I examples/olive-empire -o main.o main.asm   # no `-L`: it was removed in RGBDS 0.8
rgblink -o main.gb main.o
rgbfix -v -p 0xFF main.gb
```

No RGBDS installed? Build it from the official source:
`git clone --depth 1 --branch v1.0.4 https://github.com/gbdev/rgbds` and run
`make rgbasm rgblink rgbfix` in that directory (needs a C++ compiler, bison and libpng).

## Adding a game (example binary)

A new binary `src/bin/<name>/main.rs` (assets in `examples/<name>/`) has to be registered, or the tests fail.
Grep for an existing example name (e.g. `coin-anim`) to find every list. Today these are:

- `tests/support/examples.rs` and `tests/report.rs`: the `EXAMPLES` arrays;
- `tests/snapshots.rs`: the snapshot-test macro list;
- `scripts/assemble-examples.sh`: the `examples` array;
- the bins list under [Commands](#commands), and `CHANGELOG.md`.

Each example's generated asm is committed as `examples/<bin>/main.asm`: the snapshot that `tests/snapshots.rs`
compares with the binary's output. Write it with `UPDATE_SNAPSHOTS=1 cargo test --test snapshots` and commit it
with the change that alters the asm (every game step does).

## Architecture rules

These apply to any change of the library (`src/` outside `src/bin/`).

- Layers: `asm` (instructions, `src/asm/`) → `stdlib` (stateless routines, `If`/flow control, `src/stdlib/`) →
  `engine` (`RustBoy` and its managers, `src/engine/`), with `hw` (pure data, `src/hw.rs`) beside them and
  `prelude` (re-exports). A layer depends only on the layers below it; `asm` does not use `hw` (see `CONTEXT.md`
  §3).
- Every routine (Memcopy, WaitVBlank, UpdateKeys, …) exists **once**. Do not copy a routine into another layer
  or into a game. The exceptions, decided by the maintainer: `src/bin/unbricked.rs` (the tutorial) and
  `src/bin/basic_usage.rs` (the minimal example) are written with raw `asm` and keep their own routines.
- Generated output must be **deterministic**: never let `HashMap`/`HashSet` iteration order reach the
  output; use `BTreeMap` or an ordered `Vec`. Sprites, tiles, variables and functions are emitted in the
  order they were created.
- Generated labels must be unique and must not break RGBDS local-label scope (a global label inside an
  `If` body breaks `.end_if_N`). Prefer local labels or the label allocator.
- Do not hardcode hardware addresses/flags as strings in new code; use (or add to) the `hw` constants.
- Every bug fix gets a test; when it changes generated asm, the asm must still assemble.
- A change to an example's generated asm updates its snapshot `examples/<bin>/main.asm` in the same commit.
  Snapshots are updated only for an intended asm change, never to make a test pass.
- Never commit build artifacts (`*.gb`, `*.o`, `target/`) or `.DS_Store`.

## Git

- `develop` is the branch for game development. `main` holds only the maintainer's hand-written work and is
  not touched (tag `human-bkp` marks it); `refactor` is the library's integration branch.
- Work on `develop`, or on a short branch off it named after the game or feature (e.g.
  `olive-empire-collisions`). Never commit to `main` or `refactor`.
- Commit in small, coherent commits when a step works, with a short message. Do not push or open a PR until
  I ask; a PR then targets `develop` and I merge it.
- A library change made here that `refactor` should also get goes there through its own PR; tell me when
  one is pending.
- Never force-push, rewrite published history or delete branches or tags without asking first. Do not mention
  or link AI sessions in commit messages or PR descriptions.

### PR description template

```markdown
## Summary
Why this change: the problem and the outcome, in 2–4 sentences.

## What changed
- Per file or area, what was changed and why.

## How it was verified
- Commands run and their results (build, test, clippy, the ROM in the emulator).

## Generated-asm impact
- Which examples' output changes and how, or "none".

## Risks and follow-ups
- Breaking changes, open questions, what is left for later.
```
