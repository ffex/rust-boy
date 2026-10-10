# Changelog

## Unreleased: the refactoring (`refactor` → `main`)

The refactoring of [Task.md](Task.md) Phases 0 to 2, released together: every breaking change of the
refactoring is in this release, so a program migrates once. The reasons are in
[CONTEXT.md](CONTEXT.md): the levels assessment (§3) and the bug catalogue (§4, B1–B30).

### Highlights

- **New module layout**: `rust_boy::asm` (instructions), `rust_boy::stdlib` (routines, control flow),
  `rust_boy::engine` (`RustBoy`), `rust_boy::hw` (hardware facts), and `rust_boy::prelude`.
  `rust_boy::rust_boy` is gone.
- **`RustBoy::build(&self) -> Result<String, Error>`**:
  - a problem of the program as a whole is an `Err`, and a wrong call panics where it is made;
  - `build()` changes nothing, so building twice gives the same text.
- **`RustBoyConfig`**: the sprite size, the background tilemap, the palettes, the LCDC flags, the forced
  builtins and the animation delay, in one value.
- **Memory**:
  - HRAM variables, accessed with `ldh`;
  - every variable section at the address the allocator gives it, so `get_address` is the linked address;
  - VRAM tiles, WRAM0, HRAM and OAM are all allocated and bounded.
- **A typed assembly layer**:
  - the whole SM83 instruction set, with typed operands (`R8`, `R16`, `Mem`, ...) and `Expr` values;
  - typed sections;
  - one label allocator, with `jr` → `jp` relaxation.
- **Routines as values** (`stdlib::routine::Routine`), with their dependencies and calling convention
  (registers read, returned, clobbered). Each routine exists once in the library.
- **Bug fixes**:
  - B3: duplicate labels;
  - B4: 8x8 sprites drawn as 8x16;
  - B5: `If` compared its operands the wrong way round;
  - B6: 16x16 sprites split at the screen edges;
  - B7, B25: label collisions;
  - B8: sprite limits overshot;
  - B9: `jr` out of range;
  - B10: `PingPong` / `Once` played as `Loop`;
  - B11: `init()` code overwritten;
  - B13: non-deterministic output;
  - B14–B28: more fixes, listed in CONTEXT.md §4.
- **Tooling**:
  - CI builds and tests on stable and on Rust 1.85, and assembles every example with RGBDS 1.0.4;
  - tests run the generated code on a model of the CPU, and link whole programs with RGBDS.

### Toolchain

- RGBDS 0.9 or later (the generated code writes `0x05`-style numbers). RGBDS 1.0.4 is tested, and
  `rgbasm -L` no longer exists.
- `hardware.inc` (v4.x) is `include/hardware.inc`, given with `rgbasm -I include`. It used to be copied into
  each example.
- Rust 1.85 or later (`rust-version`, edition 2024).

## Migration guide

Each table maps the code of the previous release ("Before") to this one ("After"). "Before" uses the old module
names: `gb_asm`, `gb_std`, `rust_boy::rust_boy`.

### 1. Modules and imports

| Before | After |
|---|---|
| `use rust_boy::gb_asm::{Asm, ...}` | `use rust_boy::asm::{Asm, ...}` |
| `rust_boy::gb_asm::asm::Asm` (the inner module) | `rust_boy::asm::Asm` (the inner module is `asm::program`) |
| `use rust_boy::gb_std::flow::If` (any `gb_std::` path) | `use rust_boy::stdlib::flow::If` |
| `use rust_boy::rust_boy::{RustBoy, ...}` | `use rust_boy::engine::{RustBoy, ...}`, or `use rust_boy::prelude::*` |
| `use rust_boy::gb_asm::Chunk` | `use rust_boy::engine::Chunk` (the game layout is the engine's: `Chunk`, `Layout`) |
| (none) | `use rust_boy::prelude::*`: `RustBoy`, `RustBoyConfig`, `Error`, `SpriteSize`, `TileSource`, `TilemapArea`, `InputManager`, `AnimationType`, `Var`, `VarType`, `SpriteId`, `CompositeSpriteId`, `ANIM_DISABLED`, `BuiltinFunction`, `Chunk`, `Layout`, `Lcdc`, `Palettes`, `If`, `IfA`, `IfConst`, `IfCall`, `Call`, `Routine`, `Regs`, `PadButton`, `Asm`, `Block`, `Emittable`, `Expr`, `Instr`, `R8`, `R16`, `Mem`, `Condition`, `Section`, and `hw` |

### 2. Building a program (`engine::RustBoy`)

| Before | After |
|---|---|
| `let out = gb.build();` (`String`, `build(&mut self)`) | `let out = gb.build()?;` (`Result<String, engine::Error>`, `build(&self)`). In `main`: `fn main() -> Result<(), Error>` and `println!("{}", gb.build()?)` |
| `build()` panicked on a user function that is also a variable, a constant / `DEF` / raw label, or an external symbol | `Err(Error::NameConflict { name, first: Definition::Function, second })` |
| `build()` panicked when `wFrameCounter` or `wAnim_*` existed with another type than `u8` | `Err(Error::NameConflict { .., second: Definition::GeneratedVariable(VarType::U8) })` |
| code or data in a RAM section, or a section name used twice: rgbasm failed, then `build()` panicked | `Err(Error::Section(message))` |
| `create_*` panicked when WRAM0 was full | `build()` returns `Err(Error::MemoryFull { region, what, needed, available })` |
| `gb.call("X")`, `call_args`, `keep_function("X")` panicked when `X` was not (yet) a function | `X` may be defined after the call; `build()` returns `Err(Error::UnknownFunction { name, available })` if it is still not one |
| `call` / `keep_function` of an animation function worked only after a first `build()` | they work at any time; `function_exists` knows animation functions |
| a variable or tile name that is a register or keyword (`"a"`, `"LOW"`), or not an identifier, failed in `build()` or in rgbasm | panics in `create_*` / `tiles.add_*` / `add_sprite` |
| an `Instr` built by hand that `Instr::check` rejects, given to `init`, `add_to_main_loop`, `call_args`, `define_function`, `define_routine`, `call_routine` or an input action, panicked in `build()` | panics at that call |
| `gb.set_sprite_size(..)`, `set_background_tilemap`, `set_animation_delay`, `use_function` | unchanged, or all at once: `RustBoy::with_config(RustBoyConfig::default().sprite_size(..).palettes(..))`; `gb.config()` reads them; `set_palettes(Palettes)` is new |
| `rBGP` / `rOBP0` / `rOBP1` were always `%11100100`, and LCDC always had `BGON` and `OBJON` | the defaults of `RustBoyConfig::palettes` and `RustBoyConfig::lcdc`, which can change them |
| `SECTION "Variables", WRAM0` (floating) | `SECTION "Variables", WRAM0[$C000]`: each variable section is at its address. A `raw()` section fixed where the variables are now overlaps them; let it float |
| `get_address`: creation order across sections | section by section, in first-use order: the address the variable has when linked |
| no HRAM variables | `gb.vars.create_hram_u8("hSpeed", 0)` (also `u16`, `i8`, `i16`): `Var::set` / `get` use `ldh`; HRAM gives variables $FF80-$FFBF, and the stack keeps the rest |
| `MemoryRegion` matched exhaustively | `MemoryRegion` is `#[non_exhaustive]`, and it has `SpriteTiles`, `BackgroundTiles`, `Wram0`, `Hram`: add a `_` arm |

### 3. Engine behaviour (Phase 1 fixes that change what a program does)

| Before | After |
|---|---|
| every sprite was 8x16 (`LCDCF_OBJ16` forced) | 8x8 by default. For 8x16, call `set_sprite_size(SpriteSize::Size8x16)` (or use the config) **before** adding sprites. In 8x16 mode an odd tile count, an odd `frame_step`, or a sprite starting on an odd tile panics. `add_sprite_16x16` needs 8x16 (B4, B18) |
| `If::lt(l, r)` was true when `r < l` (and so for every two-operand comparison) | `l < r`, as documented. If you swapped your arguments to work around this, swap them back (B5). Left code that may change `b` is wrapped in `push bc` / `pop bc` |
| a sprite limit (`move_*_limit`) was the first position it could not reach, and it stopped only on exact equality | the limit is included and the move clamps to it: write the old limit minus one step towards the start (Unbricked: `15`/`105` → `16`/`104`) (B8) |
| a composite (16x16) move tested each half on its own | the leading sprite is tested, and the others follow at their offsets (B6) |
| `AnimationType::PingPong` / `Once` played as `Loop` | they work. `PingPong` adds a variable, `wAnim_{sprite}_Dir` (B10) |
| `init()` code ran before the variables and palettes were set, so they overwrote it | it runs after them, and before the LCD is turned on. `rLCDC` is still set after it (B11) |
| `raw()` code outside `Chunk::Main` was dropped | every chunk is kept, after the code generated for it. `Init` and `MainLoop` raw code runs; `Data` raw code without a section gets `SECTION "Raw Data", WRAM0` (B15) |
| `Var::set(value: i8)` wrote one byte; `Var::get` loaded one byte | `set(impl Into<i32>)` writes both bytes of a 16-bit variable and panics on a value out of range: `u8var.set(200u8 as i8)` panics, so write `set(200)`. `get` loads a 16-bit variable into `hl` (B16) |
| creating an existing variable created a second label | it returns the existing variable; another type panics (B3) |
| more than 40 sprites, sprite tiles past $8FFF, background tiles past $97FF, or a position out of OAM range was silently wrong | panics with what does not fit. A sprite's frames from several files: `RustBoy::add_sprite_tiles` (B17) |
| `gb.sprites.add(..)` (public), or tiles counted by both managers | `RustBoy::add_sprite` is the only way to add a sprite (`SpriteManager::add` is `pub(crate)`) (B18) |
| every tilemap went to `$9800` | `tiles.add_tilemap_at(name, TilemapArea::Map9C00, rows)`, `RustBoy::set_background_tilemap`. A second tilemap on one map, or more than 32 rows, panics (B19) |
| an unknown sprite / composite id or animation name gave no code | panics with the name. `set_initial_animation(id, i)` must come after `add_animation` (B20) |
| `get_pivot` clamped out-of-range offsets to 0 | it wraps like the 256-pixel map; an offset beyond ±255 panics (B22) |
| `GetTileByPixel` had two contracts | one: in `b` = X, `c` = Y; out `hl` = the tile's address and `a` = the tile. Drop the `ld a, [hl]` after the call (B23) |
| every user function was emitted | only the ones the program uses, transitively. Force one with `keep_function(name)`. `define_function(name, body)` panics unless `body` defines `name:`, and a function name must be an identifier (B24) |
| a builtin called through `Call`, `IfCall` or a function body was missing at link time | every function the code refers to is emitted. Declare a routine defined in an `INCLUDE`d file with `external_symbol(name)` (B26) |
| animation labels `Anim_{animation}`; any sprite or animation name | `Anim_{sprite}_{animation}`. Sprite, composite and animation names must be unique RGBDS identifiers (B25) |
| `Memcopy` with a length of 0 copied 64 KiB; `TileSource::from_file(path, 0)` was accepted | `Memcopy` copies nothing for 0, and `from_file(path, 0)` panics (B27) |
| the OAM was cleared only when sprites existed; `rOBP1` was never set | the OAM is always cleared, and `rOBP1` = `%11100100` like the others (B28) |

### 4. Routines and control flow (`stdlib`)

| Before | After |
|---|---|
| `memcopy()`, `wait_vblank()`, `wait_not_vblank()`, `update_keys()`, `get_tile_by_pixel()`, `is_specific_tile(..)` returned `Vec<Instr>` | they return a `Routine`. `asm.emit_all(memcopy())` and `code.extend(memcopy())` work as before; `memcopy().body()` or `Vec::<Instr>::from(memcopy())` gives the instructions |
| `gb.define_function("IsWallTile", is_specific_tile("IsWallTile", ..))` | `gb.define_routine(is_specific_tile("IsWallTile", ..))`, which keeps the routine's dependencies and calling convention. `define_function` still takes a `Vec<Instr>` |
| `BuiltinFunction::variables()` returned `&'static [&'static str]` | it returns `Vec<String>` |
| `Delay` was the engine's own (private) | `stdlib::utility::delay()` |
| `gb_std::graphics::sprites::SpriteManager` (`sm.add_sprite(x, y, tile, flags)`, `sm.draw()`, `sm.get_sprite(i)`) | `Sprite::new(oam_entry, x, y, tile, flags)` values and `draw_sprites([&a, &b])`; call the sprite's own methods (`ball.get_pivot(..)`) |
| `gb_std` `Sprite::new(40, ..)` (any id) | panics: the id is an OAM entry, 0 to 39 |
| `check_key(button, code)`, `sprite.move_left_limit(..)` | `check_key(labels, button, code)` and `sprite.move_left_limit(labels, ..)`, where `labels` is `asm.labels()` or `gb.labels()` (B7) |
| `clear_objects_screen()` | `clear_objects_screen(asm.labels())` |
| `impl Emittable for X { fn emit(&mut self, counter: &mut usize) .. }` | `fn emit(&mut self, labels: &LabelAllocator)`; take labels with `labels.local(stem)` / `labels.locals([..])`. `Emittable` lives in `asm` (re-exported by `stdlib::flow`) |
| `let mut counter = 0; asm.emit_all(code.emit(&mut counter))` | `asm.emit_code(code)` |
| `gb.next_if_counter()` | `gb.next_label_counter()` or `gb.labels().local(stem)` |
| `VariableSection::new("Vars", "WRAM0")` (fields `name`, `memory`) | `VariableSection::new(Section::wram0("Vars"))` (field `section`) |
| `IfA::eq("X + 1", ..)`, `IfConst::lt(.., "SCRN_X - 8", ..)`, `TileRef::load_address_label("_SCRN0 + 32")`, `cp_in_memory("Tiles", "_VRAM + 16")` | an `Expr`: `Expr::sym("X") + 1`, `Expr::sym("SCRN_X") - 8`, ... (or `Expr::raw(..)`). A `&str` must be a symbol or a number, and other text panics |
| `IfA::eq("LOW(X)", ..)`, `IfA::eq("'A'", ..)` | `IfA::eq(Expr::low("X"), ..)`, `IfA::eq(Expr::raw("'A'"), ..)` |
| `is_specific_tile(.., &["BRICK+1"])` | each id is a symbol or a number: define `DEF BRICK_2 EQU BRICK + 1` |

### 5. Assembly (`asm`)

| Before | After |
|---|---|
| `ld_a_label("X")`, `ld_hl_label("X")`, `ld_b_label("a")` | `ld(R8::A, "X")`, `ld(R16::HL, "X")`, `ld(R8::B, R8::A)` |
| `ld_hli_label("a")`, `ld_addr_label_a("[hl]")` | `ld(Mem::Hli, R8::A)`, `ld(R8::AtHl, R8::A)` |
| `ld_a_addr_reg(Register::DE)` / `(Register::HL)` | `ld(R8::A, Mem::De)` / `ld(R8::A, R8::AtHl)` |
| `ldh_label("[$FF40]", "a")` | `ldh(Mem::addr(Expr::hex(0xFF40)), R8::A)`, or `ldh(Mem::addr(hw::LCDC), R8::A)` |
| `add_label("a", "b")`, `add_label("hl", "bc")`, `add_label("sp", "-2")` | `add(R8::B)`, `add_hl(R16::BC)`, `add_sp(-2)` |
| `sub_label("a", "8")`, `cp_label("BRICK")`, `and_label("%11110000")` | `sub(8)`, `cp("BRICK")`, `and(Expr::bin(0b11110000))` |
| `inc_label("de")`, `dec_label("b")`, `srl_label("a")`, `swap_label("a")` | `inc(R16::DE)`, `dec(R8::B)`, `srl(R8::A)`, `swap(R8::A)` |
| `Operand::Reg(Register::A)`, `Operand::Imm(5)`, `Operand::Imm16(n)` | `R8::A`, `5`, `Operand::from(n)` (or just `n`) |
| `Operand::AddrDef("x")`, `Operand::AddrReg(Register::HL)`, `Operand::AddrRegInc(..)` | `Mem::addr("x")`, `R8::AtHl`, `Mem::Hli` |
| `Operand::Label("TilesEnd - Tiles")` | `Expr::sym("TilesEnd") - "Tiles"` (or `Expr::raw(..)`) |
| `ld 1, 2`, `inc 5`, `add a, hl`, `cp a, [wCount]` compiled and failed in rgbasm | they do not compile; `Instr::check` rejects what the types cannot (`ld a, 300`, `bit 8`, `rst $09`) |
| `And`/`Cp` took one operand, `Or`/`Xor`/`Sub` two; `AdcA` and `Adc`; `cp`/`adc` printed without `a` | one shape per family: the 8-bit ALU takes one source and prints `op a, src`; `AddHl`, `AddSp`; shifts and bit instructions take an `R8` |
| `asm.section("Header", "ROM0[$100]")` | `asm.section(Section::rom0("Header").at(0x100))` |
| `asm.section("Vars", "WRAM0")`, `"HRAM"`, ... | `Section::wram0("Vars")`, `Section::hram(..)`, or `Section::new(name, MemoryType::Hram)` |
| `"ROMX[$4000], BANK[2]"`, `"ROM0, ALIGN[8]"` | `Section::romx(name).at(0x4000).bank(2)`, `Section::rom0(name).align(8)` |
| `Instr::Section { name, mem_type }` | `Instr::Section(Section)` |
| `asm.ds("$150 - @", "0")` (`Instr::Ds { num_bytes, starter_point }`) | `asm.ds_fill("$150 - @", "0")` (`Instr::Ds { count, fill: Some(..) }`); `asm.ds("4")` reserves without a fill |
| a section text RGBDS rejects, code or data in a RAM section, a section name used twice: rgbasm failed | panics when the `Section` is built, or in `Asm::emit` (`Asm::try_emit` returns it). In a `RustBoy` program `build()` returns `Error::Section` |
| `gb.raw(\|asm: &mut Asm\| ..)` | `gb.raw(\|asm: &mut Layout\| ..)`: the same builder methods, plus `chunk`, `labels`, `emit_code` |
| `asm.chunk(Chunk::Functions)` on an `Asm` | an `Asm` is printed in the order it is written. Build a part early in a `Block` and emit it later, or use an `engine::Layout` and `layout.program()` |
| `asm.get_chunk(chunk)`, `asm.get_main_instrs()` | `layout.get_chunk(chunk)`; `asm.instrs()` |
| a fresh `Asm` used as a scratch buffer | `Block` (the same builder methods), `block.into_instrs()` |
| `LabelAllocator::new()` beside an `Asm` program | `asm.labels()`, the program's allocator |
| `LabelAllocator::local("check left")` (any text) | panics: a stem is made of identifier characters |
| generated labels `ClearOam`, `AnimEnd`, `.check_left_N_end`, `.spriteK_left_limit_N_store` / `_end` | `.clear_oam_N`, `.anim_end_N`, `.check_left_end_N`, `.spriteK_left_limit_store_N` / `_end_N` |
| a `jr` out of range (or to an external symbol) failed in rgbasm | it is printed as `jp`, one byte longer. Code of fixed size or timing must write jumps that reach. A target `Label + 2`, or `@` used elsewhere than `ds N - @`, leaves the program unrelaxed |

### 6. Hardware facts (`hw`)

| Before | After |
|---|---|
| `hw::LCDC`, `LY`, `P1`, `BGP`, `OBP0`, `OBP1`, `OAMRAM`, `LCDCF_*`, `P1F_*` were `&str` | `hw::Symbol`s: pass them to the builders as they are (`ld_addr_def_a(hw::LCDC)`); `Expr::from(hw::LCDCF_ON)`; `.name` for the text |
| `hw::SCRN0`, `SCRN1` were `u16`; `OAM_COUNT`, `OAM_ENTRY_SIZE`, `OAMA_*` were `u8` | `hw::Symbol`s: `.value` for the number |
| `hw::OAM_START`, `hw::WRAM0` | `hw::OAMRAM.value`, `hw::RAM.value` |
| `hw::SCRN_ROW_TILES`, `hw::SCRN_ROWS` (`usize`) | `hw::SCRN_VX_B.value`, `hw::SCRN_VY_B.value` (`u8`) |
| `hw::oam_offset(index, 1)` | `hw::oam_offset(index, hw::OAMA_X)`; panics past OAM or on another symbol than an `OAMA_*` |
| `format!("{}", hw::LCDC)` | `hw::LCDC.name` (`Symbol` has no `Display`) |
| `asm.cp(hw::LCDC)` (an address in an 8-bit ALU operand) | does not compile (the ALU takes a `Symbol<u8>`); write `asm.cp(hw::LCDC.name)` if it is really meant |
