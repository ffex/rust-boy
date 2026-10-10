# rust-boy — Project Context

Background for humans and AI assistants working on rust-boy: what the project is, how it is
built, how the code is layered, what is broken and why. The actionable checklist lives in
[`Task.md`](Task.md); working rules live in [`CLAUDE.md`](CLAUDE.md).

> Snapshot taken on branch `refactor` at commit `4601a5c` (October 2026). Line numbers refer to
> that commit. Since `refactor-p2-engine-api-prelude` the layers are `asm`, `stdlib` and `engine`, in `src/asm/`,
> `src/stdlib/` and `src/engine/` (they were `gb_asm`, `gb_std` and `rust_boy`; `gb_asm/asm.rs` is `asm/program.rs`):
> file paths below use the new directories, and the text keeps the module names of the time it describes. Every bug below was reproduced or confirmed by reading the code and then
> independently re-checked by a skeptical reviewer pass. Claims that did not survive are listed
> in [Checked and refuted](#checked-and-refuted).

---

## 1. What rust-boy is

A Rust library (a DSL) that **generates Game Boy assembly** in [RGBDS](https://rgbds.gbdev.io/)
syntax. A Rust program builds the game with the library and prints a `.asm` file; RGBDS turns it
into a ROM:

```
your_game.rs ──cargo run──▶ main.asm ──rgbasm──▶ main.o ──rgblink──▶ main.gb ──rgbfix──▶ playable ROM
```

- Zero Rust dependencies, edition 2024.
- The project is built with RGBDS 1.0 (the `.o` files once committed were object format `RGB9`). Generated code uses `0x05`-style
  constants, which needs **RGBDS ≥ 0.9**. `rgbasm -L` (the old README used it) was removed in RGBDS 0.8.
- `hardware.inc` (v4.x) lives once in `include/hardware.inc` (it used to be copied 6 times under
  `examples/`); pass it with `rgbasm -I include`.

### Commands

```bash
cargo build                              # library + all bins
cargo test                               # unit tests, doctests, snapshot tests (tests/snapshots.rs)
UPDATE_SNAPSHOTS=1 cargo test --test snapshots   # rewrite examples/<bin>/main.asm after an intended change
RGBDS_LINK_CHECK=1 cargo test --test emulator    # build ROMs with RGBDS, run them on the test emulator
cargo run --bin fosdem > main.asm        # bins: basic_usage, unbricked, unbricked_std,
                                         #       unbricked_rustboy, fosdem, coin-anim
rgbasm -I include -o main.o main.asm     # add -I <example dir> for its .2bpp assets
rgblink -o main.gb main.o
rgbfix -v -p 0xFF main.gb
```

### Health (snapshot at `4601a5c`, updated as fixes land)

| Check | Status |
|---|---|
| `cargo build --lib` | ✅ builds with no warnings; `cargo clippy --all-targets -- -D warnings` passes |
| `cargo test` | ✅ 312 library unit tests (and one in the `basic_usage` bin), the snapshot tests of the 6 examples (`tests/snapshots.rs`, which also tests its diff) and the doctests (README examples, the `prelude` module, `rust_boy::Error`, `RustBoy::build`, `RustBoyConfig`, `RustBoy::with_config`, `VariableManager::create_hram_u8`, `Asm::try_emit`, the `hw` module, the `gb_std::routine` module, `Regs::written_by`, `draw_sprites`, `RustBoy::define_routine`, `RustBoy::call_routine`, `Block`, the `gb_asm::section` module, the builders' `section`, `Asm::emit`, `Asm::blank_line`, `Layout`, the builders' `ld` and the compile-fail proofs that `ld 1, 2`, `inc 5`, `add a, hl` and `cp a, [wCount]` do not compile, `Expr`, `LabelAllocator`, `Asm::labels`, `Asm::emit_code`, `Asm::program`, `RustBoy::labels`, `RustBoy::keep_function`, `RustBoy::external_symbol`, `RustBoy::raw`, `RustBoy::add_sprite_tiles`, `Var`) pass (was: 8 type errors, fixed — [B1](#b1)) |
| bin `coin-anim` | ✅ compiles (was broken, fixed — [B2](#b2)); the 8×8 frames render right (were drawn as 8×16 pairs, fixed — [B4](#b4)) |
| bin `unbricked_rustboy` | ✅ assembles and links with RGBDS 1.0.4 (was: "`wCurKeys` already defined", fixed — [B3](#b3)); Paddle and Ball each draw their own tile ([B4](#b4)) |
| bin `unbricked_std` | ✅ assembles and links with RGBDS 1.0.4; paddle bounce fixed ([B5](#b5)) |
| bin `fosdem` | ✅ assembles; the 16×16 player moves as one block and stops at its limits (it collapsed at screen edges, fixed — [B6](#b6)) |
| Output determinism | ✅ every bin prints the same `.asm` on every run (was random, fixed — [B13](#b13)) |
| Generated labels | ✅ a key check or move can be used any number of times and inside an `If`, and two sprites can share an animation name (fixed — [B7](#b7), [B25](#b25)); since Phase 2 (`refactor-p2-labels`) every label generated code makes up (`If`, snippets, the OAM clear loop, the animation dispatcher) comes from one `LabelAllocator` per program, owned by its `Asm`, so it is unique in the whole program by construction; unit tests check the labels with the RGBDS scope rules (`gb_asm::label_check`) |
| Jumps | ✅ since Phase 2 (`refactor-p2-labels`) `Asm::to_asm` turns each `jr` that does not reach its target (out of -128..=127, another section, a symbol the program does not define, or behind a line of unknown size) into a `jp`, iterating until every `jr` left is in range, and writes `@`-relative targets again so they keep their instruction (`gb_asm::relax`). The `relax` test programs are assembled with RGBDS (`-Werror`) and every jump's opcode and landing address is checked in the ROM; the `RustBoy` test programs are checked with `jr_range_errors` (no `jr` out of range) and `assert_links` (linked with RGBDS under `RGBDS_LINK_CHECK`); the examples are assembled by CI |
| Sections | ✅ since Phase 2 (`refactor-p2-sections`) `SECTION` is a typed `gb_asm::Section`: a name, a memory type (`ROM0`, `ROMX`, `VRAM`, `SRAM`, `WRAM0`, `WRAMX`, `OAM`, `HRAM`), and optionally a fixed address, a bank, `ALIGN[n, offset]`, `UNION` or `FRAGMENT`; what RGBDS rejects panics when it is built, and code or data in a RAM section (which only reserves space, with `ds n` and no fill), or a section name used twice, panics when it is emitted (`Asm::emit`; since `refactor-p2-sections-layout`, before when the program was printed). Tests (`gb_asm::section`): every form prints the expected line and, with `RGBDS_LINK_CHECK`, RGBDS places it where it says (checked in the `.sym` file); every rejected form is rejected by RGBDS too |
| Start-up code | ✅ `gb.init()` code runs after the variables (animation variables included) and palettes are set, so what it sets survives (was overwritten, fixed — [B11](#b11)); the OAM is always cleared and `rOBP1` is set (fixed — [B28](#b28)); unit tests run the start-up code on `gb_asm::test_cpu` |
| Animations | ✅ any number of animated sprites and animations assemble (the dispatcher's `jr` went out of range from 3 sprites × 4 animations, fixed — [B9](#b9)); `Loop`, `PingPong` and `Once` all work (`PingPong`/`Once` played as `Loop`, fixed — [B10](#b10)); unit tests run the generated code frame by frame (`gb_asm::test_cpu`) |
| Functions and routines | ✅ since Phase 2 (`refactor-p2-routines`) every function is a `gb_std::routine::Routine` value: a name, a body, the routines it depends on, the WRAM variables it needs, and its calling convention (`Regs` it reads, returns and clobbers; every other register is preserved), checked for every `gb_std` routine on the test CPU; a builtin is its `gb_std` routine, `RustBoy::define_routine` / `call_routine` register a routine with its dependencies (see [Key concepts](#key-concepts)). `build()` emits each builtin and user function the generated code refers to (`call`, `jp`, `Call`, `IfCall`, function bodies, raw code; a builtin reached through `Call`/`IfCall`/a function body was missing, fixed — [B26](#b26)), once, with the variables it needs, and only those (unused user functions were emitted, fixed — [B24](#b24)); `RustBoy::keep_function` forces one, `RustBoy::external_symbol` declares one defined outside (an `INCLUDE`d file, which `build()` does not read); one `GetTileByPixel` in the library, with one contract (fixed — [B23](#b23)); `Memcopy` copies nothing for a length of 0, and empty raw tile data gets no copy (fixed — [B27](#b27)); every chunk of `raw()` code is kept (fixed — [B15](#b15)) |
| API safety | ✅ unknown sprite / composite ids, animation names and indices panic with a clear message (they gave no code, fixed — [B20](#b20)); VRAM tiles, WRAM0 variables and OAM entries are allocated through `MemoryAllocator` and panic when full, sprite positions and animation frames are checked (fixed — [B17](#b17)); a sprite's tile index comes from its VRAM address (fixed — [B18](#b18)); `Var::set`/`get` handle 16-bit variables (fixed — [B16](#b16)); `get_pivot` wraps around the map (fixed — [B22](#b22)); a tilemap can go to `$9C00` (fixed — [B19](#b19)) |
| Instruction set | ✅ every SM83 instruction (`push`/`pop`, `halt`, `stop`, `di`/`ei`, `reti`, `rst`, `sbc`, `bit`/`set`/`res`, the rotates and shifts, `cpl`, `scf`/`ccf`, `ld [hld]`, `ld hl, sp + e`, `jp hl`, `add sp, e`, `call cc` were missing); one shape per family, the 8-bit ALU printed `op a, src` (`cp` and `adc` were printed without `a`); `Instr` derives `Debug` and `PartialEq`; `gb_asm::isa_tests` checks every instruction family, with all the operands of the regular families (541 instructions): text and size and, with `RGBDS_LINK_CHECK`, the bytes from rgbasm against the SM83 opcode table (Phase 2, `refactor-p2-isa`). Typed operands since `refactor-p2-typed-operands`: no register or expression is a string any more (`Dst`, `Operand`, `Mem`, `AluOperand`, `IncDec`, and `Expr` for values), a value cannot be a destination, and `Instr::check` accepts exactly the `ld`/`ldh` operand pairs of the opcode table (`isa_tests`: all 91 load opcodes, `Expr` operands assembled by RGBDS, 554 instructions) |
| Hardware facts | ✅ since Phase 2 (`refactor-p2-hw`) every register, flag, address, OAM offset and screen size that `gb_std`, `rust_boy` and the examples write comes from `hw` (see [Key concepts](#key-concepts)): 110 `hw::Symbol`s, each a `hardware.inc` name and its value, plus the facts `hardware.inc` has no name for. Tests: every symbol is a name `include/hardware.inc` defines, and with `RGBDS_LINK_CHECK` a file that includes it `ASSERT`s every value (a wrong value fails); `test_no_hardware_strings_outside_hw` fails on a `hardware.inc` name or an address from `$8000` on written as a string (or a hex integer) in `gb_std`, `rust_boy` or the examples, outside their tests and the hand-written `unbricked.rs`. The 6 example ROMs, their asm, `.map` and `.sym`, are byte-identical |
| Build API | ✅ since Phase 2 (`refactor-p2-engine-api-errors`) `RustBoy::build(&self) -> Result<String, Error>`: building changes nothing (the variables it adds go to a copy), so it can be called any number of times; what only the whole program shows is an `Err` (`rust_boy::Error`: `UnknownFunction`, `NameConflict`, `MemoryFull`, `Section`), and what is wrong at a call panics there, so `build()` itself does not panic on what a program contains (see [§3](#3-are-the-levels-correct-assessment), item 6) |
| Memory | ✅ VRAM tiles and OAM entries are allocated through `MemoryAllocator` when they are added ([B17](#b17)); the WRAM0 and, since Phase 2 (`refactor-p2-engine-api-memory`), HRAM variables are laid out by `build()` (full is `Error::MemoryFull`), and each variable section is printed at the address the allocator gives it (`WRAM0[$C000]`, `HRAM[$FF80]`), so `get_address` is the linked address once the program's variables are created (the variables `build()` adds go at the end of the last `WRAM0` section and move none; checked against the RGBDS `.sym`); HRAM variables are read and written with `ldh` |
| Snapshot tests | ✅ since Phase 3 (`refactor-p3-tooling`) the asm each example prints is committed as `examples/<bin>/main.asm` (the old files were stale snapshots from `main`, regenerated), and `tests/snapshots.rs` fails with a unified diff when the output differs; `UPDATE_SNAPSHOTS=1 cargo test --test snapshots` rewrites them |
| Headless-emulator tests | ✅ since Phase 3 (`refactor-p3-tooling-emulator`) `tests/emulator.rs` builds ROMs with RGBDS and runs them on a DMG emulator written for the tests (`tests/support/gameboy.rs`, no dependency: the SM83 M-cycle by M-cycle, interrupts, timer, PPU timing without pixels, OAM DMA, joypad, MBC1/MBC5 ROM banking; OAM blocked in modes 2-3 and VRAM in mode 3, as on the hardware), checked against Blargg's `cpu_instrs` (11), `instr_timing` and `mem_timing` (3) ROMs; a test runs frames with scripted input and reads WRAM, HRAM, VRAM, OAM, the registers and every OAM write with its PPU mode. One test per example (the Unbricked ball moves and bounces, bricks break, the paddle stops at its limit; the FOSDEM player walks as one block with its animation; the coin animates on A, stops on B; `basic_usage` loops), each also checking that every OAM write lands in VBlank, and [B12](#b12) as it is today. They run with `RGBDS_LINK_CHECK` (in CI); cross-checked once against PyBoy 2.6.0 (identical OAM traces over 390 frames with input for the 3 Unbricked examples) |
| CI | ✅ GitHub Actions: fmt, clippy `-D warnings`, tests (stable and Rust 1.85), every example assembled with RGBDS 1.0.4, the whole-program unit tests linked with it (`RGBDS_LINK_CHECK`, since [B26](#b26)), and the emulator tests with the Blargg ROMs (since `refactor-p3-tooling-emulator`; RGBDS, the ROMs and the Cargo builds are cached) |
| Committed build artifacts | ✅ none (the 12 `*.gb` / `*.o` files were untracked; `.gitignore` covers them) |

---

## 2. Layer map

| Layer | Path | LOC | Role |
|---|---|---|---|
| **L1 `asm`** (was `gb_asm`) | `src/asm/` (`instr.rs`, `expr.rs`, `builders.rs`, `program.rs`, `block.rs`, `codegen.rs`, `labels.rs`, `relax.rs`, `section.rs`) | ~3200 | The whole SM83 instruction set as `Instr` (one shape per family, `Instr::check`; since Phase 2 `refactor-p2-isa`; `Instr::size` since `refactor-p2-labels`) with typed operands (`R8`, `R16`, `R16Stack`, `Mem`, `Dst`, `Operand`, `AluOperand`, `IncDec`) and `Expr` values (since `refactor-p2-typed-operands`), typed `Section`s and the checks of what goes in them (`section.rs`, since `refactor-p2-sections`), fluent `Asm` builder for whole programs (one ordered list of instructions since `refactor-p2-sections-layout`; it had `Chunk` buckets) and `Block` for pieces of code, with the same builder methods (`instruction_builders!`), `Emittable` (since `refactor-p2-typed-operands-2b`), `Display` → RGBDS text, `LabelAllocator` (since [B7](#b7); owned by the program's `Asm` since `refactor-p2-labels`), `jr` → `jp` relaxation of the whole program (`relax.rs`, since `refactor-p2-labels`) |
| **L2 `stdlib`** (was `gb_std`) | `src/stdlib/` (`flow/`, `graphics/`, `inputs.rs`, `variables.rs`, `utility.rs`, `hw_symbols.rs`, `routine.rs`) | ~3300 | Stateless routines as `Routine` values (since `refactor-p2-routines`; they returned `Vec<Instr>`): Memcopy, WaitVBlank, WaitNotVBlank, UpdateKeys, GetTileByPixel, Delay, `is_specific_tile`, each with its dependencies and calling convention (`Regs`), control flow (`If`, `IfConst`, `IfA`, `IfCall`, `Call`; `Emittable` re-exported from `gb_asm`), `Sprite` and the sprite snippets the engine uses too (`draw_sprites`, moves, `get_pivot`; its own `SpriteManager` is gone since `refactor-p2-routines-sprites`), `TileRef`, and (since `refactor-p2-hw`) the conversions of an `hw::Symbol` into an `Expr` / operand (`hw_symbols.rs`) |
| **L3 `engine`** (was `rust_boy`) | `src/engine/` (`rustboy.rs`, `layout.rs`, `sprites.rs`, `tiles.rs`, `variables.rs`, `functions.rs`, `animations.rs`, `inputs.rs`, `memory.rs`, `error.rs`) | ~2900 | `RustBoy` engine: tile/VRAM, variable/WRAM, sprite/OAM managers, builtin-function registry, input bindings, animations, the program layout (`Chunk`, `Layout`, since `refactor-p2-sections-layout`), `build()` and its `Error` (since `refactor-p2-engine-api-errors`) |
| **`prelude`** | `src/prelude.rs` | ~40 | Re-exports of the common types of every layer and `hw` (`use rust_boy::prelude::*`), since `refactor-p2-engine-api-prelude` |
| **`hw`** (data) | `src/hw.rs` | ~390 | Hardware facts as pure data (since [B22](#b22); complete since Phase 2 `refactor-p2-hw`): `hw::Symbol`s, each a `hardware.inc` name and its value, for the I/O registers, their flags, the memory map, the OAM layout and the screen sizes (`hw::SYMBOLS` lists them), the facts with no `hardware.inc` name as plain numbers, and `oam_offset`; depends on nothing, used by `gb_std` (which turns a `Symbol` into an `Expr`, `src/stdlib/hw_symbols.rs`), `rust_boy` and the examples, not by `gb_asm` |
| Examples | `src/bin/` | ~2410 | 6 binaries (raw `gb_asm`, `gb_std`, and `rust_boy` versions of the Unbricked tutorial, plus FOSDEM demo and coin animation) |

### Key concepts

- **`Emittable`** (`src/asm/block.rs` since `refactor-p2-typed-operands-2b`, re-exported by `gb_std::flow`;
  it was in `src/stdlib/flow/emittable.rs`): `fn emit(&mut self, labels: &LabelAllocator) -> Vec<Instr>`
  (since `refactor-p2-labels`; it took the `If` counter, `counter: &mut usize`). Implemented by `Block`,
  `Vec<Instr>`, `Vec<Vec<Instr>>`, `Vec<Box<dyn Emittable>>`, `Call`, `Op`, `If*`. Everything that goes into the
  main loop / init / user functions is an `Emittable`; `Asm::emit_code(code)` emits one with the program's labels.
- **Generated labels** (since `refactor-p2-labels`; [B7](#b7) started it for the snippets): every label that
  generated code makes up is *local* and comes from one `gb_asm::LabelAllocator` per program. The program's
  `Asm` owns it (`Asm::labels`; `RustBoy::labels` is the one of its `raw()` `Asm`, shared with its
  `SpriteManager`); clones share one counter. Each allocation takes the next number `N` and names its labels
  `.{stem}_N` (`LabelAllocator::local`, or `locals` for several stems with one number), so they are unique in
  the whole program by construction: `If*` → `.end_if_N`, `.else_N`, `.then_N`; `check_key` →
  `.check_left_N`, `.check_left_end_N`; a limited move → `.sprite0_left_limit_store_N`, `…_end_N`;
  `clear_objects_screen` → `.clear_oam_N` (was the global `ClearOam`); the animation dispatcher →
  `.anim_end_N` (was the global `AnimEnd`), `.anim_{sprite}_end_N`, `.skip_{sprite}_{animation}_N`. The code
  `build()` generates (start-up, dispatcher) uses a fork of the allocator (`LabelAllocator::fork`: the same
  sequence from where it is, not shared), so two builds print the same labels ([B14](#b14)). Local labels never
  change the RGBDS scope, so they can go inside an `If` body. Global labels are left to routines and functions,
  emitted once (`Memcopy`, `Anim_{sprite}_{animation}`, user functions; their own fixed local labels, such as
  `.copy`, live in their scope), and to `EntryPoint` and `Main`. `gb_std` callers pass the allocator to
  `check_key`, `Sprite::move_*_limit` and `clear_objects_screen` (`asm.labels()`, or `gb.labels()`).
- **Jump relaxation** (since `refactor-p2-labels`, `src/asm/relax.rs`): `Asm::to_asm` / `Asm::program`
  print the whole program, and each `jr` / `jr cc` that cannot be shown to reach its target becomes
  `jp` / `jp cc`: the target must be a label of the program defined once (RGBDS scope rules), in the same
  section, with only instructions of known size in between (`Instr::size`: every instruction, and `db` / `dw` /
  `ds` / `INCBIN` written with plain numbers), at -128..=127 from the end of the `jr`. Otherwise (another
  section, an external symbol, an address, a raw line with code, an `INCLUDE`, a string, a symbol in `db` /
  `dw`, which can be an `EQUS`) it is a `jp`. It starts with every `jr` short and grows the ones out of range
  until none is (a grown jump can push another out of range), which gives the fewest `jp`. A `jp` is never
  shortened, so the `jp`s of [B9](#b9) stay. A target written from `@` (`jr nz, @+4`, on a `jr`, `jp` or `call`)
  is the instruction that many bytes from the jump: it is measured like a label and its offset is written again
  for the relaxed code (a `jp` is one byte longer, so `jp nz, @+4` would land one byte early). If that
  instruction cannot be found, or a jump target is another expression (`Label + 2`), no jump of the program is
  changed (printed as written; rgbasm reports a `jr` out of range), and the same holds when `@` appears anywhere
  else in the code (a raw line, `db` / `dw`, an operand `ld hl, @ + 5`, a `DEF`), except the padding `ds N - @`.
  A `dw` of labels the program defines once has a known size (2 bytes each); another symbol in data could be an
  `EQUS`, so its size is unknown.
- **Sections** (since `refactor-p2-sections`, `src/asm/section.rs`): `Instr::Section(Section)`. A `Section` is
  built with `Section::new(name, MemoryType)` (or `Section::rom0(name)`, `wram0`, …) and `at(address)`, `bank(n)`,
  `align(bits)` / `align_offset(bits, offset)`, `union()`, `fragment()`; each panics on what RGBDS 1.0 rejects
  (`Section::check`): a bank on `ROM0`, `WRAM0`, `OAM` or `HRAM`, a bank out of the range rgbasm accepts (`ROMX`
  1-65535, `VRAM` 0-1, `SRAM` 0-255, `WRAMX` 1-7), an address outside the memory type (RGBDS's default ranges, no
  `rgblink -t`/`-w`), an alignment above 16 bits or that the fixed address, or no address of the type, has, `UNION`
  in ROM, a name with `"`, `\`, `{` or a control character other than a tab. It prints `SECTION [UNION|FRAGMENT] "name",
  TYPE[$addr], BANK[n], ALIGN[n, offset]`. `SectionTracker` follows a program as it is emitted (`Asm::emit`, since
  `refactor-p2-sections-layout`; before, in its printed order, in `Asm::to_asm`) and panics on code or data in a RAM section (an instruction, `ds n, fill`, `db`/`dw` with values,
  `INCBIN`; labels, `ds n`, `db`/`dw` without values, comments and `DEF`s are fine) and on a section name used twice
  (only `UNION`s, or `FRAGMENT`s, of one memory type and bank share one; this check is partial, it compares a new
  piece with the first one by kind, memory type and bank, and leaves the rest, such as two `UNION`s at different
  fixed addresses, to rgbasm). Raw lines are never rejected; a raw line with other code than labels and
  `db`/`dw`/`ds` (it may open a section), or an `INCLUDE`, makes the section unknown until the next typed
  `SECTION`, and a raw line with `IF`, `ELIF`, `ELSE`, `ENDC`, `MACRO`, `ENDM`, `REPT`, `FOR` or `ENDR` stops every
  check for the rest of the program (RGBDS may skip or repeat what follows: the same section in both branches of an
  `IF`, code in an `IF 0`). An instruction whose text has a line break is checked as itself, then the lines after
  the break are read as a raw line. `ds n` reserves (`Asm::ds`), `ds n, fill` fills (`Asm::ds_fill`).
- **Chunks and `Layout`** (the engine's since `refactor-p2-sections-layout`, `src/engine/layout.rs`; they were
  `gb_asm::Chunk` and the chunks of `Asm`, printed in `CHUNK_ORDER`): `Chunk::{Header, Constants, Init, MainLoop,
  Main, Functions, Tiles, Tilemap, Data}`, in that order (`Chunk::ORDER`). A `Layout` holds code by chunk, written
  with the builder methods of `Asm` to its current chunk (`Layout::chunk`; `Chunk::Main` at first), and owns the
  program's label allocator; `Layout::program()` puts the chunks that have code together in an `Asm`, in
  `Chunk::ORDER`, each followed by a blank line (`Asm::blank_line`), and that `Asm` checks the sections. `RustBoy`
  keeps its `raw()` code in a `Layout` (the closure gets `&mut Layout`), `build_layout` returns the `Layout` it fills,
  and `build()` prints `build_layout()?.try_program()?.to_asm()` (since `refactor-p2-engine-api-errors`; a section error
  is `Error::Section`). The asm layer knows instructions, sections and programs
  only: an `Asm` is one list of instructions, printed in the order they were emitted.
- **`hw`** (since `refactor-p2-hw`, `src/hw.rs`; tests in `src/hw/tests.rs`): pure data, depending on nothing. A
  `hw::Symbol<T>` is a `hardware.inc` name and its value (`T` = `u16` for an address, `u8` for a flag or a count):
  `hw::LCDC` is `rLCDC` = `$FF40`. The Rust name is the `hardware.inc` name without its `r` / `_` prefix (`OAMRAM` is
  `_OAMRAM`, flags keep their whole name), except `OAM_ENTRY_SIZE` (`sizeof_OAM_ATTRS`). It defines the I/O registers
  (`rP1`, `rIF`, `rLCDC`, `rSTAT`, `rSCY`/`rSCX`, `rLY`, `rLYC`, `rDMA`, `rBGP`, `rOBP0`/`rOBP1`, `rWY`/`rWX`, `rIE`; not
  the audio ones, which nothing uses yet), their flags (`P1F_*`, `LCDCF_*`, `STATF_*`, `IEF_*`, `PADF_*`/`PADB_*`), the
  memory map (`_VRAM`, `_VRAM8000`/`8800`/`9000`, `_SCRN0`/`_SCRN1`, `_SRAM`, `_RAM`, `_RAMBANK`, `_OAMRAM`, `_IO`,
  `_HRAM`), the OAM layout (`OAM_COUNT`, `sizeof_OAM_ATTRS`, `OAMA_*`, `OAMF_*`/`OAMB_*`) and the screen sizes
  (`SCRN_X`, `SCRN_Y`, `SCRN_VX_B`, …), all listed in `hw::SYMBOLS`. What `hardware.inc` has no name for is a plain
  number: the tile regions as `RustBoy` uses them (`VRAM_OBJ_TILES`, `VRAM_BG_TILES`, …), region ends (`WRAM0_END`,
  `OAM_END`, `HRAM_END`, `VRAM_END`), `OAM_SIZE`, `OAM_X_OFFSET`/`OAM_Y_OFFSET`, `TILE_SIZE`/`TILE_WIDTH`,
  `ROM_HEADER`/`ROM_HEADER_END`. `hw::oam_offset(index, hw::OAMA_X)` is the offset of a byte of an OAM entry, and panics
  past OAM or on any symbol but the four `OAMA_*` (`hw::OAM_ENTRY_BYTES`; `OAMB_BANK1`, whose value is 3, panics too).
  `gb_std` turns a `Symbol` into an `Expr`, `Operand` or `AluOperand` by its name (`src/stdlib/hw_symbols.rs`: the
  first layer that sees both, so `hw` stays data and `gb_asm` does not know it); the 8-bit ALU (`AluOperand`) takes
  only a `Symbol<u8>`, so `cp(hw::LCDC)` does not compile, while `Expr` and `Operand` take both widths (they hold 16-bit
  immediates and addresses as well as flag expressions; an operand typed by width is a Task.md follow-up). A symbol
  goes straight to the builders: `ld_addr_def_a(hw::LCDC)` writes `ld [rLCDC], a`,
  `Expr::from(hw::LCDCF_ON) | hw::LCDCF_BGON` writes `LCDCF_ON | LCDCF_BGON`. Code that needs the number uses `.value`
  (`cp_imm(hw::SCRN_Y.value)`, `Expr::hex(hw::SCRN0.value)`, the VRAM addresses the allocators compute). `Symbol` has no
  `Display`, so a message must say `.name` or `.value`. The generated code writes a symbol where it wrote its name
  before, and a number where it wrote a number (`ld bc, $9800` in `GetTileByPixel`, `cp a, 144`, `ld a, 0` for the LCD
  off), so the output did not change.
- **`Block`** (since `refactor-p2-typed-operands-2b`, `src/asm/block.rs`): every `gb_std`/`rust_boy` routine and
  snippet is built in a `Block` (a checked list of instructions with the same builder methods as `Asm`, expanded from
  one `instruction_builders!` in `src/asm/builders.rs`) and returned with `into_instrs()`. They used to create a
  fresh `Asm`, emit into its default `Chunk::Main` and return `asm.get_main_instrs()` (the scratch-`Asm` idiom). An
  `Asm` is now only a whole program: the one `Layout::program` makes for `RustBoy`, and the `gb_asm` example programs.
- **Routines** (since `refactor-p2-routines`, `src/stdlib/routine.rs`): a `Routine` is a routine as a value, built
  with `Routine::new(name, body)` (it panics unless `name` is an identifier and `body` defines the label `name:`) and
  `with_dep`, `with_variable`, `with_reads`, `with_returns`, `with_clobbers`. Its **dependencies** are the routines it
  calls, jumps to or reads, given as values, so a routine brings what it needs: `Routine::with_deps` lists it and every
  dependency, each once, dependencies first (a name is one routine: two different routines with one name panic), and
  `code_with_deps` is their code, the routine first. Its **variables** are the WRAM bytes it needs (`UpdateKeys`:
  `wCurKeys`, `wNewKeys`). Its **calling convention** is three `Regs` (a set of `a`, `b`, `c`, `d`, `e`, `h`, `l` and
  the flags `f`, printed `a, bc, f`): the registers it **reads**, **returns** and **clobbers**; every other register is
  **preserved**. A routine whose convention is not given may change every register (`Regs::ALL`). Every `gb_std`
  routine is a `Routine` (`memcopy()`, `wait_vblank()`, `wait_not_vblank()`, `update_keys()`, `get_tile_by_pixel()`,
  `gb_std::utility::delay()`, `is_specific_tile(..)`), and a `Routine` is its body too (`IntoIterator`, `Into<Vec<Instr>>`),
  so `asm.emit_all(memcopy())` writes it as before. Their conventions:

  | Routine | Reads | Returns | Clobbers | Variables |
  |---|---|---|---|---|
  | `Memcopy` | `bc` (length), `de` (source), `hl` (destination) | `bc` = 0, `de`, `hl` after the bytes | `a`, `f` | |
  | `WaitVBlank`, `WaitNotVBlank` | | | `a`, `f` | |
  | `UpdateKeys` | | | `a`, `b`, `f` | `wCurKeys`, `wNewKeys` |
  | `GetTileByPixel` | `b` (X), `c` (Y) | `a` (tile), `hl` (its address) | `bc`, `f` | |
  | `Delay` | `bc` (count) | | `a`, `bc` (`$FFFF`), `f` | |
  | `is_specific_tile` | `a` (tile) | `f` (Z: one of the tiles) | none | |

  The tests (`src/stdlib/routine/tests.rs`) run each one on `gb_asm::test_cpu` from two CPUs whose registers all hold
  known, different values: every register it changes must be listed (the others are preserved), and every register
  it lists must change in some case (the list is exact). The test CPU models the Z and C flags, not N and H.
  **In the engine** a builtin is its `gb_std` routine (`BuiltinFunction::routine`), and a user function is a routine
  too: `define_function(name, body)` / `define_function_from` build one from its body, and **`RustBoy::define_routine`**
  registers a `Routine` with its dependencies (each before the routines that need it, so it is emitted first), and
  **`RustBoy::call_routine(&routine)`** returns `call Name` and registers the routine the same way. A dependency is
  shared, never replaced: a `gb_std` routine of a builtin is that builtin (or the user function the program replaced it
  with, by registering the replacement itself), and a dependency equal to a function the program has (the whole
  `Routine`: code, dependencies, variables, convention) is that function. Another routine under a taken name panics,
  also under a builtin's name (it would replace the builtin for the whole program, also where the real one is needed;
  `test_a_dependency_cannot_replace_a_builtin`, `test_a_dependency_is_shared_only_when_it_is_the_same_routine`, from the
  independent review). What a function needs is its dependencies and, for a user function, what its body refers to. A builtin's
  dependencies are given in full, so its body is not read (`test_builtin_dependencies_are_complete` checks every
  symbol of each builtin's body against its dependencies, its variables, its own labels and the `hardware.inc` names).
  The symbols of code are read by `gb_asm::labels::symbols`: typed operands by their type (a jump or call target, the
  symbols of an `Expr`), what is only text (a raw line, `db`, `dw`, `ds`, a `DEF` value, `Expr::raw`) as RGBDS reads
  it (the scan of [B26](#b26), with all its rules). The variables of every emitted routine are created, as they were
  for the builtins.
  The same model describes control flow (since `refactor-p2-routines-if`): `If`, `IfConst`, `IfA` and `IfCall` have a
  `clobbers()` that returns the `Regs` they use (see [B5](#b5)), and `Regs::written_by(code)` reads which registers code
  may write (`None` when it cannot tell: a `call`, `rst`, `jp hl`, raw code, `sp`).

### What `RustBoy::build()` emits (`src/engine/rustboy.rs:637-857`, `build` prints the program of the `Layout` `build_layout` returns)

1. **Header**: `INCLUDE "hardware.inc"`, `SECTION "Header", ROM0[$100]`, `jp EntryPoint`, `ds $150 - @, 0`.
   Everything after this stays in that one ROM0 section (no further `SECTION` for code/data).
2. **Constants**: `DEF name EQU value` for each `define_const*`.
3. **Init**: `EntryPoint:` → `call WaitVBlank` → LCD off → `Memcopy` every non-empty tile/tilemap blob to VRAM ([B27](#b27)) →
   clear the whole OAM (always, with `gb_std`'s `initialize_objects_screen` + `clear_objects_screen`) and write
   the initial sprites → `rBGP`, `rOBP0`, `rOBP1` = `%11100100` → create animation variables → **variable
   initialisation** → **user `init()` code** → LCD on (`LCDCF_ON|BGON|OBJON` + `OBJ8`, or `OBJ16` after
   `set_sprite_size(Size8x16)`). Since `refactor-p2-engine-api-config` these values come from the program's
   `RustBoyConfig`: `palettes` (`%11100100` each by default; `a` is loaded once per value), `lcdc` (`BGON`, `OBJON`),
   `sprite_size`, `background_tilemap` (`| LCDCF_BG9C00`). (Since [B11](#b11)/[B28](#b28); before, user code ran before the variables were
   set, the palettes after LCD on, `rOBP1` was never set and the OAM was cleared only when sprites existed.)
4. **MainLoop**: `Main:` → `call WaitNotVBlank` → `call WaitVBlank` → animation dispatcher → user main-loop
   code (incl. `UpdateKeys` + key checks) → `jp Main`.
5. **Main (legacy)**: whatever was written through `RustBoy::raw()` to its default chunk (reached only through a
   label). Since [B15](#b15) the raw code written to the other chunks goes at the end of the same chunk (`Init`
   before LCD on, `MainLoop` before `jp Main`, see `RustBoy::raw`).
6. **Functions** (since [B24](#b24)/[B26](#b26), worked out once all the code is known, before the variables): the
   builtins, then the user functions, that the code and the animation functions refer to, directly or through
   other functions (their bodies and, since `refactor-p2-routines`, their dependencies), plus the ones forced with
   `use_function` / `keep_function`; then the `Anim_*` functions. The variables an emitted routine needs (`wCurKeys`,
   `wNewKeys` for `UpdateKeys`) are then created, unless the program defines them, so the
   variable initialisation (step 3) and the Data chunk include them.
7. **Tiles / Tilemap**: `Label:` + `dw`/`INCBIN` + `LabelEnd:`; **Data**: `SECTION "Variables", WRAM0` + `name: db/dw`
   (since `refactor-p2-engine-api-memory`: `SECTION "HRAM Variables", HRAM[$FF80]` first when the program has HRAM
   variables, then each `WRAM0` section at its address, `SECTION "Variables", WRAM0[$C000]`).

---

## 3. Are the levels correct? (assessment)

**Short answer: yes, three levels is the right idea — but the boundaries leak.** The intent
(raw instructions → reusable routines → game engine) is sound and matches how GB developers think.
The problems are where each layer reaches across the line:

1. **The assembler layer knows the game layout.** `Chunk::{Init, MainLoop, Tiles, Tilemap, Data}`
   (`src/asm/program.rs:23-43` at `4601a5c`) and their fixed order (`src/asm/codegen.rs:7-17` at `4601a5c`) are engine
   concepts. *Fixed since `refactor-p2-sections-layout`:* both are the engine's, `Chunk` and `Chunk::ORDER` in
   `src/engine/layout.rs:24-60` (see below). `include_hardware()` hardcodes `hardware.inc`
   (`src/asm/builders.rs:544-549`, since `refactor-p2-typed-operands-2b`).
   Sections were strings (`section("Header", "ROM0[$100]")`, any text), and `ds` always had a fill value, so RAM
   could not be reserved with it. *Since `refactor-p2-sections`:* sections are typed (`gb_asm::Section`, see
   [Key concepts](#key-concepts)), checked against what RGBDS 1.0.4 accepts, and a program panics on code or data in
   a RAM section; `ds` takes an optional fill. **Breaking**, with the migration:

   | Before | After |
   |---|---|
   | `asm.section("Header", "ROM0[$100]")` | `asm.section(Section::rom0("Header").at(0x100))` |
   | `asm.section("Vars", "WRAM0")`, `"HRAM"`, … | `asm.section(Section::wram0("Vars"))`, `Section::hram(..)`, or `Section::new(name, MemoryType::Hram)` |
   | `"ROMX[$4000], BANK[2]"`, `"ROM0, ALIGN[8]"` | `Section::romx(name).at(0x4000).bank(2)`, `Section::rom0(name).align(8)` |
   | `Instr::Section { name, mem_type }` | `Instr::Section(Section)` |
   | `asm.ds("$150 - @", "0")` (`Instr::Ds { num_bytes, starter_point }`) | `asm.ds_fill("$150 - @", "0")` (`Instr::Ds { count, fill: Some(..) }`); `asm.ds("4")` reserves (`fill: None`) |
   | `VariableSection::new("Vars", "WRAM0")` (`gb_std`; fields `name`, `memory`) | `VariableSection::new(Section::wram0("Vars"))` (field `section`) |
   | a section text RGBDS rejects (`"ROM0, BANK[1]"`, `"HRAM[$FFFF]"`), code or data in a RAM section, a section name used twice | panics when the `Section` is built, or when the instruction is emitted (`Asm::emit`; in a `RustBoy` program, in `build()`), with what is wrong; before, rgbasm/rgblink failed |

   *Since `refactor-p2-sections-layout`:* `Chunk` and the fixed chunk order are the engine's (`rust_boy::{Chunk,
   Layout}`, see [Key concepts](#key-concepts)); `Asm` is one program, printed in the order it is written. The
   output of `RustBoy` and of the examples is byte-identical. **Breaking**, with the migration:

   | Before | After |
   |---|---|
   | `use rust_boy::gb_asm::Chunk` | `use rust_boy::rust_boy::Chunk` |
   | `gb.raw(\|asm: &mut Asm\| …)` | `gb.raw(\|asm: &mut Layout\| …)` (the same builder methods, `chunk`, `labels`, `emit_code`; a closure without a type annotation compiles as before) |
   | `asm.chunk(Chunk::Functions)` on a `gb_asm` `Asm` program | write the parts in the order they are printed (build a part early in a `Block` and emit it later, as `unbricked_std` does), `asm.blank_line()` between them for the same text; or use a `rust_boy::Layout` and `layout.program()` |
   | `asm.get_chunk(chunk)` | `layout.get_chunk(chunk)` (engine); an `Asm` has `asm.instrs()` |
   | `asm.get_main_instrs()` | `asm.instrs()` (`&[Instr]`, every instruction as written) |
   | code or data in a RAM section panics in `Asm::to_asm` | panics in `Asm::emit` (or `emit_all`, a builder method); every builder method is `#[track_caller]`, so the panic points at the call that writes it |
2. **L1 is not really typed.** Registers and expressions are passed as strings:
   `ld_hli_label("a")`, `inc_label("de")`, `or_label("a", "c")` (`src/asm/program.rs:162-167, 382-384, 356-359`).
   `Operand::Imm`/`Label` are accepted as destinations, so `ld 1, 2` or `inc 5` compile in Rust and
   fail only in rgbasm. Instruction shapes were inconsistent (`And`/`Cp` took one operand, `Or`/`Xor`/`Sub` two;
   `AdcA` vs `Adc`). *Since Phase 2 (`refactor-p2-isa`):* one shape per family. The 8-bit ALU instructions
   (`Add`, `Adc`, `Sub`, `Sbc`, `And`, `Xor`, `Or`, `Cp`) take one source and print `op a, src`; `add hl, r16` and
   `add sp, e8` are `AddHl { src: R16 }` and `AddSp { offset: i8 }`; the rotates, shifts, `swap` and
   `bit`/`set`/`res` take an `R8` (a register or `[hl]`), `push`/`pop` an `R16Stack`; the ISA is complete; `Instr`
   derives `Debug` and `PartialEq`. What the types cannot rule out (a bit number above 7, an `rst` vector, a 16-bit
   ALU source, `[bci]`) is rejected by `Instr::check`, which `Asm::emit` and the RGBDS output call.
   *Since `refactor-p2-typed-operands`:* every operand is typed and the string helpers are gone. `ld`/`ldh` take a
   `Dst` (`R8`, `R16` or `Mem`: `[bc]`, `[de]`, `[hli]`, `[hld]`, `[c]`, `[address]`; never a value, so `ld 1, 2` does
   not compile) and an `Operand` (the same, or a value); the ALU takes an `AluOperand` (`R8` or a value), `inc`/`dec`
   an `IncDec` (`R8` or `R16`), so `inc 5`, `add a, hl` and `cp a, [wCount]` do not compile either. `Instr::check`
   accepts exactly the `ld`/`ldh` pairs of the opcode table (`ld [hl], [hl]`, `ld b, [de]`, `ld bc, de`, `ldh b, [c]`
   panic) and rejects a constant that does not fit its operand (`ld a, 300`). Values are `Expr`s
   (`src/asm/expr.rs`): numbers (decimal, `$` hex, `%` binary, negative), symbols checked to be RGBDS names and
   not registers (the string helpers let `cp_label("b")` be an expression, which `instr_size` counted as 2 bytes),
   `+ - * << >> & | ^`, `-`/`~`, `LOW`/`HIGH`, printed with RGBDS precedence (`&`, `|`, `^` bind tighter than `+`, `-`);
   `Expr::raw` passes any other RGBDS text. A `&str` where a value is expected is read as a symbol or a number, and
   panics on anything else (`"TilesEnd - Tiles"`, `"[wScore]"`, `"a"`). Migration from the removed API:

   | Before | After |
   |---|---|
   | `ld_a_label("X")`, `ld_hl_label("X")`, `ld_b_label("a")` | `ld(R8::A, "X")`, `ld(R16::HL, "X")`, `ld(R8::B, R8::A)` |
   | `ld_hli_label("a")`, `ld_addr_label_a("[hl]")` | `ld(Mem::Hli, R8::A)`, `ld(R8::AtHl, R8::A)` |
   | `ld_a_addr_reg(Register::DE)` / `(Register::HL)` | `ld(R8::A, Mem::De)` / `ld(R8::A, R8::AtHl)` |
   | `ldh_label("[$FF40]", "a")` | `ldh(Mem::addr(Expr::hex(0xFF40)), R8::A)` |
   | `add_label("a", "b")`, `add_label("hl", "bc")`, `add_label("sp", "-2")` | `add(R8::B)`, `add_hl(R16::BC)`, `add_sp(-2)` |
   | `sub_label("a", "8")`, `cp_label("BRICK")`, `and_label("%11110000")` | `sub(8)`, `cp("BRICK")`, `and(Expr::bin(0b11110000))` |
   | `inc_label("de")`, `dec_label("b")`, `srl_label("a")`, `swap_label("a")` | `inc(R16::DE)`, `dec(R8::B)`, `srl(R8::A)`, `swap(R8::A)` |
   | `Operand::Reg(Register::A)`, `Operand::Imm(5)`, `Operand::Imm16(n)` | `R8::A`, `5`, `Operand::from(n)` (or just `n`) |
   | `Operand::AddrDef("x")`, `Operand::AddrReg(Register::HL)`, `Operand::AddrRegInc(..)` | `Mem::addr("x")`, `R8::AtHl`, `Mem::Hli` |
   | `Operand::Label("TilesEnd - Tiles")` | `Expr::sym("TilesEnd") - "Tiles"` (or `Expr::raw(..)`) |
   | `ld_a_addr_def(&format!("_OAMRAM+{}", n))`, `hw::oam_address(i, b)` | `ld_a_addr_def(Expr::sym("_OAMRAM") + n)`, `hw::oam_offset(i, b)` |
   | `IfA::eq("X + 1", ..)`, `IfConst::lt(.., "SCRN_X - 8", ..)` | `IfA::eq(Expr::sym("X") + 1, ..)`, `IfConst::lt(.., Expr::sym("SCRN_X") - 8, ..)` (or `Expr::raw("X + 1")`) |
   | `IfA::eq("LOW(X)", ..)`, `IfA::eq("'A'", ..)` | `IfA::eq(Expr::low("X"), ..)`, `IfA::eq(Expr::raw("'A'"), ..)` |
   | `TileRef::load_address_label("_SCRN0 + 32")`, `TileRef::set_tile_label("T + 1")` | `load_address_label(Expr::sym("_SCRN0") + 32)`, `set_tile_label(Expr::sym("T") + 1)` |
   | `cp_in_memory("Tiles", "_VRAM + 16")` | `cp_in_memory("Tiles", Expr::sym("_VRAM") + 16)` |
   | `is_specific_tile(.., &["BRICK+1"])` | an id is a symbol or a number: define `DEF BRICK_2 EQU BRICK + 1`, or test it with `IfA` and an `Expr` |

   `IfConst`/`IfA`, `TileRef::set_tile_label`/`load_address_label` and `cp_in_memory`'s address take an
   `impl Into<Expr>`, and `is_specific_tile` (same signature) reads its tile ids as `Expr`s. **Breaking:** a `&str`
   still works only when it is a symbol or a number (spaces around it are ignored); text that is an expression
   (`"BRICK + 1"`, `"SCRN_X - 8"`, `"LOW(BRICK)"`, `"_SCRN0 + 32"`), a character literal (`"'A'"`) or a raw
   identifier (`"#name"`) assembled before and now panics when the code is generated, with a message saying what to
   write: build it with `Expr` or pass it with `Expr::raw` (rows above). RGBDS keywords (`ld`, `LOW`, `DEF`, …) are not
   symbols either. A side effect: a `RustBoy` variable whose name is not a valid RGBDS
   symbol now panics when its code is generated (`Var::set`/`get`, the start-up initialisation); before, rgbasm
   rejected the output (checking every user name stays the Phase 3 item).
3. **Hardware facts are hardcoded in every layer.** `_OAMRAM+{id*4+1}` strings in both sprite managers,
   `$9800` in three places, VRAM bases in `tiles.rs`, LCDC flags written as strings in each layer
   (until [B4](#b4) they disagreed: OBJ16 forced in `rust_boy`, not in `gb_std`). `MemoryRegion`/`MemoryAllocator` (`src/engine/memory.rs`) existed but were unused (used since [B17](#b17)); a first `hw` module (`src/hw.rs`, pure data) exists since [B22](#b22).
   *Fixed since Phase 2 (`refactor-p2-hw`):* `hw` is complete, as `hw::Symbol`s (a `hardware.inc` name and its value,
   see [Key concepts](#key-concepts)), and `gb_std`, `rust_boy` and the examples take every hardware fact from it: the
   key flags (`PadButton::flag`), the OAM clear size, the header padding, VBlank's first line, the tilemap row width,
   the LCD-off value, the score addresses of the Unbricked examples (`TileRef::from_xy`). A test fails on a new hardware
   name or address written as a string in those layers (`test_no_hardware_strings_outside_hw`; the hand-written tutorial
   `unbricked.rs` is the stated exception), and another asserts every value with RGBDS against `include/hardware.inc`.
   The output is unchanged: the 6 example ROMs, their asm, `.map` and `.sym` are byte-identical. **Breaking** (the `hw`
   API), with the migration:

   | Before | After |
   |---|---|
   | `hw::LCDC`, `LY`, `P1`, `BGP`, `OBP0`, `OBP1`, `OAMRAM`, `LCDCF_*`, `P1F_*` were `&str` | `hw::Symbol`s: pass them as they are to the builders (`ld_addr_def_a(hw::LCDC)`, `Mem::addr(hw::P1)`, `ld(R8::A, hw::P1F_GET_BTN)`); `Expr::from(hw::LCDCF_ON)` for `Expr::sym(hw::LCDCF_ON)`; `.name` for the text |
   | `hw::SCRN0`, `SCRN1` were `u16`; `OAM_COUNT`, `OAM_ENTRY_SIZE`, `OAMA_Y`, `OAMA_X`, `OAMA_TILEID` were `u8` | `hw::Symbol`s: `.value` for the number (`hw::SCRN0.value`) |
   | `hw::OAM_START`, `hw::WRAM0` | `hw::OAMRAM.value`, `hw::RAM.value` |
   | `hw::SCRN_ROW_TILES`, `hw::SCRN_ROWS` (`usize`) | `hw::SCRN_VX_B.value`, `hw::SCRN_VY_B.value` (`u8`) |
   | `hw::oam_offset(index, 1)` (any `u8` byte, any index) | `hw::oam_offset(index, hw::OAMA_X)`; panics for an index past OAM (40 entries) or any symbol but the four `OAMA_*` (`hw::OAM_ENTRY_BYTES`) |
   | `format!("{}", hw::LCDC)` | `hw::LCDC.name` (`Symbol` has no `Display`) |
   | `asm.cp(hw::LCDC)` (any `&str` constant in an 8-bit ALU operand) | does not compile: the ALU takes a `Symbol<u8>` (a flag or a count), not an address; write `asm.cp(hw::LCDC.name)` if it is really meant |
   | `gb_std` `Sprite::new(40, …)` (any id), a 41st `SpriteManager::add_sprite` | panics: an id is an OAM entry, 0 to 39 (its code read and wrote `_OAMRAM+160` and beyond) |
4. **L3 re-implements L2 instead of using it.** `src/engine/functions.rs:152-311` (at `4601a5c`) duplicated Memcopy,
   WaitVBlank, WaitNotVBlank, UpdateKeys and GetTileByPixel from `gb_std`, and they had already
   diverged ([B23](#b23)). *Since B23* `rust_boy` emits the `gb_std` routines (only `Delay` was its own), and its
   tile copies use `gb_std`'s `cp_in_memory`. L2 still contains its own `SpriteManager` that duplicates L3's.
   *Fixed since `refactor-p2-routines-sprites`:* the `gb_std` `SpriteManager` is gone; the engine's
   (`rust_boy::SpriteManager`) is the one sprite manager. A `gb_std` program uses `Sprite` values (an OAM entry, its
   position, tile and flags) and `gb_std::graphics::sprites::draw_sprites` to write them, which the engine's start-up
   code uses too (it wrote the same bytes with its own loop), as it uses `gb_std`'s `move_coord_var` for
   `move_x_var` / `move_y_var` (a copy of `Sprite::move_x_var`). Each routine is defined once in the library
   (`gb_std`); the examples written with `gb_std` or `RustBoy` emit those. `src/bin/unbricked.rs` keeps its copies
   (the stated exception); `src/bin/basic_usage.rs`, the other raw-`gb_asm` example, writes its own `WaitVBlank`, another
   routine (it waits for `rLY` = 144), and keeps it: the maintainer made it a stated exception too (CLAUDE.md), and
   added `WaitVBlank` to the list of `unbricked.rs`'s own copies (`src/bin/unbricked.rs:22`). `draw_sprites` panics on
   two sprites in one OAM entry (the independent review found that the later one silently overwrote the earlier one). The engine still writes its tile
   data and variable sections with its own code, the same text as `gb_std`'s `add_tiles` and `VariableSection` (data,
   not routines; a Task.md follow-up). The output is unchanged: the 6 example ROMs, their asm, `.map` and `.sym` are
   byte-identical. **Breaking**, with the migration:

   | Before | After |
   |---|---|
   | `let mut sm = gb_std::graphics::sprites::SpriteManager::new(); sm.add_sprite(x, y, tile, flags)` (ids from 0, in order) | `let paddle = Sprite::new(0, x, y, tile, flags)` (the id is the OAM entry, 0 to 39) |
   | `sm.draw()` | `draw_sprites([&paddle, &ball])` (the same code for entries in order; `hl` is loaded again for an entry that does not follow) |
   | `sm.get_sprite(1).unwrap().get_pivot(..)`, `get_sprite_mut(0).unwrap().move_left_limit(..)` | the `Sprite` itself: `ball.get_pivot(..)`, `paddle.move_left_limit(..)` |
   *Since Phase 2 (`refactor-p2-routines`):* routines are values (`gb_std::routine::Routine`, see
   [Key concepts](#key-concepts)) and `Delay` is `gb_std`'s too (`gb_std::utility::delay`), so the engine has no routine
   of its own: a builtin is the `gb_std` value, with its dependencies, variables and calling convention. The output is
   unchanged (the 6 example ROMs, their asm, `.map` and `.sym` are byte-identical). **Breaking**, with the migration:

   | Before | After |
   |---|---|
   | `memcopy()`, `wait_vblank()`, `wait_not_vblank()`, `update_keys()`, `get_tile_by_pixel()`, `is_specific_tile(..)` returned `Vec<Instr>` | they return a `Routine`: `asm.emit_all(memcopy())` and `code.extend(memcopy())` work as before; `memcopy().body()` (a slice) or `Vec::<Instr>::from(memcopy())` for the instructions |
   | `gb.define_function("IsWallTile", is_specific_tile("IsWallTile", ..))` | `gb.define_routine(is_specific_tile("IsWallTile", ..))` (it keeps the convention); `define_function` still takes a `Vec<Instr>` |
   | `BuiltinFunction::variables()` returned `&'static [&'static str]` | it returns `Vec<String>` (`BuiltinFunction::routine().variables()`) |
   | `Delay` was `rust_boy`'s (private) | `gb_std::utility::delay()` |
   | `build()` read a builtin's body for the functions it calls | a builtin's dependencies are its `Routine::deps` (none today); a user function's body is still read, plus its dependencies |
   | a function name such as `bc` or `ld` was found in `push bc`, `ld a, 1` (any word of the text) | typed instructions refer to their symbols only (raw text is read as before) |
5. **No layer owns labels.** `gb_std` hardcodes global labels (`Left`, `CheckLeft`, `ClearOam`), `rust_boy`
   builds them with `format!`, `If` uses local labels — they collide and break scoping ([B7](#b7), [B25](#b25)).
   *Since B7/B25:* the asm layer has a `LabelAllocator` that numbers the local labels of snippets, and
   animation labels are namespaced by sprite. *Since Phase 2 (`refactor-p2-labels`):* the allocator is the only
   source of generated labels, owned by the program's `Asm`: `Emittable::emit` takes it instead of the `If`
   counter, `If*`, the snippets, the OAM clear loop (`ClearOam` was global) and the animation dispatcher
   (`AnimEnd` was global) take their labels from it, each `.{stem}_N` with one number per allocation (see
   [Key concepts](#key-concepts)). The asm layer also turns each `jr` out of range into a `jp` when it prints the
   program (`gb_asm::relax`). **Breaking**, with the migration:

   | Before | After |
   |---|---|
   | `impl Emittable for X { fn emit(&mut self, counter: &mut usize) … }` | `fn emit(&mut self, labels: &LabelAllocator)`; take labels with `labels.local(stem)` / `labels.locals([..])` |
   | `let mut counter = 0; asm.emit_all(code.emit(&mut counter))` | `asm.emit_code(code)` (or `code.emit(asm.labels())`) |
   | `gb.next_if_counter()` | `gb.next_label_counter()` or `gb.labels().local(stem)` |
   | `clear_objects_screen()` | `clear_objects_screen(asm.labels())` (after a global label: its loop label is local) |
   | `LabelAllocator::new()` beside an `Asm` program | `asm.labels()`: the program's allocator |
   | labels `ClearOam`, `AnimEnd`, `.check_left_N_end`, `.spriteK_left_limit_N_store` / `_end` | `.clear_oam_N`, `.anim_end_N`, `.check_left_end_N`, `.spriteK_left_limit_store_N` / `_end_N` (the dispatcher's: `.anim_{sprite}_end_N`, `.skip_{sprite}_{animation}_N`) |
   | a `jr` that rgbasm rejected as out of range, or to an external symbol | assembles: printed as `jp`, one byte longer and one cycle slower when taken (4 M-cycles, `jr` 3): code of fixed size or timing (an `rst` vector, a raw fixed-size section, a cycle-counted loop) must write jumps that reach |
   | `LabelAllocator::local("check left")` (any text) | panics: a stem is made of identifier characters (letters, digits, `_`, `#`, `$`, `@`); `locals` also panics on a stem given twice |
   | a jump target `@+n` with a jump that grows in between | its offset is written again (`jp nz, @+5`); a target `Label + 2` (any other expression), or `@` anywhere else (a raw line, data, an operand; not `ds N - @`), leaves the whole program unrelaxed, so a far `jr` fails in rgbasm as before |

6. **The engine's API.** `build(&mut self) -> String` panicked on what only the whole program shows (a function also
   defined as a variable, a full WRAM, code in a RAM section), registered state on every call, and the settings of a
   program were separate setters. *Since Phase 2 (`refactor-p2-engine-api-errors`):* `build(&self) -> Result<String,
   Error>`. `rust_boy::Error` lists what can go wrong at build time: `UnknownFunction` (a name given to `call`,
   `call_args` or `keep_function` that is no function when the program is built), `NameConflict` (a name with two
   `Definition`s: a function and a variable, a constant or raw label, an external symbol, an animation function `build()`
   generates; a variable `build()` needs,
   `wFrameCounter` or `wAnim_*`, created by the program with another type), `MemoryFull` (the variables, laid out by
   `build()` with the ones it adds, do not fit in WRAM0), `Section` (code or data in a RAM section, a section name used
   twice; the asm layer's message, from `Asm::try_emit` / `Layout::try_program`). **The rule** (documented on `Error`):
   a method panics when the call itself is wrong (an invalid argument, an unknown id, a contradiction with an earlier
   call on the same object; VRAM tiles and OAM entries, which the call needs at once, are allocated there and panic when
   full), and `build()` returns an `Err` for what only the whole program shows; `build()` itself does not panic on what a
   program contains. For that, what used to panic inside `build()` is checked where it enters: a variable name, a
   routine's variable (`Routine::with_variable`) and a tile name must be symbols (not a register or keyword name), the
   section of `create_in_section` must be a section name, and an instruction built by hand is checked by `init`,
   `add_to_main_loop`, `call_args`, `define_function`, `define_routine`, `call_routine` and `add_inputs`
   (`test_build_does_not_panic_on_what_a_program_contains` gives 25 bad names through 22 ways into a program: each
   call panics, or `build()` returns `Ok` or `Err`; the independent review found the section name and the routine
   variable, which it caught, still panicking in `build()`). Building changes
   nothing, so building twice gives the same text (`test_build_changes_nothing`). Every variant is reached through
   `build()` in `test_every_error_variant_is_reachable_through_build`. The 6 example ROMs, their asm, `.map` and `.sym`
   are byte-identical. **Breaking**, with the migration:

   | Before | After |
   |---|---|
   | `let out = gb.build();` (`String`; `build(&mut self)`) | `let out = gb.build()?;` (`Result<String, rust_boy::Error>`; `build(&self)`); in a `main`: `fn main() -> Result<(), Error>` and `println!("{}", gb.build()?)` |
   | `build()` panicked: a user function also a variable, a constant / `DEF` / raw label, or an external symbol | `Err(Error::NameConflict { name, first: Definition::Function, second })` |
   | `build()` panicked: `wFrameCounter` (or `wAnim_{sprite}_Current` / `_Dir`) created by the program as another type than `u8` | `Err(Error::NameConflict { .., second: Definition::GeneratedVariable(VarType::U8) })` |
   | code or data in a RAM section, a section name used twice (through `raw()`): `build()` panicked | `Err(Error::Section(message))` |
   | `create_u8` (any `create_*`) panicked when WRAM0 was full | creating does not check the size; `build()` returns `Err(Error::MemoryFull { region, what, needed, available })`; `get_address` is `None` for a variable that does not fit |
   | `get_address`: creation order across sections | section by section, in their first-use order, as the program lists them (the same for one section) |
   | `gb.call("X")`, `call_args("X", ..)`, `keep_function("X")` panicked when `X` was not (yet) a function | no panic: `X` may be defined after the call; `build()` returns `Err(Error::UnknownFunction { name, available })` if it is still no function |
   | `call` / `keep_function` of an animation function (`Anim_{sprite}_{animation}`) worked only after a first `build()` | works at any time; `function_exists` knows them |
   | a variable named like a register or keyword (`"a"`, `"LOW"`), or not an identifier (`"w Score"`), panicked in `build()` | panics in `create_*` ("invalid variable name") |
   | a tile name that is not a symbol (`tiles.add_background("my tiles", ..)`, a sprite `"A"` or `"Low"`) panicked in `build()` (rgbasm rejected it before) | panics in `tiles.add_*` / `add_sprite` ("invalid tile name") |
   | an `Instr` built by hand that `Instr::check` rejects (`ld a, 300`), given to `init`, `add_to_main_loop`, `call_args`, `define_function`, `define_routine`, `call_routine` or an `InputManager` action, panicked in `build()` | panics at that call |

   *Since `refactor-p2-engine-api-config`:* the settings of a program are one value, `rust_boy::RustBoyConfig`, given
   to `RustBoy::with_config` (`RustBoy::new()` is `with_config(RustBoyConfig::default())`, and `RustBoy::config()` reads
   it): `sprite_size` (`SpriteSize`, 8x8), `background_tilemap` (`TilemapArea`, `$9800`), `palettes` (`Palettes { bgp,
   obp0, obp1 }`, `%11100100` each), `lcdc` (`Lcdc { background, objects }`, both on), `builtins` (the builtins emitted
   even if unused, none) and `animation_delay` (8). The defaults are the settings `RustBoy::new` had, so the default
   output is byte-identical (`test_config_defaults_reproduce_the_output`: the defaults, the same program from `new()`,
   the default config and every default written out, and the exact palette and LCDC lines). `RustBoyConfig` and `Lcdc`
   are `#[non_exhaustive]` (a setting can be added later, e.g. the window in Phase 3): build one from `default()` with
   the builder methods, one per field. The setters stay and change the same settings (`set_sprite_size`,
   `set_background_tilemap`, `set_animation_delay`, `use_function`, and `set_palettes`, new); the sprite size is also the
   sprite manager's, which needs it when a sprite is added, so both are set together. The background tile data
   (`LCDCF_BG8800`, the engine's VRAM layout) and the window (off; Phase 3) are not settings. Tests:
   `test_config_and_setters_give_the_same_program`, `test_a_forced_builtin_comes_from_the_config`,
   `test_palettes_from_the_config` and `test_lcdc_flags_from_the_config` (the start-up code on `gb_asm::test_cpu`: each
   palette register and the `rLCDC` value). `fosdem` uses `with_config` (the same asm). Not breaking, except for one row:

   | Before | After |
   |---|---|
   | `FunctionRegistry` kept the builtins of `use_function` (internal) | they are `RustBoyConfig::builtins`; `use_function` works as before |
   | (new) | `RustBoy::with_config(RustBoyConfig::default().sprite_size(..).palettes(..))`, `RustBoy::config()`, `RustBoy::set_palettes(Palettes)` |

   *Since `refactor-p2-engine-api-memory`:* **HRAM variables**, `VariableManager::create_hram_u8` / `u16` / `i8` / `i16`:
   `Var::set`, `Var::get` and the start-up initialisation use `ldh` for them (2 bytes and 3 M-cycles, `ld [n16]` 3 and 4;
   `Var::region()` says where a variable is). `MemoryRegion::Hram` is $FF80-$FFBF, 64 bytes: `RustBoy` keeps the stack
   where the boot ROM puts it (`SP` = $FFFE, never moved) and it grows down into HRAM, so the top 63 bytes are left to it
   (`HRAM_VARIABLES_END`). HRAM full is `Error::MemoryFull` from `build()`, as for WRAM0. A name is one variable: creating
   it again in the other memory panics. **Real addresses**: the choice between matching rgblink with fixed-address
   sections and documenting rgblink's placement went to fixed addresses. `build()` prints each variable section at the
   address the allocator gives its first variable (`SECTION "HRAM Variables", HRAM[$FF80]`, then `SECTION "Variables",
   WRAM0[$C000]`, the next section after it, ...), so `VariableManager::get_address` is where rgblink puts the variable,
   whatever else the program has, once its variables are created. The variables `build()` adds itself (`wFrameCounter`,
   `wAnim_*`, a routine's) go at the end of the last `WRAM0` section, after every variable of the program, so they move
   none (the independent review found that they went to the first section and moved the others: with two sections and
   an animation, `get_address` said $C001 and rgblink put the variable at $C003). Before, with floating sections rgblink placed them itself (by its own order, which puts bigger
   sections first), so the addresses held only for a program with one `WRAM0` section. The program's own sections
   (a `raw()` `SECTION`) float around the engine's; a `raw()` section fixed at an address the variables use is an
   rgblink error, as any overlap. The HRAM section comes first, so the `raw()` data still lands in the last `WRAM0`
   section (with only HRAM variables, it gets `SECTION "Raw Data", WRAM0`). The asm of `unbricked_rustboy`, `fosdem` and
   `coin-anim` changes in one line, `SECTION "Variables", WRAM0` → `WRAM0[$C000]`, where rgblink already put it: their
   ROM, `.map` and `.sym` are byte-identical (so no emulator run was needed: the bytes are the same). The test CPU
   checks an `ldh` to a symbol whose address the test gives (`TestCpu::consts16`): outside $FF00-$FFFF it panics, as
   the CPU would reach another byte. Tests: `test_hram_variables_are_read_and_written_with_ldh` (every type, set, get
   and the initialisation on the test CPU, each access an `ldh`), `test_the_test_cpu_rejects_ldh_outside_hram`,
   `test_hram_has_room_for_64_bytes_of_variables`, `test_a_variable_is_in_one_memory`, `test_hram_variables_in_a_program`
   (the start-up code on the test CPU; raw data with only HRAM variables; HRAM full), and
   `test_variables_are_where_get_address_says` (WRAM0 in two sections, HRAM, a bigger floating `raw()` section, and the
   variables `build()` adds for an animation and `UpdateKeys`: `get_address` is the same before and after the build,
   and with `RGBDS_LINK_CHECK` every variable's address in the `.sym` of RGBDS is its `get_address`, and the ROM holds
   `E0 80` / `F0 80`, `ldh [$FF80], a` / `ldh a, [$FF80]`). **Breaking**, with the migration:

   | Before | After |
   |---|---|
   | `SECTION "Variables", WRAM0` (floating) | `SECTION "Variables", WRAM0[$C000]` (each variable section at its address); a `raw()` section fixed where the variables are now overlaps them: let it float, or put it after them |
   | `MemoryRegion` matched exhaustively | `MemoryRegion::Hram` is new, and `MemoryRegion` is `#[non_exhaustive]`: add a `_` arm |
   | `Var { id, name, var_type }` (private fields) | also `region` (`Var::region()`) |
   | HRAM variables: none (`docs/variables.md` described them with `create_in_section`, which writes `WRAM0`) | `gb.vars.create_hram_u8("hSpeed", 0)` (and `u16`, `i8`, `i16`) |

### Proposed target

*Reached in Phase 2:* the layers are `rust_boy::asm`, `rust_boy::stdlib` and `rust_boy::engine`, with `rust_boy::hw` and
`rust_boy::prelude` (`refactor-p2-engine-api-prelude`). The routines layer is `stdlib`, not `std`: a module named `std`
at the crate root makes every `std::` path in the crate ambiguous, and a user's `use rust_boy::*` would do the same. The
rename changes paths only (no code), so the examples are byte-identical; the migration guide of every breaking change
of Phases 1 and 2, with the new names, is [CHANGELOG.md](CHANGELOG.md).

```
            ┌──────────────────────────────────────────────┐
  engine    │ RustBoy, managers, allocation, build pipeline│  (was rust_boy)
            └───────────────┬──────────────────────────────┘
            ┌───────────────▼──────────────────────────────┐
  stdlib    │ stateless Routines {name, body, deps,        │  (was gb_std)
            │ clobbers}, If/While/Switch, snippets          │
            └───────────────┬──────────────────────────────┘
            ┌───────────────▼──────────────────────────────┐
  asm       │ typed ISA, directives, first-class sections,  │  (was gb_asm)
            │ label allocator, Emittable + Block buffer     │
            └──────────────────────────────────────────────┘
  hw        pure data: register/flag/OAM-layout symbols (emitted as hardware.inc names),
            used by stdlib and engine; asm does not depend on it.
```

Rules: each layer depends only downward; the engine never formats register/hardware strings itself;
every routine exists exactly once; every generated label comes from the allocator. Since `refactor-p2-routines` the
std layer's routines are `Routine { name, body, deps, reads/returns/clobbers, variables }` values
(`src/stdlib/routine.rs`).

---

## 4. Bug catalogue

Severity: **P0** = broken today (build, or visibly wrong in a shipped example) · **P1** = real bug users
will hit · **P2** = latent, edge case or documentation.

### P0

#### B1
**`cargo test` does not compile.** `src/engine/variables.rs:282-284, 295-297, 306-307` pass the `Var`
returned by `create_u8/create_u16/create_i8` to `get_label/get_address/get_type`, which take `VarId`
(`:174, :179, :184`). 8 × E0308. *Fix:* return/accept the right type (e.g. store `VarId` inside `Var`, or
look up by name) and update the tests.
**Status: fixed** on `refactor-p0-fix-build` — `Var` now carries its `VarId` (`Var::id()`), tests use it.

#### B2
**Bin `coin-anim` does not compile.** `src/bin/coin-anim/main.rs:15, 18, 19` call
`enable_animation("CoinAnim")` / `disable_animation("CoinAnim")`, but the signatures are now
`enable_animation(SpriteId, u8)` / `disable_animation(SpriteId)` (`src/engine/sprites.rs:184, 210`).
Line 15 also discards its result (no effect). *Fix:* delete line 15 (the default is already disabled),
use `enable_animation(coin, idx)` / `disable_animation(coin)`.
**Status: fixed** on `refactor-p0-fix-build` (A starts the animation, B stops it).

#### B3
**Duplicate labels `wCurKeys` / `wNewKeys` in `unbricked_rustboy`.** The example creates them
(`src/bin/unbricked_rustboy/main.rs:50-51`) and `RustBoy::add_inputs` creates them again
(`src/engine/rustboy.rs:487-488`); `VariableManager::create_var` (`src/engine/variables.rs:146`) never
rejects duplicates, so `wCurKeys: db` is emitted twice → rgbasm "already defined". The same would happen
with `wFrameCounter` as soon as the example adds an animation. *Fix:* make `create_var` idempotent for
same name+type (or error on conflict); remove the manual creation from the example.
**Status: fixed** on `refactor-p1-duplicate-vars`: creating an existing name returns that variable (first
initial value and section kept), a different type panics; the example no longer creates the input variables.

#### B4
**All sprites are forced to 8×16.** `src/engine/rustboy.rs:313` (at `4601a5c`) always sets `LCDCF_OBJ16` (added in
`0be3a2f` for the FOSDEM 16×16 character). In 8×16 mode the hardware ignores bit 0 of the tile index, so
8×8 sprites draw wrong: in `unbricked_rustboy` Paddle (tile 0) and Ball (tile 1) both draw tiles 0+1;
`coin-anim`'s 8×8 frames show stacked pairs. *Fix:* sprite size in a `RustBoyConfig`; align tile indices
to even numbers in 8×16 mode.
**Status: fixed** on `refactor-p1-sprite-size`: `RustBoy::set_sprite_size(SpriteSize::{Size8x8, Size8x16})`,
**8×8 by default** (the hardware default); `build()` writes `LCDCF_OBJ8` or `LCDCF_OBJ16`. The size is set
once, before the first sprite (changing it later panics), so sizes are never mixed. In 8×16 mode every tile
index is even by construction: allocation starts at 0, a sprite with an odd tile count panics, `add_animation`
steps two tiles per frame and an odd `frame_step` panics (padding instead would draw a stray tile as the
bottom half). `add_sprite_16x16` panics in 8×8 mode. `fosdem` opts into 8×16 (byte-identical asm);
`unbricked_rustboy` and `coin-anim` now emit `LCDCF_OBJ8` (their only change), checked in an emulator.

#### B5
**Two-operand `If` comparisons are inverted.** `If::emit` (`src/stdlib/flow/flow_if.rs:287-327`) runs
left → `ld b, a` (`:302`), right → `a`, then `cp b` (`:308`), so the flags describe **right − left**:
`If::lt(l, r)` is true when `r < l`. The docs (`:123, :138, :153, :168`) say `l < r`.
- `unbricked_rustboy` (`src/bin/unbricked_rustboy/main.rs:124-134`) works only because its arguments were
  swapped to compensate (commit `e9a3690` "fix paddle bounce").
- `unbricked_std` (`src/bin/unbricked_std/main.rs:174-182`) follows the documented meaning, so its two
  conditions can never both hold → **the paddle never bounces**.
- Latent: the right-hand side can clobber `b` (e.g. a `Call` with `get_pivot`, which does `ld b, a`).

*Fix:* evaluate right first into `b`, then left into `a`, `cp b` (or swap via another register); then fix
both examples to the documented meaning. Decide the semantics once (see Task.md Phase 0).
**Decision (2026-10-07):** `If::lt(l, r)` means `l < r`. **Status: fixed** on `refactor-p1-if-semantics`:
right is evaluated first into `b`, left into `a`, then `cp b`; `unbricked_rustboy` is written in natural
order again (byte-identical asm), `unbricked_std` now bounces and its right edge is `+16` (was `+24`).
Regression test `test_if_compares_left_with_right` runs the emitted code for every operator. The left
operand must not change `b` (documented; a register-safe `If` is planned in Phase 2).
**Phase 2 (`refactor-p2-routines-if`):** the registers of each `If` kind are documented in the clobber model of
routines (`Regs`, see [Key concepts](#key-concepts)): `If::clobbers()` = `a`, `b`, flags (the bodies start with `a` =
left, `b` = right); `IfConst` = `a`, flags; `IfA` = flags (`a` is kept); `IfCall` = flags (the routine's result; the
routine changes what its convention says). The choice: document, not save. Saving every register the `If` uses
(`push bc` / `pop bc` around the whole compare, 2 bytes and 7 M-cycles per `If`) would change the ROM of both
Unbricked examples for no bug. The latent bug is fixed instead where it is: left code that may change `b` (a `call`,
`rst`, `jp hl`, raw code, or an instruction that writes `b`, read by `Regs::written_by`) is wrapped in `push bc` /
`pop bc`, so the compare reads the right value (the left code's changes to `b` and `c` are undone). Left code that
cannot change `b` gets no `push`/`pop`: no example's does, so the 6 example ROMs, their asm, `.map` and `.sym` are
byte-identical. `Regs::written_by` does not guess: data in the code (`db`, `dw`, `ds`, `INCBIN`), a `SECTION`, or a jump
to a label the code does not define makes it unknown, so `bc` is saved (fixed after the independent review, which found
they counted as writing nothing). The left code must leave the stack balanced: with `bc` saved, a `pop bc` in it would
take the `If`'s saved value (documented on `If`). Tests: `test_each_if_kind_changes_only_what_it_lists` (every kind and operator, with and without
else, on the test CPU: the changed registers are exactly the listed ones), `test_if_left_code_that_changes_b` (it
failed before: left code writing `b`), `test_if_left_code_that_calls_a_routine` (a left `Call` to `GetTileByPixel`),
`test_if_left_code_that_keeps_b_is_emitted_as_before`, and `test_written_by_covers_what_each_instruction_changes`
(`Regs::written_by` against the test CPU).

#### B6
**Composite (16×16) sprites split apart at screen edges.** `move_composite_{left,right}_limit`
(`src/engine/sprites.rs:333-366`) apply the same absolute limit to each 8×16 half independently. In
`fosdem` the halves start at OAM X 88/96; holding Left, the left half stops at X=1 but the right half keeps
going until it also reaches 1 → the character collapses to 8 px wide and stays so (visible in
`examples/fosdem/main.asm:119-156`). *Fix:* test only the leading half and move all halves together (or
offset each half's limit by its position in the composite).
**Status: fixed** on `refactor-p1-sprite-limits`: a composite move tests only its leading sprite (the leftmost,
rightmost, topmost or lowest one at creation; the first on a tie), clamps it as in [B8](#b8), then puts every
other sprite at the offset from it that it was created with, so the composite moves as one block or not at all
(and a split composite is put back together on the next move). `fosdem` keeps its stop positions (limits
written as `1`/`149`, see B8); checked in an emulator (PyBoy): before, both halves ended at X 1, then 149;
now the halves stay 8 px apart. Regression test `test_composite_moves_as_one_block`.

### P1

#### B7
**Reusable snippets emit fixed global labels.** (Line numbers at `7b54900`, after [B8](#b8).)
- `check_key` → `CheckLeft` / `CheckLeftEnd` (`src/stdlib/inputs.rs:129-140`): two bindings on the same
  button → duplicate label.
- `move_*_limit` → `Sprite{N}LeftLimitStore` / `Sprite{N}LeftLimitEnd` etc., built at
  `src/engine/sprites.rs:668` (one sprite) and `:540` (a composite, which uses its leading sprite's labels,
  the same as that sprite's own move), with the suffixes added by `move_coord_limit`
  (`src/stdlib/graphics/sprites.rs:53-54`): the same move used twice (e.g. two buttons) → duplicate label.
- `gb_std` `Sprite::move_*` → `Left`/`LeftEnd` (`src/stdlib/graphics/sprites.rs:169-211`) and
  `LeftLimit`/`LeftLimitStore`/`LeftLimitEnd`… (`:216-279`, `:53-54`), with no sprite id: two sprites → duplicate label.
- Scope: a global label inside an `If` body (e.g. `If::eq(.., gb.sprites.move_left_limit(..))`) makes the
  `.end_if_N:` definition land under the new global scope while the `jp` referenced it under the old one →
  unresolved symbol (rgblink: "Undefined symbol `Main.end_if_0`").

*Fix:* a label allocator; generated labels local or uniquely numbered.
**Status: fixed** on `refactor-p1-unique-labels`. `gb_asm::LabelAllocator` hands out numbered **local**
labels (`.{stem}_{n}`); its clones share one counter, so every label of a program comes from one sequence,
in call order (deterministic). Local labels never change the RGBDS scope, so a snippet inside an `If` body
no longer hides the `If`'s `.end_if_N`. `check_key` emits `.check_left_N` / `.check_left_N_end`, a limited
move `.sprite{oam}_{dir}_limit_N_store` / `…_end`; the plain `gb_std` moves (`Left:`/`LeftEnd:`) and the
`LeftLimit:` markers jumped nowhere and are gone. `RustBoy` owns the allocator and shares it with its
`SpriteManager` (the public move API is unchanged) and with `add_inputs`; `RustBoy::unique_label` uses it
too. `gb_std` is stateless, so `check_key` and `Sprite::move_*_limit` now take a `&LabelAllocator`
(breaking change for direct `gb_std` users; `unbricked_std` updated); a `RustBoy` program that mixes them in
passes `gb.labels()`, the program's own allocator (a new one would start again at 0 and repeat its labels).
Only label names change in the examples' asm: all 6 ROMs are byte-identical. Tests: the same check or move
twice, on two buttons, from two `InputManager`s, inside an `If`/else and a function, for single and composite
sprites, and `gb_std` snippets mixed into a `RustBoy` program; `gb_asm::label_check`
checks a whole program's labels with the RGBDS scope rules (it agrees with rgbasm/rgblink on all examples).
`If` kept its own counter (`.end_if_N`), and `ClearOam`/`AnimEnd` stayed global (emitted once, outside user
code); *since Phase 2 (`refactor-p2-labels`)* one allocator gives every generated label, these included, and the
suffixed labels end with their number (`.check_left_end_N`, `.sprite0_left_limit_store_N`).

#### B8
**`move_*_limit` only stops on exact equality.** `cp limit` + `jp z` (`src/engine/sprites.rs:490, 510,
530, 550`; `src/stdlib/graphics/sprites.rs:132, 146, 160, 174`). With distance 2 from x=24 toward limit 15
the sprite goes 22, 20, 18, 16, 14 … and wraps through 0/255. A start position already past the limit
never stops. *Fix:* compare with carry (`jr c`/`jr nc`) and clamp.
**Decision and status: fixed** on `refactor-p1-sprite-limits`. The limit is **included and the move clamps to
it**: a step that would go past the limit stops exactly on it (so the sprite reaches the same edge at any
speed), and a sprite already past the limit does not move in that direction (it is not pulled back either).
The code is generated once, by `gb_std::graphics::sprites::move_coord_limit`, which both `Sprite::move_*_limit`
(`gb_std`) and `SpriteManager::move_*_limit` (`rust_boy`) use: it works on `A = coord - limit` and tests the
carry flag (`jp c`/`jp nc`), adding one label `…Store` next to `…End`. Before, the limit was the first position
the sprite could not reach, so the examples' limits moved by one to keep their exact stop positions
(Unbricked paddle `16`/`104`, was `15`/`105`; `fosdem` `1`/`149`, was `0`/`150`); the README quick-start limits
were already written as included. Tests run the generated code on a CPU model (`gb_asm::test_cpu`, shared with
the `If` test) for every start position.

#### B9
**`jr` out of range in the animation dispatcher.** `jr c, AnimEnd` (`src/engine/sprites.rs:661`, label at
`:702`) jumps over the whole dispatch block: 5 bytes + 7 per animated sprite + 9 per animation. FOSDEM
(2 sprites × 4 animations) = 91 bytes; **3 sprites × 4 animations = 134 bytes > 127** → rgbasm error.
Two 16×16 animated characters are enough. *Fix:* `jp`, or a jump table.
**Status: fixed** on `refactor-p1-animations`. Every jump whose distance grows with the number of sprites or
animations is a `jp`: `jp c, AnimEnd`, a sprite's `jp z, .animEnd_{sprite}` (disabled) and `jp .animEnd_{sprite}`
after each call (with 16 animations on one sprite these two were out of range too). The only `jr` left,
`jr nz, .skip_{sprite}_{animation}`, always skips 6 bytes (`call` + `jp`). The dispatcher grows by 1 byte per
`jp` (`fosdem` +11 bytes, `coin-anim` +3: their only change, same animation in an emulator). A jump table was not
chosen: it needs `jp hl`, which the typed ISA did not have yet (it does since Phase 2, `refactor-p2-isa`). Test
`test_animation_dispatch_jumps_stay_in_range` checks every `jr` with `gb_asm::label_check::jr_range_errors`, which
knows instruction sizes and agrees with RGBDS 1.0.4 (127 and -128 accepted, 128 and -129 rejected); with the old
code it reports the offsets 285, 144 and 135 that rgblink reports for the same program. *Since Phase 2
(`refactor-p2-labels`)* any `jr` out of range becomes a `jp` when the program is printed (`gb_asm::relax`); these
`jp`s stay as they are (a `jp` is never shortened), so the examples' ROMs did not change.

#### B10
**`AnimationType::PingPong` and `::Once` are ignored.** `Animation.anim_type`
(`src/engine/animations.rs:17` at `4601a5c`) was never read; `generate_loop_func` (`:23-61`) always looped. Advertised in
the `add_animation` docs (`src/engine/sprites.rs:118`) and in `docs/animations.md` (documentations branch).
*Fix:* implement them (or remove the variants until implemented).
**Status: fixed** on `refactor-p1-animations`, with the meaning `docs/animations.md` gives them. Each mode has its
own function body (`Animation::generate_func`); `Loop` is unchanged.
- `PingPong` plays forward, then backward, and repeats; the end frames are shown once per turn
  (0 1 2 3 2 1 0 1 …). The direction is kept in a new WRAM variable `wAnim_{sprite}_Dir` (0 forward, 1 backward),
  created only for a sprite that has a `PingPong` animation of two frames or more (so `fosdem` and `coin-anim` get none). The direction
  is followed only between the two ends: the first frame always goes forward and the last one backward, so a
  direction left over from another animation does no harm. A one-frame `PingPong` stays on its frame.
- `Once` plays to the last frame and stays there (0 1 2 3 3 3 …), still enabled. Enabled again while the sprite
  shows that last frame, it does not replay; after another animation it starts again from its first frame.
  End-of-animation events stay in Phase 3.
- As for `Loop`, a sprite that shows none of the animation's frames starts on the first one. `PingPong` and
  `Once` compare with the last frame itself, so they do not have the `Loop` overflow of [B17](#b17).

Tests run the dispatcher and the functions frame by frame on `gb_asm::test_cpu` (which now models `call`, `ret`,
`ret cc`, `inc`/`dec` and the RGBDS scope of local labels) for every mode, in 8×8 and 8×16, starting outside or
on the first frame, one-frame animations, switching between animations, the delay, and a composite whose two
halves stay on the same frame; the ROM of a 3-sprite program was also checked in an emulator (PyBoy).

#### B11
**Code passed to `gb.init()` is overwritten.** (Lines at `4601a5c`.) `build()` emits user init code at
`src/engine/rustboy.rs:297`, then creates the animation variables (`:300-306`) and emits variable
initialisation at `:310`, which writes every variable's initial value. `gb.init(lives.set(3))` ends with
`wLives = 0`; `gb.init(gb.sprites.enable_animation(coin, 0))` is reset to 255 (disabled). LCDC/palette
writes in init are likewise overwritten by `:313-320`. *Fix:* emit variable init (and hardware defaults)
**before** user init code.
**Status: fixed** on `refactor-p1-init-order`. The start-up code now runs: LCD off → VRAM copies → OAM clear and
initial sprites → default palettes → every variable set to its initial value (the animation variables
`wFrameCounter`, `wAnim_{sprite}_Current` and `wAnim_{sprite}_Dir` are created first, so they are included) →
**user `init()` code** (then the `raw()` code written to `Chunk::Init`, [B15](#b15)) → LCD on (`src/engine/rustboy.rs:515-560`, put together at `:661-664`). So `gb.init(lives.set(3))`,
`gb.init(gb.sprites.enable_animation(coin, 0))`, a `PingPong` direction or a palette set in `init()` survive.
One difference from the fix above: **`rLCDC` stays after the user code**, because turning the LCD on ends
the start-up, and `init()` code keeps running with the LCD off, so it can still write VRAM and OAM freely; an
`rLCDC` value written in `init()` is still replaced (documented on `RustBoy::init`; LCDC flags belong to the
Phase 2 `RustBoyConfig`: since `refactor-p2-engine-api-config`, `RustBoyConfig::lcdc`, `sprite_size` and
`background_tilemap`, and the palettes are `RustBoyConfig::palettes`). Also documented there, and not new: `init()` code must not wait for VBlank, because with
the LCD off `rLY` stays 0 and the wait never ends. Tests run the start-up code (`RustBoy::build_layout`, `build_asm` then, the `Init` chunk) on
`gb_asm::test_cpu`, which now models the register pairs `bc`/`de`/`hl` as symbolic addresses
(`ld hl, _OAMRAM` + `ld [hli], a`; one address has one name, `_OAMRAM+4+1` is `_OAMRAM+5`), stubbed routines
(`Memcopy`) and an ordered trace of writes and calls; what it cannot know panics when read (the 8-bit halves of a
pair loaded with an address, and every register, pair and flag after a stub):
`test_init_code_runs_after_variable_initialisation` (a variable, the animation, the `PingPong` direction and
`rBGP` set in `init()`) and `test_startup_order` (the order above).

#### B12
**OAM is accessed directly, without shadow OAM + DMA.** Sprite moves, `get_x/get_y/get_pivot` and the
animation functions read-modify-write `_OAMRAM+n` from the main loop (`src/engine/sprites.rs:769-1010`,
`src/engine/animations.rs:86-203`, the `Loop`, `Once` and `PingPong` bodies since [B10](#b10); loop at `src/engine/rustboy.rs:565-584`). OAM is only accessible in
VBlank/HBlank: in modes 2/3 writes are dropped and reads return `$FF`. It works only while the whole main
loop fits in VBlank (~1140 M-cycles; `unbricked_rustboy` already uses ~600). Growth → silent sprite glitches.
*Fix:* shadow OAM in WRAM (`ALIGN[8]`) + OAM DMA routine in HRAM, run in VBlank.
**Status: open; today's behaviour is tested** (`refactor-p3-tooling-emulator`, `b12_oam_written_outside_vblank_is_lost` in
`tests/emulator.rs`): a `RustBoy` program that moves a sprite after ~7000 M-cycles of `Delay` moves it while the PPU is
in mode 3 of line 52, so the read gives `$FF`, the write is dropped and the sprite never moves; without the delay it
moves one pixel a frame. The 6 examples write OAM only in VBlank today (each example test checks it).

### P2

#### B13
**Generated assembly is non-deterministic.** Output order depends on `HashMap`/`HashSet` iteration:
`src/engine/functions.rs:67-71, 133-147`; `src/engine/variables.rs:99-102, 194, 214`;
`src/engine/sprites.rs:49-50, 629, 668, 713`; `src/engine/tiles.rs:72, 186, 206, 234, 257`;
`src/stdlib/variables.rs:21, 41`. Reproduced: 3 runs of `fosdem` / `unbricked_rustboy` → 3 different files.
No behavioural impact today, but it makes diffs and snapshot tests impossible (must be fixed before them).
*Fix:* `BTreeMap` / insertion-ordered `Vec`.
**Status: fixed** on `refactor-p0-deterministic-output`: id-keyed managers use `BTreeMap` (ids are
sequential, so creation order), name-keyed lists (user functions, variable sections, `gb_std`
`VariableSection`) use a `Vec` in declaration order, builtins come in enum order. `Asm.chunks` stays a
`HashMap` because `to_asm` reads it in a fixed order (since `refactor-p2-sections-layout` an `Asm` is one ordered
list, and the engine's `Layout` keeps its chunks in a `BTreeMap`, read in `Chunk::ORDER`).

#### B14
**`build()` is not idempotent.** `build(&mut self)` (`src/engine/rustboy.rs:315`, through `build_asm`) creates `wFrameCounter`
and the `wAnim_*_Current` variables on every call (`:368-375`) → a second `build()` emits duplicate labels.
*Fix:* `build(&self)`, all registration done up front.
**Status: fixed** on `refactor-p1-duplicate-vars`: with B3 fixed, the variables created again by a second
`build()` are the existing ones, so two builds give the same output (tested). Making `build` take `&self`
stays in Phase 2. *Done in Phase 2 (`refactor-p2-engine-api-errors`):* `build(&self) -> Result<String, Error>`; the
variables `build()` adds (`wFrameCounter`, `wAnim_*`, a routine's) go to a copy of the variable manager, and the
animation functions are no longer registered by a build (`call` and `keep_function` know them from the sprites), so a
build changes nothing: `test_build_twice_gives_the_same_output` (through a shared reference) and
`test_build_changes_nothing` (built 0, 1 or 3 times, then extended: the same text; the program's variables unchanged).

#### B15
**`RustBoy::raw()` silently drops code.** The closure runs on `self.asm` (`src/engine/rustboy.rs:196-202`)
but `build()` copies only its `Chunk::Main` (`:434-438`); anything written after `asm.chunk(Chunk::Functions)`
inside the closure is lost (and later `raw()` calls too, since the chunk persists). The `Main` chunk is printed
right after `jp Main`, so raw code is unreachable unless it starts with a label that is called — the doc
example (`ld a, 0x42; ret`) is dead code. *Fix:* merge all chunks; document placement.
**Status: fixed** on `refactor-p1-api-safety`. Each `raw()` call starts in `Chunk::Main` again, and `build()` keeps every
chunk the raw code wrote, each one right after the code it generates for that chunk (`RustBoy::raw_chunk`): `Header`,
`Constants`, `Tiles`, `Tilemap` after theirs; `Init` at start-up after the `init()` code and before the LCD is turned on
(so it runs, like `init()` code); `MainLoop` in the main loop after the `add_to_main_loop` code and before `jp Main` (it
runs every frame); `Main` where it was, after `jp Main` (reached only through a label); `Functions` after the generated
functions; `Data` after the variable sections (so inside the last `WRAM0` one, unless the raw code opens a `SECTION`;
in a program without variables, raw `Data` code that does not start with a `SECTION` gets `SECTION "Raw Data", WRAM0`,
where it used to land in the ROM0 section of the code: `test_raw_data_always_lands_in_wram`). The raw `Functions` and `Data` code is scanned like the rest, so a
builtin a raw routine calls is emitted (with its variables) and a name it defines is the program's ([B26](#b26)). The
`raw()` doc example is now a compiled doctest: a labelled routine called from the main loop, and `MainLoop` code. No
example changes (no example writes to another chunk). Tests: `test_raw_keeps_every_chunk` (each chunk, a second
`raw()` call, a builtin called from a raw function; linked with RGBDS) and `test_raw_init_and_main_loop_code_runs`
(the start-up code on `gb_asm::test_cpu`: the raw `Init` code runs after the `init()` code and before LCD on; the raw
`MainLoop` code is before `jp Main`); both failed before (the code was dropped).

#### B16
**`Var::set`/`Var::get` ignore 16-bit variables.** `set(value: i8)` (`src/engine/variables.rs:31`) writes
only the low byte; `get` (`:43`) loads one byte; `var_type` (`:26`) is never read. (Setting a `u8` > 127
works via `200u8 as i8`, but the API is awkward.) *Fix:* typed setters per `VarType`, 16-bit load/store.
**Status: fixed** on `refactor-p1-api-safety`. `Var::set(value: impl Into<i32>)` takes any integer (`set(-1)`,
`set(200u8)`, `set(1000)`) and panics, naming the variable, if the value is out of the range of its type
(`VarType::range`: `U8` 0 to 255, `I8` -128 to 127, `U16` 0 to 65535, `I16` -32768 to 32767). An 8-bit variable gets the
same code as before (`ld a, -1` / `ld [name], a`); a 16-bit one both bytes, little-endian as `dw` stores them (`name`,
then `name+1`). `Var::get` loads an 8-bit variable into `a` as before, a 16-bit one into `hl` (`a` holds the high byte;
the `If` comparisons, which test `a`, are for 8-bit variables). `set` and the start-up initialisation share one function,
so an `I16` initial value is written as two plain bytes (`ld a, 255`, was `ld a, -1`: the same byte). `Var::var_type()`
reads the type, and `Var` is exported. `create_in_section` panics on an initial value out of its type's range (it was
cut to a byte or a word). Breaking: `set` took an `i8`, so a call with an `i8` *variable* still compiles (`impl
Into<i32>`), but `u8var.set(200u8 as i8)` (the old workaround, -56) now panics: write `set(200)`. Tests run the code
on `gb_asm::test_cpu`, which now reads an 8-bit operand written as a number (`ld a, -1`), -128 to 255 like rgbasm, and
panics outside: `test_set_writes_both_bytes_of_a_16_bit_variable`, `test_get_loads_a_16_bit_variable_into_hl` (both
failed before: the high byte was not written, `hl` not loaded), `test_set_panics_on_a_value_out_of_the_type_range`,
`test_set_takes_any_value_of_the_variable_type`, `test_set_and_get_an_8_bit_variable`,
`test_initial_values_of_every_type`, `test_an_initial_value_must_fit_the_type`. No example changes.

#### B17
**No bounds or overflow checks.**
- VRAM: sprite tiles past `$8FFF` (`src/engine/tiles.rs:94`), BG tiles past `$97FF` run into the tilemap (`:117`).
- OAM: no 40-sprite cap; `generate_init_code` writes past `$FE9F`.
- `u8` arithmetic that panics in debug / wraps in release: `next_tile_index += tile_count`
  (`src/engine/sprites.rs:82`, overflows at exactly 256 tiles, e.g. two FOSDEM characters),
  `sprite.y + 16` / `sprite.x + 8` (`:425, :429`), `x + 8` (`src/engine/rustboy.rs:577`),
  `tile_count() as u8` (`:518`), `oam_index * 4` (many sites).
- `cp_imm(abs_end + self.frame_step)` (`src/engine/animations.rs:104` since [B10](#b10), `:50` at `4601a5c`) overflows when the last frame is tile
  254/255 → in release `cp 0`, the animation freezes on its first frame. (Since [B10](#b10) only `Loop` does this.)
- `MemoryAllocator` (`src/engine/memory.rs:36`) exists, with overflow checks, but nothing uses it.

*Fix:* use the allocator, `u16` counters + `checked_add`, clear errors.
**Status: fixed** on `refactor-p1-api-safety`. The managers allocate through `MemoryAllocator` (`src/engine/memory.rs`,
with the regions from the new `hw` module), whose `allocate_or_panic` panics with "no room for {what}: N bytes needed,
but {region} (…) has M bytes left":
- `TileManager`: sprite tiles in `MemoryRegion::SpriteTiles` ($8000-$8FFF, 256 tiles), background tiles in
  `MemoryRegion::BackgroundTiles` ($9000-$97FF, 128 tiles, before the tilemaps). Sizes are counted in `usize`
  (`TileSource::size_bytes` panics past 65535 bytes instead of wrapping); the sprite tile index comes from the VRAM
  address ([B18](#b18)), so the `u8` counter that overflowed at 256 tiles is gone (two FOSDEM characters, 2 × 128 tiles,
  now fit exactly, tested).
- `VariableManager`: `MemoryRegion::Wram0` ($C000-$CFFF): every variable section is a `WRAM0` section, so all of them
  share its 4 KiB (rgblink rejected a bigger section; the counter could also wrap). *Since Phase 2
  (`refactor-p2-engine-api-errors`)* the variables are laid out when the program is built, section by section with the
  ones `build()` adds, and `build()` returns `Error::MemoryFull` when they do not fit (creating one panicked);
  `get_address` follows the sections.
- `SpriteManager`: `MemoryRegion::Oam`, 40 entries; a 41st sprite panics (it was written past `$FE9F`). `x` above 247
  or `y` above 239 panics (OAM X = x + 8 and OAM Y = y + 16 are bytes); `add_sprite_16x16` checks its right half,
  x + 8, first (x at most 239). `oam_index * 4` cannot overflow any more (at most 159).
- Animations: `add_animation_with_step` panics when `start_frame` > `end_frame`, `frame_step` is 0, or a frame is not
  among the sprite's own tiles (it showed the next sprite's tiles, and past tile 255 the `u8` frame arithmetic overflowed).
  A sprite whose frames come from several blobs (several `.2bpp` files; on `refactor` the frames simply ran on into tiles
  added after it with `tiles.add_sprite`) gets the other tiles with **`RustBoy::add_sprite_tiles(sprite, name, source)`**:
  they go right after the sprite's tiles and count as its own; it panics if other sprite tiles were added in between (the
  frames would not be contiguous), and the frame check's message points to it. Tests
  `test_a_sprite_with_tiles_from_two_sources` (plays the four frames on `gb_asm::test_cpu`, links) and
  `test_add_sprite_tiles_must_follow_the_sprite`.
  The `Loop` code compares the current tile with the first and the last frame themselves (like `Once` and `PingPong`):
  `cp first` / `jr c, .reset` / `cp last` / `jr c, .next` / `.reset: ld a, first - step` / `.next: inc a` (or
  `add a, step`) / store. The reset loads the tile *before* the first frame (modulo 256: 255 for frame 0) and falls into
  the step, so the code has exactly the same size as before (17 bytes in 8x8, 18 in 8x16) and nothing else in the ROM
  moves. Before, `cp last + step` was `cp 256` for a last frame on tile 255 (8x8) or 254 (8x16): a panic when generated
  in debug, `cp 0` in release (the animation froze on its first frame). For every tile and frame range it does what the old code did
  (checked exhaustively), except with a step of 2 when the sprite shows the odd tile just before the first frame (255
  when the first frame is tile 0): the old code went to the tile after the first frame, an odd one, the new code goes
  to the first frame.
- `MemoryRegion` has three new variants (`SpriteTiles`, `BackgroundTiles`, `Wram0`) and `size()`: breaking for code that
  matches on it exhaustively.

`fosdem` and `coin-anim` change only in their `Loop` functions (and `Memcopy`, [B27](#b27)); their ROMs were run in an
emulator (PyBoy) before and after for 1500 frames with the same inputs (A, B and every direction): the OAM and the
screen are identical on every frame, and every frame of every animation is shown. Tests: `test_sprite_tiles_must_fit_in_their_vram_block`,
`test_background_tiles_must_fit_before_the_tilemap`, `test_at_most_40_sprites`, `test_sprite_position_must_fit_in_oam`,
`test_variables_must_fit_in_wram0`, `test_animation_frames_must_be_the_sprite_tiles`,
`test_loop_animation_ending_on_the_last_sprite_tile` (runs the animation on `gb_asm::test_cpu` in 8x8 and 8x16; it
overflowed before), `test_allocation_stops_at_the_end_of_the_region`, `test_regions`; all the new ones failed before.
Not covered: a `.2bpp` file shorter or longer than the tile count given to `from_file` (its size is only known when
assembled).

#### B18
**Two tile counters can desync.** `SpriteManager.next_tile_index` (`src/engine/sprites.rs:82`) and
`TileManager.next_sprite_addr` (`src/engine/tiles.rs:94`) are kept in sync only by `RustBoy::add_sprite`.
Calling the public `gb.tiles.add_sprite` or `gb.sprites.add` directly (the `RustBoy` doc example does) → wrong
tile indices. *Fix:* single source of truth.
**Status: fixed** on `refactor-p1-api-safety`: the tile manager is the one source. `RustBoy::add_sprite` adds the tiles,
then takes the sprite's tile index from their VRAM address (`TileManager::sprite_tile_index`: `($8000 + 16 n) → n`) and
gives it to the sprite manager, which no longer counts tiles (`next_tile_index` is gone). So tiles added alone with
`gb.tiles.add_sprite` (still public: tiles a program swaps in itself) just move the next sprite further, index and VRAM
copy together. **Breaking:** `SpriteManager::add` is `pub(crate)` now (it takes the tiles); `RustBoy::add_sprite` is
the way to add a sprite, as every example and the README already do. In 8x16 mode a sprite that would start on an odd tile
(after an odd number of tiles added alone) panics, as an odd tile count already did ([B4](#b4)). Tests:
`test_sprite_tiles_have_one_source` (failed before: tile index 1, VRAM `$8040`; it also runs the start-up code to check
the OAM tile byte) and `test_8x16_sprite_after_an_odd_number_of_tiles_panics`. No example changes.

#### B19
**Every tilemap goes to `$9800`.** `add_tilemap` hardcodes `vram_address: 0x9800`
(`src/engine/tiles.rs:160`); with two tilemaps, the last one created wins (it was random before B13). No `$9C00`.
**Status: fixed** on `refactor-p1-api-safety`. `TilemapArea::{Map9800, Map9C00}` (exported, addresses from `hw`) names
the two maps; `TileManager::add_tilemap_at(name, area, rows)` puts a tilemap at either, and `add_tilemap` is
`add_tilemap_at(.., Map9800)` as before. A second tilemap on the same map panics, naming both (each is copied to the
start of its map, so one would replace the other), and so does a tilemap of more than 32 rows (it ran into the next map,
or out of VRAM from `$9C00`). **`RustBoy::set_background_tilemap(area)`** chooses the map the background shows: `build()`
adds `| LCDCF_BG9C00` to the LCD-on value for `Map9C00` (for `Map9800`, the default, the LCDC line is unchanged: the flag
is 0). Not done: the window layer, which could show the other map (Phase 3), and `GetTileByPixel`, which still reads the
`$9800` map (documented on `set_background_tilemap`). No example changes. Tests: `test_a_second_tilemap_on_one_map_panics`
and `test_a_tilemap_has_at_most_32_rows` (both failed before: no panic), `test_a_tilemap_at_9c00` (the start-up code with
the real `Memcopy` on `gb_asm::test_cpu` copies each map to its address; the LCDC value; linked with RGBDS).

#### B20
**Silent failures.** Unknown `SpriteId`/`CompositeSpriteId` → empty `Vec` (move/get/enable methods in
`src/engine/sprites.rs`); `enable_animation_by_name` / `set_initial_animation_by_name` (`:197, :171`) do
nothing on a typo; `add_animation_with_step` returns 0 for an unknown id. *Fix:* `Result` or panic with a
clear message.
**Status: fixed** on `refactor-p1-api-safety` with panics (like the other API checks; a `Result` API is the Phase 2
`build(&self) -> Result` item: done in `refactor-p2-engine-api-errors`, where these stay panics, by the rule on
`rust_boy::Error`: an unknown id is wrong at the call). In `SpriteManager` (`src/engine/sprites.rs`), every method that takes a `SpriteId` or a
`CompositeSpriteId` (moves, `move_*_var`, `get_x`/`get_y`/`get_pivot`, the animation methods) looks it up with
`sprite()` / `composite()`, which panic with "unknown sprite id N" / "unknown composite sprite id N". The animation methods
also panic, naming the sprite and listing what it has, on: an unknown name (`enable_animation_by_name`,
`set_initial_animation_by_name`); an index the sprite does not have (`enable_animation`, `set_initial_animation`, which
still takes `ANIM_DISABLED`; so `set_initial_animation(id, 0)` must now come after `add_animation`, where it could come
before it on `refactor`); `enable_animation(.., ANIM_DISABLED)` (use `disable_animation`); `enable_animation` /
`disable_animation` on a sprite without animations, whose `wAnim_{sprite}_Current` does not exist (it was an undefined
symbol at link time, see [Checked and refuted](#checked-and-refuted)); and a 256th animation on one sprite (indices are
`u8` and 255 is `ANIM_DISABLED`: the 256th got index 255, which disables the sprite). The one exception is `get_composite_sprites`, a query, which keeps returning an `Option` (`None` for an unknown id). `TileId` and `VarId` are only used by queries that return an `Option`
(`get_address`, `get_label`, `get_type`), so nothing generates code from an unknown one. No example changes. Tests:
`test_an_unknown_sprite_id_panics` (15 methods), `test_an_unknown_composite_id_panics` (8),
`test_an_unknown_animation_name_panics`, `test_an_unknown_animation_index_panics`, `test_at_most_255_animations_per_sprite`;
all failed before (no panic).

#### B21
**`basic_usage` and the README Basic Example lack header padding.** `SECTION "Header", ROM0[$100]` with only
`nop` + `jp EntryPoint` and no `ds $150 - @, 0` (`src/bin/basic_usage.rs:8`, `README.md:53`). Once the floating
ROM0 section grows past 256 bytes, rgblink can place it at `$0104`, where `rgbfix` overwrites the cartridge
header. (`RustBoy` itself is correct: `src/stdlib/utility.rs:5-7`.)
**Status: fixed** on `refactor-p0-readme`: both now emit `ds $150 - @, 0`.

#### B22
**`get_pivot` silently clamps.** `u8::try_from(16 + y_offset).unwrap_or(0)` (`src/engine/sprites.rs:570, 576`;
`src/stdlib/graphics/sprites.rs:208, 214`) turns out-of-range offsets into `sub 0`. *Fix:* wrapping arithmetic
or an error.
**Status: fixed** on `refactor-p1-api-safety`, with both: the two copies are now one, `gb_std::graphics::sprites::pivot`,
which `Sprite::get_pivot` (`gb_std`) and `SpriteManager::get_pivot` (`rust_boy`) call. It computes `sub (16 + y_offset)`
and `sub (8 + x_offset)` modulo 256, so `b` = screen x − `x_offset` and `c` = screen y − `y_offset` wrap around like the
256-pixel background map (positive offsets go left / up, as before: `(0, 1)` is above, `(-1, 0)` is right; see
[B30](#b30)). An offset out of -255..=255 panics (on a 256-pixel map 256 is 0, so it is a mistake). The examples' offsets
(±1) give the same code. It takes the OAM addresses from the new `hw` module (`src/hw.rs`, pure data, the start of the
Phase 2 one: `hw::oam_address`, `OAMA_Y`, `OAM_X_OFFSET`, …; since `refactor-p2-typed-operands` `hw::oam_offset`, the `Expr` being built in `gb_std`). Tests: `test_get_pivot_handles_every_offset` (in both layers,
every offset from -255 to 255 on the test CPU; it failed before, from `get_pivot(-255, 0)`) and
`test_get_pivot_rejects_an_offset_past_the_map`.

#### B23
**Duplicated routines have diverged.** (Lines at `4601a5c`.) `rust_boy`'s `GetTileByPixel` appends `ld a, [hl]`
(`src/engine/functions.rs:307`), the `gb_std` one does not (`src/stdlib/graphics/utility.rs:109-150`): one
label, two contracts. Memcopy, WaitVBlank, WaitNotVBlank and UpdateKeys are also duplicated
(`src/engine/functions.rs:152-266` vs `src/stdlib/graphics/utility.rs`, `src/stdlib/inputs.rs`), and
`src/bin/unbricked.rs` has a third copy. *Fix:* one routine registry.
**Status: fixed** on `refactor-p1-builtins`. There is one `GetTileByPixel`, in `gb_std`
(`src/stdlib/graphics/utility.rs:159`), with the contract of the `rust_boy` copy, which `unbricked_rustboy` and
the `Call` doc example rely on: in, `b` = X and `c` = Y (pixels on the `$9800` map, as `get_pivot` loads them);
out, `hl` = the address of the tile and `a` = the tile index (`[hl]`); it changes `bc` and the flags and keeps
`de` (documented on the function). `BuiltinFunction::generate` (`src/engine/functions.rs:83`, the code of `BuiltinFunction::routine`, `:71`, since `refactor-p2-routines`) returns the
`gb_std` routine for Memcopy, WaitVBlank, WaitNotVBlank, UpdateKeys and GetTileByPixel (the other four were
identical), and `TileManager` copies with `gb_std`'s `cp_in_memory`; only `Delay` was `rust_boy`'s own (it is `gb_std`'s since `refactor-p2-routines`). `gb_std`'s
Memcopy passes its registers typed instead of as strings (same text). Every caller follows the contract:
`unbricked_std` drops its four `ld a, [hl]` after `call GetTileByPixel` (its only change: −4 bytes, +1 in the
routine; the same game in an emulator, 3000 frames compared, see the PR). **Decided by the maintainer (no longer pending):**
`src/bin/unbricked.rs` keeps its own copies of the routines (GetTileByPixel, Memcopy, UpdateKeys, and WaitVBlank at `:22`), and its
`GetTileByPixel` (`:345`) keeps the old contract (`hl` only, no `a`; its callers load `[hl]` themselves). It is the
tutorial written instruction by instruction with `gb_asm` alone (README), a program of its own and not a layer, and
it never links with the library's routines; switching it to `gb_std`'s routine would make it a `gb_std` example. It
is a stated exception to "every routine exists once" (CLAUDE.md, Architecture rules; `basic_usage.rs`, with its own `WaitVBlank`, is the other since `refactor-p2-routines-sprites`); its `Memcopy` is still
the do-while loop, which its four callers use with non-empty data (its own tiles and tilemap). Tests run the routine on `gb_asm::test_cpu`
for 81 pixel positions (`test_get_tile_by_pixel_returns_the_address_and_the_tile`), the `gb_std` callers'
`get_pivot` → `GetTileByPixel` → tile test, and the `unbricked_rustboy` brick handler, which tests `a`
(`IfConst`, `IfA`) and blanks the brick through `hl` (`TileRef`); `test_builtins_are_the_gb_std_routines`
checks that each builtin is the `gb_std` routine. For them the test CPU now models numbers in register pairs
(`ld bc, $9800`, or both halves known), `add hl, rr`, 16-bit `inc`/`dec`, `srl`, `adc`, `or`, and 16-bit
constants (`TestCpu::consts16`).

#### B24
**All user functions are emitted, even unused.** (Lines at `4601a5c`.) `generate_all` (`src/engine/functions.rs:133-147`) emits
every `user_functions` body; `used_user_functions` (`:71`) is written but never read. Note: filtering on it
today would break linking, because calls made through `Call`/`IfCall` are not tracked ([B26](#b26)) — fix B26 first.
**Status: fixed** on `refactor-p1-builtins`, with B26: `FunctionRegistry::generate_used`
(`src/engine/functions.rs:361` since `refactor-p2-routines`) emits only the user functions the program refers to, from the start-up code,
the main loop, `raw()` code or the animation functions, then from those functions, and so on; in registration
order. A function called only from raw code (`raw()`, or an `Asm::raw` line, of one or several lines) needs
nothing. To emit one that only code `build()` does not
see calls (asm appended to its output, an `INCLUDE`d file), **`RustBoy::keep_function(name)`**
(`src/engine/rustboy.rs:326`; it takes a builtin name too; it panicked on an unknown name, like `call`: since
`refactor-p2-engine-api-errors` the name is looked up by `build()`, which returns `Error::UnknownFunction`);
`use_function(BuiltinFunction)` still forces a builtin. `used_user_functions` is gone. A function is found by
its label, so `define_function(name, body)` panics if `body` does not define the label `name` (a body labelled
otherwise used to be emitted anyway and could be called by its own label); another global label in a body (a
second entry point) also finds the function. `define_function` and `define_function_from` panic on a name that
is not an RGBDS identifier. `build()` panics (since `refactor-p2-engine-api-errors`, returns
`Error::NameConflict`) if a user function's name is also defined elsewhere in the program: a variable, a constant or label of its code (`define_const`, a `DEF`, a label in raw code), or an external
symbol (a function is defined either with `define_function`, or outside with `external_symbol`, not both); such a
function used to be dropped silently, and `call Jump` reached the constant or the variable (on `refactor`, rgbasm
reported the name defined twice). A user function that defines a builtin's name, as its own name or as a second
entry point (a routine bundle with an `UpdateKeys:` inside), always replaces the builtin, also when `use_function`
forces the builtin; the builtin's variables are then not created. `keep_function` on a function `build()`
generates (an animation) does nothing, since it is always emitted (before `refactor-p2-engine-api-errors`, its name was
unknown until a first `build()` had registered it). No example changes (every example function is used, under its own name). Tests:
`test_only_used_user_functions_are_emitted` (unused, an unused cycle, recursive, through another function, by
address, by `jp`, from raw code, from a raw line after a comment and after a `;` in a string),
`test_keep_function_emits_a_function_nothing_calls`, `test_keep_function_needs_a_function`,
`test_define_function_needs_its_label`, `test_function_name_must_be_an_identifier`,
`test_a_second_entry_point_of_a_function_is_found`, `test_a_function_named_like_a_constant_is_an_error` (and
`_a_variable_`, `_a_raw_label_or_def_`, `test_a_function_cannot_be_external`; they were `…_panics`),
`test_a_user_function_replaces_a_forced_builtin`, `test_a_second_entry_point_replaces_a_builtin`,
`test_keep_function_accepts_a_generated_function`, `test_redefining_a_function_moves_its_second_entry_point`.

#### B25
**Animation labels are not namespaced by sprite.** `Anim_{name}` and `.skip_{name}`
(`src/engine/sprites.rs:632, 685-686`; `:744, 797-798` at `7b54900`): two sprites that both have a `"Spin"`
animation, or two composites with `"Walk"` (each half got `Walk_0` / `Walk_1`, `:386`), emit duplicate labels.
Names with `-` or spaces produce invalid labels. *Fix:* prefix with the sprite; validate names.
**Status: fixed** on `refactor-p1-unique-labels`: the function is `Anim_{sprite}_{animation}` and the
dispatcher label `.skip_{sprite}_{animation}`; a composite gives each of its sprites the animation name
itself (`Anim_player_left_Walk`, `Anim_player_right_Walk`; the `_0`/`_1` suffix is gone). Sprite, composite
and animation names must be valid RGBDS identifiers (a letter or `_`, then letters, digits, `_#$@`), sprite
names must be unique, and so must animation names per sprite and per composite; a name that would give an
existing animation label (`"Big_Coin"` + `"Spin"` vs `"Big"` + `"Coin_Spin"`) is rejected too. Each case
panics with a message naming the sprite and the animation. Not checked, left to the Phase 3 item "Validate
user-supplied symbol names":
- RGBDS keywords (a sprite called `a`: its tile label `a:` fails);
- a valid, unique sprite name that clashes with another global label: the tile labels `{name}` and
  `{name}End` (sprites `"Coin"` and `"CoinEnd"` both define `CoinEnd`), or a fixed label such as `Main`,
  `EntryPoint`, a routine (`Memcopy`) or a variable (both cases confirmed with rgbasm);
- the other user names (tiles, variables, functions, constants).

#### B26
**Builtins reached through `Call`/`IfCall` are not auto-included.** (Lines at `4601a5c`.) `Call::emit`
(`src/stdlib/flow/emittable.rs:48`) and `IfCall::emit` (`src/stdlib/flow/flow_if.rs:903-943`) just emit
`call X`; only `RustBoy::call`/`call_args`/`use_function` register builtins (`src/engine/functions.rs:101-115`),
and `define_function_from`/`add_to_main_loop` (`src/engine/rustboy.rs:235, 457`) don't scan bodies. Following
the `Call` doc example alone (`Call::with_args("GetTileByPixel", ..)`) → `call GetTileByPixel` with no routine
→ rgblink "undefined symbol". `unbricked_rustboy` works only because it also calls `gb.call_args("GetTileByPixel", ..)`.
*Fix:* routines as values with dependencies, or scan emitted `Call` targets in `build()`.
**Status: fixed** on `refactor-p1-builtins` by scanning in `build()` (routines as values stay in Phase 2). The
Functions chunk is worked out once all the code is known (`src/engine/rustboy.rs:788-824` since `refactor-p2-engine-api-prelude`): the global
symbols of every other chunk and of the animation functions are looked up (`symbols`, since `refactor-p2-routines`
`src/asm/labels.rs:440`; it read the text of every instruction, it now reads typed operands by their type and
reads the text of each raw instruction line by line as RGBDS does, with
`gb_asm::labels::code_lines`: `;` and `/* … */` comments (also over several lines) and the contents of strings
are skipped, a line ending with `\` continues on the next, a line starting with `Name:` defines `Name`; so
`call`, `jp`, `ld hl, Name`, `dw Name`, `LOW(Name)` and raw lines of one or several lines count, and sections
and file names do not; triple-quoted strings and macros are not handled); each builtin or user function found is
emitted, and its body scanned the same way. Each one is emitted once, builtins in `BuiltinFunction` order then
user functions in registration order; a user function with the name of a builtin replaces it. A name defined in
the code `build()` generates is not taken for a builtin (for a user function it is an error, see [B24](#b24)): a label (its own copy of a routine in `raw()` code), a
`DEF` (`define_const("Delay", 5)`, or a `DEF`/`REDEF` line of raw text in any form: `EQU`, `=`, `+=`, `EQUS`,
`RB`…, through `gb_asm::labels::split_def`, which `label_check` uses too) or a variable (`create_u8("Delay", 0)`);
before, each of these also emitted the builtin `Delay:`, which rgbasm rejected as defined twice. Names defined
where `build()` cannot see are not known: an `INCLUDE`d file (not read: its path depends on the assembler's
include directories), a macro, a symbol made by `EQUS` interpolation; a program declares those with
**`RustBoy::external_symbol(name)`** (`src/engine/rustboy.rs:361`): a function of that name is never emitted,
nor the variables of a builtin of that name. Then the variables the emitted builtins need
(`BuiltinFunction::variables`: `wCurKeys`, `wNewKeys` for `UpdateKeys`) are created, unless the program already
defines them (as variables, of any type, in raw code, or as external symbols), before the variable initialisation
and the Data chunk are emitted (`:743-763`), so `UpdateKeys` called without `add_inputs` links too. The scan is
linear: each user function body is read once, when it is registered, and maps (name → function, and each other
global label of a body → its function, kept up to date as functions are defined) find a function, also for
`call` and `keep_function`; each name is handled once (a 100-function, 5000-line program builds in about 6 ms in
release; `test_a_large_program_builds_quickly`). The animation functions are registered as generated names (known
to `call`), not as user functions. `build()` no longer registers
WaitVBlank, WaitNotVBlank, Memcopy and UpdateKeys itself, and `call`/`call_args` only check the name. No example
ROM changes because of it. Tests: `test_builtins_are_emitted_whatever_calls_them` (`Call`, `IfCall`, a
`define_function_from` body, a `define_function` body, a function called by a function, a function only `init()`
calls, `raw()` code, a multi-line raw instruction with a comment, a `db` with a `;` in a string),
`test_a_builtin_called_from_everywhere_is_emitted_once`, `test_a_routine_is_never_emitted_twice`,
`test_names_the_program_defines_are_not_functions` (a constant, a variable, the `UpdateKeys` variables in raw code
or as a `u16`, a call in a block comment), `test_a_def_in_raw_code_is_not_a_function`,
`test_external_symbols_are_not_emitted` (an `INCLUDE`d `UpdateKeys`, `Delay`, `Memcopy`, linked with the file),
`test_a_large_program_builds_quickly`, `test_get_tile_by_pixel_callers_follow_its_contract`. They check that the program links with
`gb_asm::label_check::assert_links`: no label error, and no symbol used but not defined (`undefined_symbols`:
labels, variables, constants; `hardware.inc` names allowed; `assert_links_with` adds included files). With `RGBDS_LINK_CHECK` set it also assembles and
links each program with rgbasm/rgblink; CI's `assemble` job runs `cargo test --lib` that way.
**Since Phase 2 (`refactor-p2-routines`), routines as values:** every function is a `Routine` with explicit
dependencies (see [Key concepts](#key-concepts)). A builtin's dependencies are given in full and its body is no longer
read (a test checks them against the body); a user function needs its dependencies and what its body refers to;
`RustBoy::define_routine` and `call_routine` register a routine with its dependencies, so a typed call brings what it
calls even when nothing names it (a jump table in an `INCLUDE`d file). The scan stays, as the reading of raw text:
`symbols` moved to `gb_asm::labels` and reads typed operands by type. The variables of every emitted routine are
created (`Routine::variables`; they were the builtins' only). Every behaviour above is unchanged and its tests pass
(`keep_function`, `external_symbol`, `use_function`, a user function replacing a builtin, second entry points, the
name conflicts, `wCurKeys`/`wNewKeys`, the order). New tests: `test_a_typed_call_brings_the_routine_and_its_dependencies`
(each routine once, in a fixed order, linked), `test_a_dependency_nothing_names_is_emitted`,
`test_a_routine_and_a_function_of_one_name`, `test_define_routine_replaces_a_builtin_with_its_variables`, and in
`functions.rs` `test_a_routine_brings_its_dependencies`, `test_a_dependency_is_shared_not_replaced`,
`test_two_routines_with_one_name_panic`, `test_a_routine_variable_is_created_once`,
`test_builtin_dependencies_are_complete`.

#### B27
**`Memcopy` with length 0 copies 64 KiB.** (Lines at `4601a5c`.) Memcopy is a do-while loop (`src/stdlib/graphics/utility.rs:54-70`;
`src/engine/functions.rs:152-170`); `BC = 0` wraps to `$FFFF` and overwrites WRAM, the stack, I/O and IE.
Reachable through `cp_in_memory` (`src/stdlib/graphics/utility.rs:45`) or `generate_memcopy_calls`
(`src/engine/tiles.rs:252`) with an empty tile set / empty `.2bpp`. *Fix:* skip empty blobs at generation
time, or test `BC` before the first copy.
**Status: fixed** on `refactor-p1-builtins` at generation time: `TileManager::generate_memcopy_calls`
(`src/engine/tiles.rs:407`) skips an empty blob of raw data (`from_raw` with no tiles, a tilemap with no rows).
Its labels are still emitted, with nothing between them, so code that names them still links; with no copy left,
`Memcopy` is not emitted at all (B26). A file blob (`INCBIN`) is always copied whole, as before: its size is only
known once assembled, so `TileSource::from_file(path, 0)` now panics (a user error) instead of being taken for an
empty blob, and so does adding a `TileSource::File(path, 0)` built directly (`add_sprite`, `add_background`). `Memcopy` itself is unchanged, so no ROM changes. Left open then (fixed since, below): `gb_std`'s `cp_in_memory` only knows labels and cannot see an empty blob, and an empty
`.2bpp` file was not checked; testing `BC` in `Memcopy` covers these, at 3 bytes and a few cycles per call. Tests: `test_empty_blobs_are_not_copied` runs
the start-up code with the real `Memcopy` on `gb_asm::test_cpu` (blob lengths from `TestCpu::consts16`): before, the
first empty blob made it copy past its data; `test_a_tile_file_needs_tiles`,
`test_a_tile_file_built_directly_needs_tiles`.
**Then fixed in the routine too** on `refactor-p1-api-safety` (decided by the maintainer): `Memcopy`
(`src/stdlib/graphics/utility.rs`, `memcopy`) starts with `ld a, b` / `or c` / `ret z`, so a length of 0 copies
nothing, whoever calls it: `gb_std`'s `cp_in_memory` on an empty blob, an empty `.2bpp` file. The copy loop
jumps back to a local `.copy` after the test. It costs 3 bytes once (in every example that copies tiles) and 4
M-cycles per call. `RustBoy` still skips its empty raw blobs (no code for nothing). Still not checked: a `.2bpp` file
shorter than the tile count given to `from_file` (its size is only known when assembled). Test
`test_memcopy_with_length_0_copies_nothing` runs the routine on `gb_asm::test_cpu` with `bc` = 0 (before: it copied
past the data) and with `bc` = 1 to 4.

#### B28
**OBP1 never initialised; OAM not cleared without sprites.** (Lines at `4601a5c`.) Only `rBGP` and `rOBP0` are written
(`src/engine/rustboy.rs:316-320`), so sprites with the OBP1 palette flag get a random palette on DMG. OAM is
cleared only `if !self.sprites.is_empty()` (`:292`), yet `LCDCF_OBJON` is always set (`:313`) → a
background-only program shows garbage objects on real hardware.
**Status: fixed** on `refactor-p1-init-order`: `build()` always clears the 160 OAM bytes, with or without
sprites, using `gb_std`'s `initialize_objects_screen` + `clear_objects_screen` (`rust_boy` had its own copy of
that loop; it now only writes the initial sprites), and sets `rBGP`, `rOBP0` **and `rOBP1`** to `%11100100`
(colour i is shade i, so a sprite with the OBP1 flag looks like one with OBP0 until the program changes it),
before LCD on. Tests: `test_oam_is_cleared_without_sprites`, `test_oam_is_cleared_before_the_sprites_are_written`,
`test_every_palette_is_set`. A palette API stays in Phase 3.

#### B29
**Documentation errors in code and README.**
- `src/stdlib/inputs.rs:54` says `wCurKeys` "0 = pressed"; after the `xor` it is 1 = pressed.
- `RustBoy::call` doc example `gb.add_to_main_loop(gb.call("X"))` (`src/engine/rustboy.rs:202, 206` at `4601a5c`; the corrected example is at `:435-445`) does
  not compile (E0499, two `&mut` borrows); hidden by ```` ```ignore ````.
- README: "Type-safe … compile-time guarantees" (`:12`) is an overclaim; "Complete support for … instruction
  set" (`:19`) — `push/pop/halt/di/ei/reti/sbc/bit/set/res/rl/rr/sla/sra/cpl/nop/scf/ccf/rst` are missing;
  "two example programs" (`:86`) — there are 6; the project tree omits `src/engine/`; `rgbasm -L` (`:146`)
  no longer exists; "Current branch: `gbz80-std`" (`:153`) is stale; Basic Example lacks padding ([B21](#b21)).
- `If*` comparisons are **unsigned** (native `cp` semantics) — correct, but worth documenting next to the
  `i8` variable API.

**Status: fixed** on `refactor-p0-readme` (README rewritten and used as the crate docs, so its examples are
compiled; `inputs.rs` comment and `RustBoy::call` example corrected; the `If` docs say "unsigned" since B5).

#### B30
**The `documentations` branch contradicts the code** (its `src/` is identical to `main`, so these are doc bugs):
- `docs/sprite-movement.md:79-81`: `get_pivot(ball, 0, 1)` is "1 px below" and `(-1, 0)` "1 px left", and it
  "loads the tilemap address" — actually `(0, 1)` is **above**, `(-1, 0)` is **right**, and it only sets `b`/`c`.
- `docs/sprite-movement.md:28-31, 40-43, 123`: `move_left(player, 2)` (no such method in `rust_boy`); limits
  described in screen pixels, but they are raw OAM coordinates (+8 / +16).
- `docs/button-actions.md:40-43`: uses `gb.sprites.move_left(player, 1)` (does not exist in `rust_boy`);
  `:105` says `check_key` reads `wNewKeys` (newly pressed) — it reads `wCurKeys` (**held**).
- `docs/animations.md`: string-based `enable_animation("CoinSpin")`, working `PingPong`/`Once` ([B10](#b10)),
  wrong default delay.
- `docs/graphics.md:106`: "`build()` inserts … OAM DMA automatically" — there is no OAM DMA ([B12](#b12)).
- `docs/variables.md:99`: HRAM variables via `create_in_section` "from `gb_std`" — it lives in `rust_boy` and
  always emits `WRAM0` (HRAM variables exist since Phase 2, `refactor-p2-engine-api-memory`: `create_hram_*`).
- Since [B8](#b8) a sprite **limit is included**: `docs/sprite-movement.md:40-43, 105` and
  `docs/api-levels.md:93-94` use `15` / `105` (and `145`), the Unbricked values from when the limit was the
  first position the sprite could not reach. With the same values a sprite now goes one pixel further on each
  side: with a step of 1 it stops on OAM X 15 and 105 (before: 16 and 104). Write `16` / `104` (as the
  Unbricked examples now do) to keep the old stop positions.
- Since [B7](#b7), `gb_std` `check_key` and `Sprite::move_*_limit` take a `&LabelAllocator` first:
  `docs/api-levels.md:65`, `docs/button-actions.md:97-98` and `docs/sprite-movement.md:105` call them without it.

---

## Checked and refuted

Claims that were investigated and **rejected**, kept here so nobody re-investigates them:

| Claim | Why rejected |
|---|---|
| `is_specific_tile` panics on an empty slice (`tiles_ids.len() - 1`) | The subtraction is inside the `for` body, which never runs for an empty slice. |
| `Functions`/`Tiles` chunks have no `SECTION` and land in the wrong section | Only reachable via `raw()` opening a section → covered by [B15](#b15). Sections are first-class since Phase 2 (`refactor-p2-sections`): code that `raw()` puts after a RAM section now panics at `build()`. |
| `Asm::ds` always emits a fill byte, so RAM can't be reserved | True, but a missing feature, not a bug. Since Phase 2 (`refactor-p2-sections`) `ds(n)` reserves without a fill and `ds_fill(n, fill)` fills. |
| Everything in one `ROM0[$100]` section fails above 16 KB | Tile data is copied to VRAM anyway; ROM banking is a feature (Phase 3: MBC). |
| `If` comparisons are unsigned while `i8` vars exist | Native `cp` semantics, documented in `flow_if.rs:8-15`; only a doc note ([B29](#b29)). |
| `enable_animation` on a sprite without animations references an undefined variable | API misuse; covered by "fail loudly" in [B20](#b20) (it panics since). |
| 8×16 tile indices not aligned after an odd-sized 8×8 sprite | Only happens when mixing sizes; since [B4](#b4) the size is set once, before the first sprite. |
| `0x05` constants require RGBDS ≥ 0.9 | The project toolchain is RGBDS 1.0; handled by pinning the version in CI. |
| `TileSource::from_file` tile count not checked against the file | Caller error; became a feature (derive the count from the file size). |

---

## 5. Missing features (summary)

Detailed list in [`Task.md`](Task.md) Phase 3. Biggest gaps:

- **Audio: nothing at all** (no APU registers, no sound effects, no music driver).
- **Graphics:** no shadow OAM/DMA, no scrolling, no window layer, no palette API/fades, no
  metasprites beyond 16×16, no text/numbers (a `$9C00` map exists since [B19](#b19), without a window layer).
- **Animation:** global speed only; no events (`Loop`, `PingPong` and `Once` work since [B10](#b10)).
- **Engine:** polling instead of VBlank interrupt + `halt`; no interrupts/timers, scenes, RNG, collision,
  16-bit math, loops/switch.
- **ISA:** complete since Phase 2 (`refactor-p2-isa`; `push/pop`, `halt`, `di/ei`, `reti`, `sbc`, `bit/set/res`,
  rotates/shifts, `cpl`, `ld [hl-]`… were missing); the engine does not use the new instructions yet.
- **Platform:** single ROM0 bank, no SRAM saves, no GBC.
- **Tooling:** CI exists now (fmt, clippy, tests, assembling every example), and snapshot tests of the examples'
  generated asm (Phase 3, `refactor-p3-tooling`) and headless-emulator tests (`refactor-p3-tooling-emulator`);
  still missing: a one-command "build ROM and run".

---

## 6. Branch analysis

Compared with `origin/main` (`git rev-list --count`):

| Branch | Last commit | Ahead / behind | Content | Verdict |
|---|---|---|---|---|
| `documentations` | `8fe484d` | 1 / 0 | 7 docs (`api-levels`, `animations`, `button-actions`, `control-flow`, `graphics`, `sprite-movement`, `variables`), 892 lines | **Keep** — good material; fix [B30](#b30), then fast-forward merge |
| `unbricked-example` | `a7167ea` | 3 / 57 | Early experiment using the external `retroshield-z80-workbench` crate (binary output); `originals/` identical to `main` | **Delete** (superseded); optionally tag `archive/unbricked-example` first |
| `fosdem-example` | `4601a5c` | 0 / 0 | Same commit as `main` | **Delete** |
| `test-animation` | `3e471a4` | 0 / 11 | Fully merged | **Delete** |
| `rust-boy-implementation` | `41d3eb1` | 0 / 12 | Fully merged (tag `v0.2.0-poc` marks it) | **Delete** |
| `gbz80-std` | `fc63483` | 0 / 45 | Fully merged | **Delete** |
| `gbz80-workbench-more-idiomatic` | `a9a4381` | 0 / 51 | Fully merged | **Delete** |
| `gbz80-workbench` | `ae304a1` | 0 / 57 | Fully merged | **Delete** |

Tags `v0.1.0-poc` (`032f8ce`) and `v0.2.0-poc` (`41d3eb1`) preserve the historical milestones.

**Status (2026-10-07):** the 7 *Delete* branches were deleted by the maintainer. To restore one:
`git push origin <last commit>:refs/heads/<branch>`; only `unbricked-example` had commits that are not in
`main`, so it can only be restored while GitHub still keeps those commits.
