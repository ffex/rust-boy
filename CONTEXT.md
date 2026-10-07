# rust-boy — Project Context

Background for humans and AI assistants working on rust-boy: what the project is, how it is
built, how the code is layered, what is broken and why. The actionable checklist lives in
[`Task.md`](Task.md); working rules live in [`CLAUDE.md`](CLAUDE.md).

> Snapshot taken on branch `refactor` at commit `4601a5c` (October 2026). Line numbers refer to
> that commit. Every bug below was reproduced or confirmed by reading the code and then
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
- The committed `.o` files are RGBDS object format `RGB9` (RGBDS 1.0). Generated code uses `0x05`-style
  constants, which needs **RGBDS ≥ 0.9**. `rgbasm -L` (the old README used it) was removed in RGBDS 0.8.
- `hardware.inc` (v4.x) lives once in `include/hardware.inc` (it used to be copied 6 times under
  `examples/`); pass it with `rgbasm -I include`.

### Commands

```bash
cargo build                              # library + all bins
cargo test                               # unit tests
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
| `cargo test` | ✅ 38 unit tests and the README examples as doctests pass (was: 8 type errors, fixed — [B1](#b1)) |
| bin `coin-anim` | ✅ compiles (was broken, fixed — [B2](#b2)); sprites still render wrong until [B4](#b4) |
| bin `unbricked_rustboy` | ✅ assembles and links with RGBDS 1.0.4 (was: "`wCurKeys` already defined", fixed — [B3](#b3)) |
| bin `unbricked_std` | ✅ assembles and links with RGBDS 1.0.4; paddle bounce fixed ([B5](#b5)) |
| bin `fosdem` | ⚠️ assembles, but the 16×16 player collapses at screen edges ([B6](#b6)) |
| Output determinism | ✅ every bin prints the same `.asm` on every run (was random, fixed — [B13](#b13)) |
| CI | ✅ GitHub Actions: fmt, clippy `-D warnings`, tests (stable and Rust 1.85), every example assembled with RGBDS 1.0.4 |
| Committed build artifacts | ✅ none (the 12 `*.gb` / `*.o` files were untracked; `.gitignore` covers them) |

---

## 2. Layer map

| Layer | Path | LOC | Role |
|---|---|---|---|
| **L1 `gb_asm`** | `src/gb_asm/` (`instr.rs`, `asm.rs`, `codegen.rs`) | ~815 | `Instr`/`Operand`/`Register` enums, fluent `Asm` builder, `Chunk` buckets, `Display` → RGBDS text |
| **L2 `gb_std`** | `src/gb_std/` (`flow/`, `graphics/`, `inputs.rs`, `variables.rs`, `utility.rs`) | ~2040 | Stateless routines returning `Vec<Instr>` (Memcopy, WaitVBlank, UpdateKeys, GetTileByPixel…), control flow (`If`, `IfConst`, `IfA`, `IfCall`, `Call`, `Emittable`), a simple `SpriteManager`, `TileRef` |
| **L3 `rust_boy`** | `src/rust_boy/` (`rustboy.rs`, `sprites.rs`, `tiles.rs`, `variables.rs`, `functions.rs`, `animations.rs`, `inputs.rs`, `memory.rs`) | ~2550 | `RustBoy` engine: tile/VRAM, variable/WRAM, sprite/OAM managers, builtin-function registry, input bindings, animations, `build()` |
| Examples | `src/bin/` | ~2410 | 6 binaries (raw `gb_asm`, `gb_std`, and `rust_boy` versions of the Unbricked tutorial, plus FOSDEM demo and coin animation) |

### Key concepts

- **`Emittable`** (`src/gb_std/flow/emittable.rs`): `fn emit(&mut self, counter: &mut usize) -> Vec<Instr>`.
  Implemented by `Vec<Instr>`, `Vec<Vec<Instr>>`, `Vec<Box<dyn Emittable>>`, `Call`, `Op`, `If*`.
  Everything that goes into the main loop / init / user functions is an `Emittable`.
- **`If` labels**: each `If*` takes a unique number from `RustBoy::if_counter` and emits *local*
  labels `.end_if_N`, `.else_N`, `.then_N`. RGBDS local labels are scoped to the **last global label**,
  so any global label emitted inside an `If` body breaks it ([B7](#b7)).
- **Chunks** (`src/gb_asm/asm.rs:10-30`): `Header, Constants, Init, MainLoop, Main(legacy), Functions,
  Tiles, Tilemap, Data`, printed in that fixed order by `Asm::to_asm` (`src/gb_asm/codegen.rs:18-28`).
- **Scratch-`Asm` idiom**: most `gb_std`/`rust_boy` helpers create a fresh `Asm`, emit into its default
  `Chunk::Main` and return `asm.get_main_instrs()`.

### What `RustBoy::build()` emits (`src/rust_boy/rustboy.rs:258-372`)

1. **Header**: `INCLUDE "hardware.inc"`, `SECTION "Header", ROM0[$100]`, `jp EntryPoint`, `ds $150 - @, 0`.
   Everything after this stays in that one ROM0 section (no further `SECTION` for code/data).
2. **Constants**: `DEF name EQU value` for each `define_const*`.
3. **Init**: `EntryPoint:` → `call WaitVBlank` → LCD off → `Memcopy` every tile/tilemap blob to VRAM →
   clear OAM + write initial sprites (only if sprites exist) → **user `init()` code** → create animation
   variables → **variable initialisation** → LCD on (`LCDCF_ON|BGON|OBJON|OBJ16`) → `rBGP`, `rOBP0` = `%11100100`.
4. **MainLoop**: `Main:` → `call WaitNotVBlank` → `call WaitVBlank` → animation dispatcher → user main-loop
   code (incl. `UpdateKeys` + key checks) → `jp Main`.
5. **Main (legacy)**: whatever was written through `RustBoy::raw()` (unreachable unless labelled, [B15](#b15)).
6. **Functions**: used builtins + all user functions + `Anim_*` functions.
7. **Tiles / Tilemap**: `Label:` + `dw`/`INCBIN` + `LabelEnd:`; **Data**: `SECTION "Variables", WRAM0` + `name: db/dw`.

---

## 3. Are the levels correct? (assessment)

**Short answer: yes, three levels is the right idea — but the boundaries leak.** The intent
(raw instructions → reusable routines → game engine) is sound and matches how GB developers think.
The problems are where each layer reaches across the line:

1. **The assembler layer knows the game layout.** `Chunk::{Init, MainLoop, Tiles, Tilemap, Data}`
   (`src/gb_asm/asm.rs:10-30`) and their fixed order (`src/gb_asm/codegen.rs:18-28`) are engine
   concepts. `include_hardware()` hardcodes `hardware.inc` (`src/gb_asm/asm.rs:393-397`).
2. **L1 is not really typed.** Registers and expressions are passed as strings:
   `ld_hli_label("a")`, `inc_label("de")`, `or_label("a", "c")` (`src/gb_asm/asm.rs:142-147, 250-252, 279-284`).
   `Operand::Imm`/`Label` are accepted as destinations, so `ld 1, 2` or `sub hl, bc` compile in Rust and
   fail only in rgbasm. Instruction shapes are inconsistent (`And`/`Cp` take one operand, `Or`/`Xor`/`Sub` two;
   `AdcA` vs `Adc`).
3. **Hardware facts are hardcoded in every layer.** `_OAMRAM+{id*4+1}` strings in both sprite managers,
   `$9800` in three places, VRAM bases in `tiles.rs`, LCDC flags that disagree between layers
   (OBJ16 forced in `rust_boy`, not in `gb_std`). `MemoryRegion`/`MemoryAllocator` (`src/rust_boy/memory.rs`) exist but are unused.
4. **L3 re-implements L2 instead of using it.** `src/rust_boy/functions.rs:152-311` duplicates Memcopy,
   WaitVBlank, WaitNotVBlank, UpdateKeys and GetTileByPixel from `gb_std`, and they have already
   diverged ([B23](#b23)). L2 also contains its own `SpriteManager` that duplicates L3's.
5. **No layer owns labels.** `gb_std` hardcodes global labels (`Left`, `CheckLeft`, `ClearOam`), `rust_boy`
   builds them with `format!`, `If` uses local labels — they collide and break scoping ([B7](#b7), [B25](#b25)).

### Proposed target

```
            ┌──────────────────────────────────────────────┐
  engine    │ RustBoy, managers, allocation, build pipeline│  (was rust_boy)
            └───────────────┬──────────────────────────────┘
            ┌───────────────▼──────────────────────────────┐
  std       │ stateless Routines {name, body, deps,        │  (was gb_std)
            │ clobbers}, If/While/Switch, snippets          │
            └───────────────┬──────────────────────────────┘
            ┌───────────────▼──────────────────────────────┐
  asm       │ typed ISA, directives, first-class sections,  │  (was gb_asm)
            │ label allocator, Emittable + Block buffer     │
            └──────────────────────────────────────────────┘
  hw        pure data: register/flag/OAM-layout symbols (emitted as hardware.inc names),
            used by std and engine; asm does not depend on it.
```

Rules: each layer depends only downward; the engine never formats register/hardware strings itself;
every routine exists exactly once; every generated label comes from the allocator.

---

## 4. Bug catalogue

Severity: **P0** = broken today (build, or visibly wrong in a shipped example) · **P1** = real bug users
will hit · **P2** = latent, edge case or documentation.

### P0

#### B1
**`cargo test` does not compile.** `src/rust_boy/variables.rs:282-284, 295-297, 306-307` pass the `Var`
returned by `create_u8/create_u16/create_i8` to `get_label/get_address/get_type`, which take `VarId`
(`:174, :179, :184`). 8 × E0308. *Fix:* return/accept the right type (e.g. store `VarId` inside `Var`, or
look up by name) and update the tests.
**Status: fixed** on `refactor-p0-fix-build` — `Var` now carries its `VarId` (`Var::id()`), tests use it.

#### B2
**Bin `coin-anim` does not compile.** `src/bin/coin-anim/main.rs:15, 18, 19` call
`enable_animation("CoinAnim")` / `disable_animation("CoinAnim")`, but the signatures are now
`enable_animation(SpriteId, u8)` / `disable_animation(SpriteId)` (`src/rust_boy/sprites.rs:184, 210`).
Line 15 also discards its result (no effect). *Fix:* delete line 15 (the default is already disabled),
use `enable_animation(coin, idx)` / `disable_animation(coin)`.
**Status: fixed** on `refactor-p0-fix-build` (A starts the animation, B stops it).

#### B3
**Duplicate labels `wCurKeys` / `wNewKeys` in `unbricked_rustboy`.** The example creates them
(`src/bin/unbricked_rustboy/main.rs:50-51`) and `RustBoy::add_inputs` creates them again
(`src/rust_boy/rustboy.rs:418-419`); `VariableManager::create_var` (`src/rust_boy/variables.rs:146`) never
rejects duplicates, so `wCurKeys: db` is emitted twice → rgbasm "already defined". The same would happen
with `wFrameCounter` as soon as the example adds an animation. *Fix:* make `create_var` idempotent for
same name+type (or error on conflict); remove the manual creation from the example.
**Status: fixed** on `refactor-p1-duplicate-vars`: creating an existing name returns that variable (first
initial value and section kept), a different type panics; the example no longer creates the input variables.

#### B4
**All sprites are forced to 8×16.** `src/rust_boy/rustboy.rs:313` always sets `LCDCF_OBJ16` (added in
`0be3a2f` for the FOSDEM 16×16 character). In 8×16 mode the hardware ignores bit 0 of the tile index, so
8×8 sprites draw wrong: in `unbricked_rustboy` Paddle (tile 0) and Ball (tile 1) both draw tiles 0+1;
`coin-anim`'s 8×8 frames show stacked pairs. *Fix:* sprite size in a `RustBoyConfig`; align tile indices
to even numbers in 8×16 mode.

#### B5
**Two-operand `If` comparisons are inverted.** `If::emit` (`src/gb_std/flow/flow_if.rs:287-327`) runs
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

#### B6
**Composite (16×16) sprites split apart at screen edges.** `move_composite_{left,right}_limit`
(`src/rust_boy/sprites.rs:333-366`) apply the same absolute limit to each 8×16 half independently. In
`fosdem` the halves start at OAM X 88/96; holding Left, the left half stops at X=1 but the right half keeps
going until it also reaches 1 → the character collapses to 8 px wide and stays so (visible in
`examples/fosdem/main.asm:119-156`). *Fix:* test only the leading half and move all halves together (or
offset each half's limit by its position in the composite).

### P1

#### B7
**Reusable snippets emit fixed global labels.**
- `check_key` → `CheckLeft` / `CheckLeftEnd` (`src/gb_std/inputs.rs:129-139`): two bindings on the same
  button → duplicate label.
- `move_*_limit` → `Sprite{N}LeftLimitEnd` etc. (`src/rust_boy/sprites.rs:485, 505, 525, 545`): the same
  move used twice (e.g. two buttons) → duplicate label.
- `gb_std` `Sprite::move_*` → `Left`/`LeftEnd`, `LeftLimit`/`LeftLimitEnd`… with no sprite id
  (`src/gb_std/graphics/sprites.rs:83-174`): two sprites → duplicate label.
- Scope: a global label inside an `If` body (e.g. `If::eq(.., gb.sprites.move_left_limit(..))`) makes the
  `.end_if_N:` definition land under the new global scope while the `jp` referenced it under the old one →
  unresolved symbol.

*Fix:* a label allocator; generated labels local or uniquely numbered.

#### B8
**`move_*_limit` only stops on exact equality.** `cp limit` + `jp z` (`src/rust_boy/sprites.rs:490, 510,
530, 550`; `src/gb_std/graphics/sprites.rs:132, 146, 160, 174`). With distance 2 from x=24 toward limit 15
the sprite goes 22, 20, 18, 16, 14 … and wraps through 0/255. A start position already past the limit
never stops. *Fix:* compare with carry (`jr c`/`jr nc`) and clamp.

#### B9
**`jr` out of range in the animation dispatcher.** `jr c, AnimEnd` (`src/rust_boy/sprites.rs:661`, label at
`:702`) jumps over the whole dispatch block: 5 bytes + 7 per animated sprite + 9 per animation. FOSDEM
(2 sprites × 4 animations) = 91 bytes; **3 sprites × 4 animations = 134 bytes > 127** → rgbasm error.
Two 16×16 animated characters are enough. *Fix:* `jp`, or a jump table.

#### B10
**`AnimationType::PingPong` and `::Once` are ignored.** `Animation.anim_type`
(`src/rust_boy/animations.rs:17`) is never read; `generate_loop_func` (`:23-61`) always loops. Advertised in
the `add_animation` docs (`src/rust_boy/sprites.rs:118`) and in `docs/animations.md` (documentations branch).
*Fix:* implement them (or remove the variants until implemented).

#### B11
**Code passed to `gb.init()` is overwritten.** `build()` emits user init code at
`src/rust_boy/rustboy.rs:297`, then creates the animation variables (`:300-306`) and emits variable
initialisation at `:310`, which writes every variable's initial value. `gb.init(lives.set(3))` ends with
`wLives = 0`; `gb.init(gb.sprites.enable_animation(coin, 0))` is reset to 255 (disabled). LCDC/palette
writes in init are likewise overwritten by `:313-320`. *Fix:* emit variable init (and hardware defaults)
**before** user init code.

#### B12
**OAM is accessed directly, without shadow OAM + DMA.** Sprite moves, `get_x/get_y/get_pivot` and the
animation functions read-modify-write `_OAMRAM+n` from the main loop (`src/rust_boy/sprites.rs:445-612`,
`src/rust_boy/animations.rs:28-58`; loop at `src/rust_boy/rustboy.rs:325-339`). OAM is only accessible in
VBlank/HBlank: in modes 2/3 writes are dropped and reads return `$FF`. It works only while the whole main
loop fits in VBlank (~1140 M-cycles; `unbricked_rustboy` already uses ~600). Growth → silent sprite glitches.
*Fix:* shadow OAM in WRAM (`ALIGN[8]`) + OAM DMA routine in HRAM, run in VBlank.

### P2

#### B13
**Generated assembly is non-deterministic.** Output order depends on `HashMap`/`HashSet` iteration:
`src/rust_boy/functions.rs:67-71, 133-147`; `src/rust_boy/variables.rs:99-102, 194, 214`;
`src/rust_boy/sprites.rs:49-50, 629, 668, 713`; `src/rust_boy/tiles.rs:72, 186, 206, 234, 257`;
`src/gb_std/variables.rs:21, 41`. Reproduced: 3 runs of `fosdem` / `unbricked_rustboy` → 3 different files.
No behavioural impact today, but it makes diffs and snapshot tests impossible (must be fixed before them).
*Fix:* `BTreeMap` / insertion-ordered `Vec`.
**Status: fixed** on `refactor-p0-deterministic-output`: id-keyed managers use `BTreeMap` (ids are
sequential, so creation order), name-keyed lists (user functions, variable sections, `gb_std`
`VariableSection`) use a `Vec` in declaration order, builtins come in enum order. `Asm.chunks` stays a
`HashMap` because `to_asm` reads it in a fixed order.

#### B14
**`build()` is not idempotent.** `build(&mut self)` (`src/rust_boy/rustboy.rs:258`) creates `wFrameCounter`
and the `wAnim_*_Current` variables on every call (`:300-306`) → a second `build()` emits duplicate labels.
*Fix:* `build(&self)`, all registration done up front.
**Status: fixed** on `refactor-p1-duplicate-vars`: with B3 fixed, the variables created again by a second
`build()` are the existing ones, so two builds give the same output (tested). Making `build` take `&self`
stays in Phase 2.

#### B15
**`RustBoy::raw()` silently drops code.** The closure runs on `self.asm` (`src/rust_boy/rustboy.rs:141-147`)
but `build()` copies only its `Chunk::Main` (`:365-369`); anything written after `asm.chunk(Chunk::Functions)`
inside the closure is lost (and later `raw()` calls too, since the chunk persists). The `Main` chunk is printed
right after `jp Main`, so raw code is unreachable unless it starts with a label that is called — the doc
example (`ld a, 0x42; ret`) is dead code. *Fix:* merge all chunks; document placement.

#### B16
**`Var::set`/`Var::get` ignore 16-bit variables.** `set(value: i8)` (`src/rust_boy/variables.rs:31`) writes
only the low byte; `get` (`:43`) loads one byte; `var_type` (`:26`) is never read. (Setting a `u8` > 127
works via `200u8 as i8`, but the API is awkward.) *Fix:* typed setters per `VarType`, 16-bit load/store.

#### B17
**No bounds or overflow checks.**
- VRAM: sprite tiles past `$8FFF` (`src/rust_boy/tiles.rs:94`), BG tiles past `$97FF` run into the tilemap (`:117`).
- OAM: no 40-sprite cap; `generate_init_code` writes past `$FE9F`.
- `u8` arithmetic that panics in debug / wraps in release: `next_tile_index += tile_count`
  (`src/rust_boy/sprites.rs:82`, overflows at exactly 256 tiles, e.g. two FOSDEM characters),
  `sprite.y + 16` / `sprite.x + 8` (`:425, :429`), `x + 8` (`src/rust_boy/rustboy.rs:487`),
  `tile_count() as u8` (`:443`), `oam_index * 4` (many sites).
- `cp_imm(abs_end + self.frame_step)` (`src/rust_boy/animations.rs:50`) overflows when the last frame is tile
  254/255 → in release `cp 0`, the animation freezes on its first frame.
- `MemoryAllocator` (`src/rust_boy/memory.rs:36`) exists, with overflow checks, but nothing uses it.

*Fix:* use the allocator, `u16` counters + `checked_add`, clear errors.

#### B18
**Two tile counters can desync.** `SpriteManager.next_tile_index` (`src/rust_boy/sprites.rs:82`) and
`TileManager.next_sprite_addr` (`src/rust_boy/tiles.rs:94`) are kept in sync only by `RustBoy::add_sprite`.
Calling the public `gb.tiles.add_sprite` or `gb.sprites.add` directly (the `RustBoy` doc example does) → wrong
tile indices. *Fix:* single source of truth.

#### B19
**Every tilemap goes to `$9800`.** `add_tilemap` hardcodes `vram_address: 0x9800`
(`src/rust_boy/tiles.rs:160`); with two tilemaps, the last one created wins (it was random before B13). No `$9C00`.

#### B20
**Silent failures.** Unknown `SpriteId`/`CompositeSpriteId` → empty `Vec` (move/get/enable methods in
`src/rust_boy/sprites.rs`); `enable_animation_by_name` / `set_initial_animation_by_name` (`:197, :171`) do
nothing on a typo; `add_animation_with_step` returns 0 for an unknown id. *Fix:* `Result` or panic with a
clear message.

#### B21
**`basic_usage` and the README Basic Example lack header padding.** `SECTION "Header", ROM0[$100]` with only
`nop` + `jp EntryPoint` and no `ds $150 - @, 0` (`src/bin/basic_usage.rs:8`, `README.md:53`). Once the floating
ROM0 section grows past 256 bytes, rgblink can place it at `$0104`, where `rgbfix` overwrites the cartridge
header. (`RustBoy` itself is correct: `src/gb_std/utility.rs:5-7`.)
**Status: fixed** on `refactor-p0-readme`: both now emit `ds $150 - @, 0`.

#### B22
**`get_pivot` silently clamps.** `u8::try_from(16 + y_offset).unwrap_or(0)` (`src/rust_boy/sprites.rs:570, 576`;
`src/gb_std/graphics/sprites.rs:208, 214`) turns out-of-range offsets into `sub 0`. *Fix:* wrapping arithmetic
or an error.

#### B23
**Duplicated routines have diverged.** `rust_boy`'s `GetTileByPixel` appends `ld a, [hl]`
(`src/rust_boy/functions.rs:307`), the `gb_std` one does not (`src/gb_std/graphics/utility.rs:109-150`): one
label, two contracts. Memcopy, WaitVBlank, WaitNotVBlank and UpdateKeys are also duplicated
(`src/rust_boy/functions.rs:152-266` vs `src/gb_std/graphics/utility.rs`, `src/gb_std/inputs.rs`), and
`src/bin/unbricked.rs` has a third copy. *Fix:* one routine registry.

#### B24
**All user functions are emitted, even unused.** `generate_all` (`src/rust_boy/functions.rs:133-147`) emits
every `user_functions` body; `used_user_functions` (`:71`) is written but never read. Note: filtering on it
today would break linking, because calls made through `Call`/`IfCall` are not tracked ([B26](#b26)) — fix B26 first.

#### B25
**Animation labels are not namespaced by sprite.** `Anim_{name}` and `.skip_{name}`
(`src/rust_boy/sprites.rs:632, 685-686`): two sprites that both have a `"Spin"` animation, or two composites
with `"Walk"`, emit duplicate labels. Names with `-` or spaces produce invalid labels. *Fix:* prefix with the
sprite; validate names.

#### B26
**Builtins reached through `Call`/`IfCall` are not auto-included.** `Call::emit`
(`src/gb_std/flow/emittable.rs:48`) and `IfCall::emit` (`src/gb_std/flow/flow_if.rs:903-943`) just emit
`call X`; only `RustBoy::call`/`call_args`/`use_function` register builtins (`src/rust_boy/functions.rs:101-115`),
and `define_function_from`/`add_to_main_loop` (`src/rust_boy/rustboy.rs:180, 388`) don't scan bodies. Following
the `Call` doc example alone (`Call::with_args("GetTileByPixel", ..)`) → `call GetTileByPixel` with no routine
→ rgblink "undefined symbol". `unbricked_rustboy` works only because it also calls `gb.call_args("GetTileByPixel", ..)`.
*Fix:* routines as values with dependencies, or scan emitted `Call` targets in `build()`.

#### B27
**`Memcopy` with length 0 copies 64 KiB.** Memcopy is a do-while loop (`src/gb_std/graphics/utility.rs:54-70`;
`src/rust_boy/functions.rs:152-170`); `BC = 0` wraps to `$FFFF` and overwrites WRAM, the stack, I/O and IE.
Reachable through `cp_in_memory` (`src/gb_std/graphics/utility.rs:45`) or `generate_memcopy_calls`
(`src/rust_boy/tiles.rs:252`) with an empty tile set / empty `.2bpp`. *Fix:* skip empty blobs at generation
time, or test `BC` before the first copy.

#### B28
**OBP1 never initialised; OAM not cleared without sprites.** Only `rBGP` and `rOBP0` are written
(`src/rust_boy/rustboy.rs:316-320`), so sprites with the OBP1 palette flag get a random palette on DMG. OAM is
cleared only `if !self.sprites.is_empty()` (`:292`), yet `LCDCF_OBJON` is always set (`:313`) → a
background-only program shows garbage objects on real hardware.

#### B29
**Documentation errors in code and README.**
- `src/gb_std/inputs.rs:54` says `wCurKeys` "0 = pressed"; after the `xor` it is 1 = pressed.
- `RustBoy::call` doc example `gb.add_to_main_loop(gb.call("X"))` (`src/rust_boy/rustboy.rs:202, 206`) does
  not compile (E0499, two `&mut` borrows); hidden by ```` ```ignore ````.
- README: "Type-safe … compile-time guarantees" (`:12`) is an overclaim; "Complete support for … instruction
  set" (`:19`) — `push/pop/halt/di/ei/reti/sbc/bit/set/res/rl/rr/sla/sra/cpl/nop/scf/ccf/rst` are missing;
  "two example programs" (`:86`) — there are 6; the project tree omits `src/rust_boy/`; `rgbasm -L` (`:146`)
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
  always emits `WRAM0`.

---

## Checked and refuted

Claims that were investigated and **rejected**, kept here so nobody re-investigates them:

| Claim | Why rejected |
|---|---|
| `is_specific_tile` panics on an empty slice (`tiles_ids.len() - 1`) | The subtraction is inside the `for` body, which never runs for an empty slice. |
| `Functions`/`Tiles` chunks have no `SECTION` and land in the wrong section | Only reachable via `raw()` opening a section → covered by [B15](#b15). Sections become first-class in Phase 2. |
| `Asm::ds` always emits a fill byte, so RAM can't be reserved | True, but a missing feature, not a bug (Phase 2: sections / `ds n` without fill). |
| Everything in one `ROM0[$100]` section fails above 16 KB | Tile data is copied to VRAM anyway; ROM banking is a feature (Phase 3: MBC). |
| `If` comparisons are unsigned while `i8` vars exist | Native `cp` semantics, documented in `flow_if.rs:8-15`; only a doc note ([B29](#b29)). |
| `enable_animation` on a sprite without animations references an undefined variable | API misuse; covered by "fail loudly" in [B20](#b20). |
| 8×16 tile indices not aligned after an odd-sized 8×8 sprite | Only happens when mixing sizes → part of [B4](#b4). |
| `0x05` constants require RGBDS ≥ 0.9 | The project toolchain is RGBDS 1.0; handled by pinning the version in CI. |
| `TileSource::from_file` tile count not checked against the file | Caller error; became a feature (derive the count from the file size). |

---

## 5. Missing features (summary)

Detailed list in [`Task.md`](Task.md) Phase 3. Biggest gaps:

- **Audio: nothing at all** (no APU registers, no sound effects, no music driver).
- **Graphics:** no shadow OAM/DMA, no scrolling, no window layer, no palette API/fades, 8×16 forced, no
  metasprites beyond 16×16, no text/numbers, no `$9C00` map.
- **Animation:** only `Loop` works; global speed only; no events.
- **Engine:** polling instead of VBlank interrupt + `halt`; no interrupts/timers, scenes, RNG, collision,
  16-bit math, loops/switch.
- **ISA:** `push/pop`, `halt`, `di/ei`, `reti`, `sbc`, `bit/set/res`, rotates/shifts, `cpl`, `ld [hl-]`…
- **Platform:** single ROM0 bank, no SRAM saves, no GBC.
- **Tooling:** CI exists now (fmt, clippy, tests, assembling every example); still missing: snapshot tests of
  the generated asm and a one-command "build ROM and run".

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
