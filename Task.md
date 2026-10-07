# rust-boy — Refactoring Tasks

The actionable roadmap. Background, the levels assessment and the full bug catalogue (B1–B30, with
file:line, failure scenario and fix) are in [`CONTEXT.md`](CONTEXT.md). Working rules are in
[`CLAUDE.md`](CLAUDE.md).

## How we work

- `refactor` is the integration branch (created from `main` at `4601a5c`).
- Each phase, or each major edit inside a phase, is done on its own branch cut from `refactor`,
  named `refactor-<phase>-<topic>` (e.g. `refactor-p1-if-semantics`), and merged through a **PR into
  `refactor`** for review.
- Tick the boxes in this file **in the same PR** that completes the task.
- When all phases are done: one PR `refactor` → `main`.

Priorities: **P0** broken today · **P1** real bug users will hit · **P2** latent / minor.
Bug ids link to [`CONTEXT.md`](CONTEXT.md#4-bug-catalogue).

---

## Phase 0 — Housekeeping & prerequisites

- [x] Create `Task.md` and `CONTEXT.md` (branch `refactor-p0-plan-docs`)
- [x] **Add `CLAUDE.md`** — commands, layer rules, working style, git/PR workflow (branch `refactor-p0-plan-docs`)
- [x] **P0** Fix `cargo test` compilation — [B1](CONTEXT.md#b1) (branch `refactor-p0-fix-build`)
- [x] **P0** Fix the `coin-anim` binary — [B2](CONTEXT.md#b2) (branch `refactor-p0-fix-build`)
- [ ] Deterministic output: replace `HashMap`/`HashSet` iteration with `BTreeMap`/ordered `Vec` — [B13](CONTEXT.md#b13).
      *Prerequisite for every snapshot test.*
- [x] Decide the `If` semantics once: `If::lt(l, r)` means `l < r` (documented meaning) — decided 2026-10-07. Needed before
      fixing [B5](CONTEXT.md#b5), because the two Unbricked examples use opposite argument orders.
- [ ] CI (GitHub Actions): `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, build all bins,
      and **required** `rgbasm` + `rgblink` of every example's output. Pin the RGBDS version (≥ 0.9; the
      committed objects are RGBDS 1.0).
- [ ] One in-crate `hardware.inc` (v4.x) instead of the 6 copies under `examples/`
- [ ] Fix the 10 compiler warnings (unused `MemoryAllocator`, `needs_special_handling`, `is_used`, unused
      import `JumpTarget`, private `SpriteData` leaking through `SpriteManager::get`, dead fields)
- [ ] `Cargo.toml`: `rust-version = "1.85"` (edition 2024), description, license, repository
- [ ] Untrack the 12 committed `*.gb` / `*.o` files, extend `.gitignore`; keep the example `.asm` files as
      golden fixtures and keep `.2bpp` / `.png` / `.aseprite` assets
- [ ] Delete `examples/if_example.rs.old`
- [ ] README refresh — [B29](CONTEXT.md#b29), [B21](CONTEXT.md#b21): bin list (6), `rust_boy` quick start, project tree,
      remove "complete instruction set" / "compile-time guarantees" claims, drop `rgbasm -L`, state the
      RGBDS version, remove "Current branch: gbz80-std"
- [ ] Branch cleanup — **approved** by the maintainer on 2026-10-07 (the 7 branches marked *Delete* in
      [Branches](#branches)). The Claude session cannot delete branches (GitHub answered HTTP 403), so the
      maintainer deletes them, on GitHub → Branches or with:
      `git push origin --delete unbricked-example fosdem-example test-animation rust-boy-implementation gbz80-std gbz80-workbench-more-idiomatic gbz80-workbench`
- [ ] *(optional)* Mirror this file into GitHub issues + milestones

## Phase 1 — Bug fixes

Every fix comes with a test (unit or snapshot) whose generated asm **assembles** in CI.

### P0 — broken in shipped examples
- [ ] Duplicate `wCurKeys`/`wNewKeys` labels; variable names never deduplicated — [B3](CONTEXT.md#b3)
- [ ] `LCDCF_OBJ16` forced → 8×8 sprites render wrong; add sprite-size config + even tile alignment in 8×16 — [B4](CONTEXT.md#b4)
- [x] Two-operand `If` compares right-vs-left; fix `If`, then both Unbricked examples (paddle bounce in
      `unbricked_std` never fires) — [B5](CONTEXT.md#b5) (branch `refactor-p1-if-semantics`; the left operand
      must not change `b`, now documented — a register-safe `If` is in Phase 2)
- [ ] Composite 16×16 sprite collapses at screen edges (FOSDEM demo) — [B6](CONTEXT.md#b6)

### P1
- [ ] Fixed global labels in reusable snippets (`check_key`, `move_*_limit`, gb_std `Sprite::move_*`) and
      global labels breaking `If` local-label scope — [B7](CONTEXT.md#b7)
- [ ] `move_*_limit` stops only on exact equality → overshoot and wrap — [B8](CONTEXT.md#b8)
- [ ] `jr` out of range in the animation dispatcher (≥ 3 animated sprites) — [B9](CONTEXT.md#b9)
- [ ] `AnimationType::PingPong` / `Once` silently behave as `Loop` — [B10](CONTEXT.md#b10)
- [ ] `gb.init()` code overwritten by variable initialisation — [B11](CONTEXT.md#b11)
- [ ] Direct OAM access from the main loop (no shadow OAM / DMA) — [B12](CONTEXT.md#b12) *(implementation in Phase 3 graphics)*

### P2
- [ ] `build()` not idempotent — [B14](CONTEXT.md#b14)
- [ ] `raw()` drops non-`Main` chunks; raw code is unreachable — [B15](CONTEXT.md#b15)
- [ ] `Var::set`/`get` ignore 16-bit variables — [B16](CONTEXT.md#b16)
- [ ] No bounds / overflow checks (VRAM, OAM, u8 tile counter at 256, animation freeze at tile 255) — [B17](CONTEXT.md#b17)
- [ ] Sprite and tile counters can desync — [B18](CONTEXT.md#b18)
- [ ] Every tilemap at `$9800`; add `$9C00` — [B19](CONTEXT.md#b19)
- [ ] Silent failures on unknown ids / animation-name typos — [B20](CONTEXT.md#b20)
- [ ] `basic_usage` + README header without `ds $150 - @, 0` — [B21](CONTEXT.md#b21)
- [ ] `get_pivot` clamps out-of-range offsets to 0 — [B22](CONTEXT.md#b22)
- [ ] Duplicated, diverged builtins (`GetTileByPixel` with two contracts) — [B23](CONTEXT.md#b23)
- [ ] Unused user functions always emitted (after B26) — [B24](CONTEXT.md#b24)
- [ ] Animation labels not namespaced by sprite; validate label names — [B25](CONTEXT.md#b25)
- [ ] Builtins reached through `Call`/`IfCall`/`define_function_from` not auto-included → link error — [B26](CONTEXT.md#b26)
- [ ] `Memcopy` with length 0 copies 64 KiB — [B27](CONTEXT.md#b27)
- [ ] `OBP1` never initialised; OAM not cleared when there are no sprites — [B28](CONTEXT.md#b28)
- [ ] Code-level doc errors (`inputs.rs` pressed bit, `RustBoy::call` example E0499, unsigned `If` note) — [B29](CONTEXT.md#b29)

## Phase 2 — Refactor the levels

See [CONTEXT.md §3](CONTEXT.md#3-are-the-levels-correct-assessment) for the reasoning.

- [ ] `hw` module: pure data (registers, flags, OAM layout, VRAM map) emitted as `hardware.inc` symbol
      names; no more `"_OAMRAM+N"` / `"$9800"` / LCDC strings in `std` or `engine`
- [ ] Move `Emittable` into the asm layer; add a `Block` instruction buffer (stop using `Asm` + `get_main_instrs()` as scratch)
- [ ] Typed operands: remove string-register helpers (`inc_label("de")`, `ld_hli_label("a")`, …); add an
      `Expr` operand for constants/expressions; reject invalid destinations
- [ ] Consistent instruction shapes (`And`/`Cp` vs `Or`/`Xor`/`Sub`, `AdcA` vs `Adc`); derive `Debug`, `PartialEq` on `Instr`
- [ ] Complete the ISA (needed by interrupts, DMA, 16-bit math, audio): `push`/`pop`, `halt`, `stop`,
      `nop`, `di`/`ei`, `reti`, `rst`, `sbc`, `bit`/`set`/`res`, `rl`/`rr`/`rlc`/`rrc`/`sla`/`sra` (+ `rla`…),
      `cpl`, `scf`/`ccf`, `ld [hl-]`, `ld hl, sp+e`, `jp hl`, `add sp, e`
- [ ] First-class sections (type, bank, `ALIGN`, `ds n` without fill for RAM); move `Chunk` and the game
      layout out of `gb_asm` into the engine
- [ ] Label allocator owned by the asm layer; automatic `jr` → `jp` when out of range
- [ ] Routines as values: `Routine { name, body, deps, clobbers }` → automatic inclusion of dependencies
      (fixes B26) and a documented calling convention (which registers each routine clobbers)
- [ ] One source of truth for builtins: `rust_boy` reuses `gb_std`; remove the duplicate `gb_std::graphics::sprites::SpriteManager`
- [ ] `build(&self) -> Result<String, Error>`; loud errors instead of empty `Vec`s
- [ ] `RustBoyConfig` (sprite size, palettes, LCDC flags, which builtins)
- [ ] Use `MemoryAllocator` for VRAM / WRAM / OAM / HRAM
- [ ] `If` that never clobbers user registers (or documents what it uses)
- [ ] `prelude` module; avoid the `rust_boy::rust_boy` stutter (optional rename: `asm` / `std` / `engine`)
- [ ] Ship all breaking API changes together in one release

## Phase 3 — Missing features

### Graphics
- [ ] Shadow OAM in WRAM + OAM DMA routine in HRAM (fixes [B12](CONTEXT.md#b12))
- [ ] VRAM write queue flushed during VBlank (tilemap edits, score)
- [ ] Background scrolling (`SCX`/`SCY`), camera helpers
- [ ] Window layer (`WX`/`WY`, `$9C00` map)
- [ ] Palette API (`BGP`, `OBP0`, `OBP1`) + fade in / fade out
- [ ] Typed sprite flags (flip X/Y, priority, palette)
- [ ] Choice of 8×8 or 8×16 sprites; metasprites of any size (generalise 16×16)
- [ ] Sprite show / hide, OAM slot allocation
- [ ] Text: font loading, print string, print number / BCD score
- [ ] Background tile animation
- [ ] PNG → 2bpp conversion in Rust (tile count derived from file size)

### Animation
- [ ] `PingPong` and `Once` ([B10](CONTEXT.md#b10))
- [ ] Per-animation speed (today only a global delay)
- [ ] End-of-animation events / callbacks

### Audio *(nothing exists today)*
- [ ] APU init (sound on/off at boot), register constants in `hw`
- [ ] Sound-effect API for channels 1–4 (`play_sfx`)
- [ ] Music: integrate a driver (e.g. hUGEDriver)

### Input
- [ ] Just-pressed (`wNewKeys`), released, and held as separate bindings
- [ ] Auto-repeat
- [ ] Multiple bindings for the same button

### Engine
- [ ] VBlank interrupt + `halt` main loop (instead of busy-polling `LY`)
- [ ] Interrupts API (VBlank, STAT, Timer, Joypad) and timers
- [ ] Scenes / game states
- [ ] Entities: spawn / despawn
- [ ] Fixed-point sub-pixel movement, velocity, gravity
- [ ] WRAM arrays, HRAM variables
- [ ] RNG
- [ ] Sprite-vs-sprite collision (AABB), generalised tile collision
- [ ] 16-bit math helpers (add/sub/compare, multiply/divide)
- [ ] Control flow: `While`, `For`, `Switch`; functions with parameters
- [ ] Soft reset (A+B+Start+Select)

### Platform
- [ ] MBC1/MBC5 ROM banking (> 32 KB)
- [ ] SRAM save games
- [ ] *(optional)* Game Boy Color: palettes, VRAM bank 1, double speed

### Tooling
- [ ] One-command build & run: produce `.gb` + `.sym` + `.map` via rgbasm/rgblink/rgbfix and open an emulator
- [ ] Snapshot tests of generated asm (after [B13](CONTEXT.md#b13))
- [ ] Headless-emulator tests (run the ROM, assert on memory/registers)
- [ ] Asm comments pointing back to the Rust source (`#[track_caller]`)
- [ ] ROM-size and cycle-budget report (e.g. "main loop exceeds VBlank")
- [ ] Peephole optimisations (`ld a, 0` → `xor a`, `cp 0` → `and a`, …)
- [ ] Validate user-supplied symbol names

## Phase 4 — Documentation

- [ ] Fix the inaccuracies in the `documentations` branch ([B30](CONTEXT.md#b30)), then fast-forward merge it
- [ ] Rustdoc on every public item; turn ```` ```ignore ```` examples into compiled doctests
- [ ] Architecture / levels document (from [CONTEXT.md §3](CONTEXT.md#3-are-the-levels-correct-assessment))
- [ ] Calling-convention document (registers each routine uses/clobbers)
- [ ] Tutorial: "your first game" step by step with `RustBoy`
- [ ] Hardware notes: VBlank & OAM timing, 8×16 rules, coordinate offsets (+8 / +16)
- [ ] `CHANGELOG.md`
- [ ] Keep `CLAUDE.md` and `CONTEXT.md` up to date as phases land

## Future

- [ ] **Move the examples to a separate repository** (e.g. `rust-boy-examples`): the example binaries,
      their assets (`.png`, `.aseprite`, `.2bpp`, `hardware.inc`) and golden `.asm` files; depend on
      `rust-boy` via git. Keep 1–2 minimal Cargo `examples/` here for CI.
- [ ] Publish on crates.io (move `src/bin` → `examples/` first)

---

## Branches

Verified with `git rev-list --count` against `origin/main`. Deletion of the 7 *Delete* branches was
approved on 2026-10-07 and is waiting for the maintainer (see Phase 0). A deleted branch can be restored
with `git push origin <last commit>:refs/heads/<branch>`.

| Branch | Last commit | Ahead / behind | Action |
|---|---|---|---|
| `documentations` | `8fe484d` | 1 / 0 | **Keep** — 7 good docs; fix [B30](CONTEXT.md#b30), then fast-forward merge (Phase 4) |
| `unbricked-example` | `a7167ea` | 3 / 57 | **Delete** — superseded experiment with the external `retroshield-z80-workbench` crate (optionally tag `archive/unbricked-example` first) |
| `fosdem-example` | `4601a5c` | 0 / 0 | **Delete** — same commit as `main` |
| `test-animation` | `3e471a4` | 0 / 11 | **Delete** — fully merged |
| `rust-boy-implementation` | `41d3eb1` | 0 / 12 | **Delete** — fully merged (tag `v0.2.0-poc` keeps the milestone) |
| `gbz80-std` | `fc63483` | 0 / 45 | **Delete** — fully merged |
| `gbz80-workbench-more-idiomatic` | `a9a4381` | 0 / 51 | **Delete** — fully merged |
| `gbz80-workbench` | `ae304a1` | 0 / 57 | **Delete** — fully merged |
