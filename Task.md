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
- [x] Deterministic output: replace `HashMap`/`HashSet` iteration with `BTreeMap`/ordered `Vec` — [B13](CONTEXT.md#b13).
      *Prerequisite for every snapshot test.* (branch `refactor-p0-deterministic-output`; output follows creation order)
- [x] Decide the `If` semantics once: `If::lt(l, r)` means `l < r` (documented meaning) — decided 2026-10-07. Needed before
      fixing [B5](CONTEXT.md#b5), because the two Unbricked examples use opposite argument orders.
- [x] CI (GitHub Actions): `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, build all bins,
      and **required** `rgbasm` + `rgblink` of every example's output. Pin the RGBDS version (≥ 0.9; the
      old committed objects were RGBDS 1.0). (branch `refactor-p0-ci`: `.github/workflows/ci.yml` +
      `scripts/assemble-examples.sh`, RGBDS v1.0.4, a Rust 1.85 job for `rust-version`)
- [x] One in-crate `hardware.inc` (v4.x) instead of the 6 copies under `examples/` — now `include/hardware.inc` (branch `refactor-p0-repo-hygiene`)
- [x] Fix the 10 compiler warnings (unused `MemoryAllocator`, `needs_special_handling`, `is_used`, unused
      import `JumpTarget`, private `SpriteData` leaking through `SpriteManager::get`, dead fields), and
      the clippy lints: `cargo clippy --all-targets -- -D warnings` passes (branch `refactor-p0-warnings`)
- [x] `Cargo.toml`: `rust-version = "1.85"` (edition 2024), description, license, repository (branch `refactor-p0-repo-hygiene`)
- [x] Untrack the 12 committed `*.gb` / `*.o` files, extend `.gitignore`; keep the example `.asm` files as
      golden fixtures and keep `.2bpp` / `.png` / `.aseprite` assets (branch `refactor-p0-repo-hygiene`)
- [x] Delete `examples/if_example.rs.old` (branch `refactor-p0-repo-hygiene`)
- [x] README refresh — [B29](CONTEXT.md#b29), [B21](CONTEXT.md#b21): bin list (6), `rust_boy` quick start, project tree,
      remove "complete instruction set" / "compile-time guarantees" claims, drop `rgbasm -L`, state the
      RGBDS version, remove "Current branch: gbz80-std" (branch `refactor-p0-readme`); the README is also the crate docs, so its
      Rust examples are compiled as doctests
- [x] Branch cleanup — the 7 branches marked *Delete* in [Branches](#branches) were deleted by the
      maintainer on 2026-10-07
- [ ] *(optional)* Mirror this file into GitHub issues + milestones

## Phase 1 — Bug fixes

Every fix comes with a test (unit or snapshot) whose generated asm **assembles** in CI.

### P0 — broken in shipped examples
- [x] Duplicate `wCurKeys`/`wNewKeys` labels; variable names never deduplicated — [B3](CONTEXT.md#b3) (branch `refactor-p1-duplicate-vars`)
- [x] `LCDCF_OBJ16` forced → 8×8 sprites render wrong; add sprite-size config + even tile alignment in 8×16 — [B4](CONTEXT.md#b4)
      (branch `refactor-p1-sprite-size`: `RustBoy::set_sprite_size`, 8×8 by default; `fosdem` opts into 8×16)
- [x] Two-operand `If` compares right-vs-left; fix `If`, then both Unbricked examples (paddle bounce in
      `unbricked_std` never fires) — [B5](CONTEXT.md#b5) (branch `refactor-p1-if-semantics`; since
      `refactor-p2-routines-if` the left operand may change `b`: the `If` saves `bc` around it when it may, and each
      `If` kind documents the registers it uses)
- [x] Composite 16×16 sprite collapses at screen edges (FOSDEM demo) — [B6](CONTEXT.md#b6)
      (branch `refactor-p1-sprite-limits`: the leading sprite is tested, the others follow at their offsets)

### P1
- [x] Fixed global labels in reusable snippets (`check_key`, `move_*_limit`, gb_std `Sprite::move_*`) and
      global labels breaking `If` local-label scope — [B7](CONTEXT.md#b7) (branch `refactor-p1-unique-labels`:
      numbered local labels from `gb_asm::LabelAllocator`; `gb_std` `check_key` / `Sprite::move_*_limit` take one)
- [x] `move_*_limit` stops only on exact equality → overshoot and wrap — [B8](CONTEXT.md#b8)
      (branch `refactor-p1-sprite-limits`: carry compares; a move stops exactly on its limit, which is included)
- [x] `jr` out of range in the animation dispatcher (≥ 3 animated sprites) — [B9](CONTEXT.md#b9)
      (branch `refactor-p1-animations`: `jp` for every jump whose distance grows with the animations)
- [x] `AnimationType::PingPong` / `Once` silently behave as `Loop` — [B10](CONTEXT.md#b10)
      (branch `refactor-p1-animations`: both implemented; `PingPong` keeps its direction in `wAnim_{sprite}_Dir`)
- [x] `gb.init()` code overwritten by variable initialisation — [B11](CONTEXT.md#b11)
      (branch `refactor-p1-init-order`: variables and palettes are set before the user code, LCD on stays last)
- [ ] Direct OAM access from the main loop (no shadow OAM / DMA) — [B12](CONTEXT.md#b12) *(implementation in Phase 3 graphics)*

### P2
- [x] `build()` not idempotent — [B14](CONTEXT.md#b14) (branch `refactor-p1-duplicate-vars`); `build(&self)` stays a Phase 2 item
- [x] `raw()` drops non-`Main` chunks; raw code is unreachable — [B15](CONTEXT.md#b15) (branch `refactor-p1-api-safety`:
      every chunk is kept, after the code generated for it; `Init` and `MainLoop` raw code runs; placement documented)
- [x] `Var::set`/`get` ignore 16-bit variables — [B16](CONTEXT.md#b16) (branch `refactor-p1-api-safety`: `set` takes
      any value of the variable's type and writes both bytes of a 16-bit one; `get` loads a 16-bit one into `hl`)
- [x] No bounds / overflow checks (VRAM, OAM, u8 tile counter at 256, animation freeze at tile 255) — [B17](CONTEXT.md#b17)
      (branch `refactor-p1-api-safety`: sprite tiles, background tiles, WRAM0 variables and OAM entries are allocated
      through `MemoryAllocator` and panic when full; sprite positions and animation frames are checked; `Loop` compares
      with its last frame)
- [x] Sprite and tile counters can desync — [B18](CONTEXT.md#b18) (branch `refactor-p1-api-safety`: a sprite's tile
      index comes from where the tile manager put its tiles; `SpriteManager::add` is no longer public)
- [x] Every tilemap at `$9800`; add `$9C00` — [B19](CONTEXT.md#b19) (branch `refactor-p1-api-safety`:
      `tiles.add_tilemap_at(name, TilemapArea::Map9C00, rows)`, `RustBoy::set_background_tilemap`; a second tilemap on
      one map, or more than 32 rows, panics)
- [x] Silent failures on unknown ids / animation-name typos — [B20](CONTEXT.md#b20) (branch `refactor-p1-api-safety`:
      every sprite / composite method that generates code or changes a sprite panics on an unknown id (the query
      `get_composite_sprites` returns `None`), and the animation methods on an unknown animation name or index, a sprite
      without animations, or a 256th animation)
- [x] `basic_usage` + README header without `ds $150 - @, 0` — [B21](CONTEXT.md#b21) (branch `refactor-p0-readme`)
- [x] `get_pivot` clamps out-of-range offsets to 0 — [B22](CONTEXT.md#b22) (branch `refactor-p1-api-safety`: one
      `gb_std` routine for both layers, wrapping arithmetic like the 256-pixel map; an offset beyond ±255 panics)
- [x] Duplicated, diverged builtins (`GetTileByPixel` with two contracts) — [B23](CONTEXT.md#b23)
      (branch `refactor-p1-builtins`: one `GetTileByPixel` in the library, in `gb_std`: `hl` = tile address and
      `a` = tile index; `rust_boy` emits the `gb_std` routines. Decided by the maintainer: the raw-`gb_asm`
      tutorial `src/bin/unbricked.rs` keeps its own copies (GetTileByPixel, Memcopy, UpdateKeys, and WaitVBlank), the one stated
      exception to "every routine exists once" (CLAUDE.md; `basic_usage.rs` is the second since `refactor-p2-routines-sprites`); its `GetTileByPixel` keeps the old contract, `hl` only)
- [x] Unused user functions always emitted (after B26) — [B24](CONTEXT.md#b24)
      (branch `refactor-p1-builtins`: only used functions, transitively; `RustBoy::keep_function` forces one)
- [x] Animation labels not namespaced by sprite; validate label names — [B25](CONTEXT.md#b25)
      (branch `refactor-p1-unique-labels`: `Anim_{sprite}_{animation}`; sprite, composite and animation names
      must be unique RGBDS identifiers)
- [x] Builtins reached through `Call`/`IfCall`/`define_function_from` not auto-included → link error — [B26](CONTEXT.md#b26)
      (branch `refactor-p1-builtins`: `build()` emits every function the generated code refers to, once, with its
      variables; `RustBoy::external_symbol` declares a routine defined outside, e.g. in an `INCLUDE`d file)
- [x] `Memcopy` with length 0 copies 64 KiB — [B27](CONTEXT.md#b27)
      (branch `refactor-p1-builtins`: `RustBoy` skips empty raw blobs, `from_file(path, 0)` panics; branch
      `refactor-p1-api-safety`, the maintainer's choice: `Memcopy` tests `bc` first, so a length of 0 copies nothing)
- [x] `OBP1` never initialised; OAM not cleared when there are no sprites — [B28](CONTEXT.md#b28)
      (branch `refactor-p1-init-order`: `rOBP1` = `%11100100` like `rBGP`/`rOBP0`; the OAM is always cleared)
- [x] Code-level doc errors (`inputs.rs` pressed bit, `RustBoy::call` example E0499, unsigned `If` note) — [B29](CONTEXT.md#b29) (branch `refactor-p0-readme`)

## Phase 2 — Refactor the levels

See [CONTEXT.md §3](CONTEXT.md#3-are-the-levels-correct-assessment) for the reasoning.

**Phase 2 is complete** (branch `refactor-p2-engine-api-prelude`, the last of the engine API group). The layers were
renamed there: `gb_asm` → `asm`, `gb_std` → `stdlib`, `rust_boy` → `engine` (`src/asm/`, `src/stdlib/`,
`src/engine/`); the items below keep the names of their time. Three follow-ups stay open below (marked *Follow-up*);
the next step is Phase 3, and the `refactor` → `main` PR, whose release notes are [CHANGELOG.md](CHANGELOG.md).

- [x] `hw` module: pure data (registers, flags, OAM layout, VRAM map) emitted as `hardware.inc` symbol
      names; no more `"_OAMRAM+N"` / `"$9800"` / LCDC strings in `std` or `engine` (branch `refactor-p2-hw`:
      `hw::Symbol`, a `hardware.inc` name and its value, for the I/O registers, their flags, the memory map, the OAM
      layout and the screen sizes; `gb_std` turns one into an `Expr` by its name; `gb_std`, `rust_boy` and the examples
      take every hardware fact from `hw`. Tests: each value `ASSERT`ed with RGBDS against `include/hardware.inc`, and a
      guard that fails on a hardware name or address written as a string outside `hw` (the `unbricked.rs` tutorial
      excepted). The 6 example ROMs, their asm, `.map` and `.sym` are byte-identical)
  - [ ] Follow-up: write the hardware facts the output still writes as numbers by their `hardware.inc` names, a
        cosmetic asm change (`ld bc, $9800` → `ld bc, _SCRN0` in `GetTileByPixel`, `ld a, 0` → `ld a, LCDCF_OFF`,
        `cp a, 144` → `cp a, SCRN_Y`, `ld b, 160` → `ld b, OAM_COUNT * sizeof_OAM_ATTRS`, `ldh [$FF40]` in `basic_usage`);
        kept as numbers in `refactor-p2-hw` so the asm stayed identical (the test CPU then needs the `_SCRN0` value)
  - [ ] Follow-up: operands typed by width for `hw` symbols: today the 8-bit ALU takes only a `Symbol<u8>`, but `Expr`,
        `Operand` and `Mem::addr` take a symbol of either width, so a flag can be used as an address and an address as an
        8-bit immediate (`ld a, rLCDC`); needs a 16-bit address / 8-bit immediate distinction in `gb_asm`
- [x] Move `Emittable` into the asm layer; add a `Block` instruction buffer (stop using `Asm` + `get_main_instrs()` as scratch)
      (branch `refactor-p2-typed-operands-2b`: `gb_asm::Block` with the same builders as `Asm`, written once; `Emittable`
      and `boxed` in `gb_asm`, re-exported by `gb_std::flow`; `gb_std` and `rust_boy` build every snippet in a `Block`)
- [x] Typed operands: remove string-register helpers (`inc_label("de")`, `ld_hli_label("a")`, …); add an
      `Expr` operand for constants/expressions; reject invalid destinations (branch `refactor-p2-typed-operands`:
      `Dst`/`Operand`/`Mem`/`AluOperand`/`IncDec`, `ld 1, 2` and `inc 5` do not compile, `Instr::check` accepts exactly
      the `ld`/`ldh` pairs of the opcode table; `Expr` with `Expr::raw` as the escape hatch; every call site migrated)
- [x] Consistent instruction shapes (`And`/`Cp` vs `Or`/`Xor`/`Sub`, `AdcA` vs `Adc`); derive `Debug`, `PartialEq` on `Instr`
      (branch `refactor-p2-isa`: the 8-bit ALU instructions take one source and print `op a, src`; `AddHl`/`AddSp` for
      the 16-bit additions; the shifts and bit instructions take an `R8`; `Instr::check` rejects what the types cannot)
- [x] Complete the ISA (needed by interrupts, DMA, 16-bit math, audio): `push`/`pop`, `halt`, `stop`,
      `nop`, `di`/`ei`, `reti`, `rst`, `sbc`, `bit`/`set`/`res`, `rl`/`rr`/`rlc`/`rrc`/`sla`/`sra` (+ `rla`…),
      `cpl`, `scf`/`ccf`, `ld [hl-]`, `ld hl, sp+e`, `jp hl`, `add sp, e` (branch `refactor-p2-isa`, plus `call cc`;
      every family, with all the operands of the regular families, checked with rgbasm against the SM83 opcode table in `gb_asm::isa_tests`)
- [x] First-class sections (type, bank, `ALIGN`, `ds n` without fill for RAM); move `Chunk` and the game
      layout out of `gb_asm` into the engine (branch `refactor-p2-sections`: typed `gb_asm::Section` for every memory
      type, with fixed address, bank, `ALIGN`, `UNION`, `FRAGMENT`, checked against RGBDS 1.0.4; `ds n` without fill;
      code or data in a RAM section, or a section name used twice, panics; the relaxation splits on typed sections.
      Branch `refactor-p2-sections-layout`: `Chunk` and its order are the engine's, `rust_boy::{Chunk, Layout}`; an
      `Asm` is one program printed in the order it is written, and checks its sections in `emit`. The 6 example
      ROMs, and their asm, are byte-identical)
- [x] Label allocator owned by the asm layer; automatic `jr` → `jp` when out of range
      (branch `refactor-p2-labels`: the program's `Asm` owns its `LabelAllocator` (`Asm::labels`, `Asm::emit_code`),
      `Emittable::emit` takes it instead of the `If` counter, and every generated label (`If*`, key checks, moves, the OAM
      clear loop, the animation dispatcher) is a local `.{stem}_N` from it, unique by construction; `Asm::to_asm` turns
      each `jr` that does not provably reach its target into a `jp`, iterating until every `jr` left is in range
      (`gb_asm::relax`, sizes from `Instr::size`); the 6 example ROMs are byte-identical, only label names changed)
- [x] Routines as values: `Routine { name, body, deps, clobbers }` → automatic inclusion of dependencies
      and a documented calling convention (which registers each routine clobbers). ([B26](CONTEXT.md#b26) is fixed
      since by scanning the generated code for function names in `build()`; `GetTileByPixel` documents its registers since [B23](CONTEXT.md#b23))
      (branch `refactor-p2-routines`: `gb_std::routine::Routine` with its dependencies, variables and calling convention
      (`Regs` read, returned, clobbered), checked for every `gb_std` routine on the test CPU; every builtin is its `gb_std`
      value (`Delay` moved to `gb_std`), whose dependencies are given in full; `RustBoy::define_routine` / `call_routine`
      register a routine with its dependencies; the scan reads typed operands by type and raw text as before. The 6
      example ROMs, their asm, `.map` and `.sym` are byte-identical)
- [x] One source of truth for builtins: `rust_boy` reuses `gb_std` (done for the routines since [B23](CONTEXT.md#b23);
      `Delay` is `gb_std`'s too since `refactor-p2-routines`); remove the duplicate `gb_std::graphics::sprites::SpriteManager`
      (branch `refactor-p2-routines-sprites`: the `gb_std` `SpriteManager` is gone, `unbricked_std` uses `Sprite` values and
      `gb_std::graphics::sprites::draw_sprites`, which the engine's start-up code uses too, as it uses `gb_std`'s
      `move_coord_var` for `move_x_var` / `move_y_var`; each routine is defined once in the library, `unbricked.rs` excepted.
      The 6 example ROMs, their asm, `.map` and `.sym` are byte-identical)
  - [ ] Follow-up (data, not routines): the engine writes its tile data and its variable sections itself, as `gb_std`'s
        `add_tiles` / `add_tiles_2bpp` and `VariableSection::generate` do (the same text); make the engine call them
  - [x] Decided by the maintainer (2026-10-10): `src/bin/basic_usage.rs`, the minimal raw-`gb_asm` example, keeps its own
        `WaitVBlank` (another routine: it waits for `rLY` = 144, `jr nz`; the README's `gb_asm` example does the same). It
        is a stated exception next to `unbricked.rs` (CLAUDE.md)
- [x] `build(&self) -> Result<String, Error>`; loud errors instead of empty `Vec`s (the sprite manager panics since
      [B20](CONTEXT.md#b20) instead of returning empty `Vec`s) (branch `refactor-p2-engine-api-errors`: `rust_boy::Error`
      with `UnknownFunction`, `NameConflict`, `MemoryFull`, `Section`; the rule, on `Error`: a method panics when the call
      itself is wrong, `build()` returns an `Err` for what only the whole program shows and does not panic itself;
      `call` / `call_args` / `keep_function` names are checked by `build()`, variables are laid out by `build()`; building
      changes nothing. The 6 example ROMs, their asm, `.map` and `.sym` are byte-identical)
- [x] `RustBoyConfig` (sprite size, palettes, LCDC flags, which builtins); the sprite size exists since B4
      as `RustBoy::set_sprite_size` (branch `refactor-p2-engine-api-config`: `RustBoy::with_config(RustBoyConfig)` with
      `sprite_size`, `background_tilemap`, `palettes` (`Palettes`: `rBGP`, `rOBP0`, `rOBP1`), `lcdc` (`Lcdc`: background,
      objects), `builtins` (forced, as `use_function`), `animation_delay`; builder methods, and the setters kept
      (`set_palettes` new); the defaults give byte-identical output, and so do the 6 example ROMs)
- [x] Use `MemoryAllocator` for VRAM / WRAM / OAM / HRAM (done for VRAM tiles, WRAM0 and OAM since
      [B17](CONTEXT.md#b17); HRAM, and real addresses for the variables (rgblink places the sections), are left)
      (branch `refactor-p2-engine-api-memory`: `create_hram_*` variables in `MemoryRegion::Hram`, $FF80-$FFBF (the stack
      keeps the top of HRAM), read and written with `ldh`; every variable section is printed at the address the
      allocator gives it, `WRAM0[$C000]`, `HRAM[$FF80]`, so `get_address` is the linked address once the program's
      variables are created (`build()` adds its own at the end of the last `WRAM0` section; checked against the
      `.sym` of RGBDS). The asm of the 3 `RustBoy` examples changes in that one line; their ROM, `.map` and `.sym` are
      byte-identical)
- [x] `If` that never clobbers user registers (or documents what it uses) (branch `refactor-p2-routines-if`: documented,
      in the clobber model of routines: `If::clobbers()` is `a`, `b` and the flags, `IfConst` `a` and the flags, `IfA` and
      `IfCall` the flags, each checked on the test CPU for every operator, with and without else. Not a `push`/`pop` of
      every user register, which would change the ROMs of `unbricked_std` and `unbricked_rustboy`, the examples that use
      an `If`. The B5 hole is closed: left code that may change `b` (a
      call, raw code, a write to `b`; `Regs::written_by`) is wrapped in `push bc` / `pop bc`, which no example needs, so
      the 6 example ROMs, their asm, `.map` and `.sym` are byte-identical)
- [x] `prelude` module; avoid the `rust_boy::rust_boy` stutter (optional rename: `asm` / `std` / `engine`)
      (branch `refactor-p2-engine-api-prelude`: the layers are `rust_boy::asm`, `rust_boy::stdlib` (not `std`: a module
      named `std` makes `std::` ambiguous in the crate and in a glob import) and `rust_boy::engine`, the inner
      `gb_asm::asm` module is `asm::program`; `rust_boy::prelude` re-exports the common types of every layer and `hw`,
      with a whole program as its doctest. Only paths changed: the 6 example ROMs, asm, `.map` and `.sym` are
      byte-identical to the branch before)
- [x] Ship all breaking API changes together in one release (branch `refactor-p2-engine-api-prelude`: the release
      notes and one migration guide for every breaking change of Phases 1 and 2 are [CHANGELOG.md](CHANGELOG.md), for
      the `refactor` → `main` PR, which ships them together; that PR is not opened yet)

## Phase 3 — Missing features

### Graphics
- [ ] Shadow OAM in WRAM + OAM DMA routine in HRAM (fixes [B12](CONTEXT.md#b12))
- [ ] VRAM write queue flushed during VBlank (tilemap edits, score)
- [ ] Background scrolling (`SCX`/`SCY`), camera helpers
- [ ] Window layer (`WX`/`WY`; a tilemap at `$9C00` can be added since [B19](CONTEXT.md#b19))
- [ ] `GetTileByPixel` for the `$9C00` map: it reads only `$9800`, also when the background shows `$9C00`
      (`RustBoy::set_background_tilemap`, [B19](CONTEXT.md#b19))
- [ ] Tilemaps kept in ROM only, to copy at runtime (several levels or screens): today each tilemap is copied to its
      map at start-up, and since [B19](CONTEXT.md#b19) a second tilemap on the same map panics
- [ ] Palette API (`BGP`, `OBP0`, `OBP1`) + fade in / fade out
- [ ] Typed sprite flags (flip X/Y, priority, palette)
- [ ] Metasprites of any size (generalise 16×16); the choice of 8×8 or 8×16 sprites is done ([B4](CONTEXT.md#b4))
- [ ] Sprite show / hide, OAM slot allocation
- [ ] Text: font loading, print string, print number / BCD score
- [ ] Background tile animation
- [ ] PNG → 2bpp conversion in Rust (tile count derived from file size)

### Animation
- [x] `PingPong` and `Once` ([B10](CONTEXT.md#b10)) (branch `refactor-p1-animations`)
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
- [ ] WRAM arrays (HRAM variables: done in Phase 2, `refactor-p2-engine-api-memory`, `VariableManager::create_hram_*`)
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
- [ ] The committed example asm files (`examples/fosdem/main.asm`, `examples/coin-anim/main.asm`,
      `examples/unbricked/generated/`, `generated-std/`, `unbricked-rustboy/`) are old snapshots from `main`: they still
      show `ClearOam`, `AnimEnd` and other code the library no longer generates. Regenerate them (and keep them in sync,
      e.g. as the snapshot tests above), delete them, or mark them as old snapshots — the maintainer's choice
- [ ] Headless-emulator tests (run the ROM, assert on memory/registers)
- [ ] Asm comments pointing back to the Rust source (`#[track_caller]`)
- [ ] ROM-size and cycle-budget report (e.g. "main loop exceeds VBlank")
- [ ] Peephole optimisations (`ld a, 0` → `xor a`, `cp 0` → `and a`, …)
- [ ] Validate user-supplied symbol names (sprite, composite and animation names are checked since
      [B25](CONTEXT.md#b25), but not against RGBDS keywords, nor against the other global labels: sprites
      `"Coin"` and `"CoinEnd"` both define `CoinEnd`, a sprite `"Main"` clashes with `Main`; tiles, variables,
      functions and constants are not checked at all, e.g. `add_sprite_tiles(player, "Player", ..)` gives
      "`Player` already defined" in rgbasm)

## Phase 4 — Documentation

- [ ] Fix the inaccuracies in the `documentations` branch ([B30](CONTEXT.md#b30)), then fast-forward merge it
- [ ] Rustdoc on every public item; turn ```` ```ignore ```` examples into compiled doctests
- [ ] Architecture / levels document (from [CONTEXT.md §3](CONTEXT.md#3-are-the-levels-correct-assessment))
- [ ] Calling-convention document (registers each routine uses/clobbers)
- [ ] Tutorial: "your first game" step by step with `RustBoy`
- [ ] Hardware notes: VBlank & OAM timing, 8×16 rules, coordinate offsets (+8 / +16)
- [x] `CHANGELOG.md` (the release notes of the refactoring, branch `refactor-p2-engine-api-prelude`; keep it up to date)
- [ ] Keep `CLAUDE.md` and `CONTEXT.md` up to date as phases land

## Future

- [ ] **Move the examples to a separate repository** (e.g. `rust-boy-examples`): the example binaries,
      their assets (`.png`, `.aseprite`, `.2bpp`, `hardware.inc`) and golden `.asm` files; depend on
      `rust-boy` via git. Keep 1–2 minimal Cargo `examples/` here for CI.
- [ ] Publish on crates.io (move `src/bin` → `examples/` first)

---

## Branches

Verified with `git rev-list --count` against `origin/main`. The 7 *Delete* branches were deleted on
2026-10-07. A deleted branch can be restored with `git push origin <last commit>:refs/heads/<branch>`.

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
