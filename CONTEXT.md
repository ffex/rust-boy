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
- The project is built with RGBDS 1.0 (the `.o` files once committed were object format `RGB9`). Generated code uses `0x05`-style
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
| `cargo test` | ✅ 147 library unit tests (and one in the `basic_usage` bin) and the doctests (README examples, `LabelAllocator`, `RustBoy::labels`, `RustBoy::keep_function`) pass (was: 8 type errors, fixed — [B1](#b1)) |
| bin `coin-anim` | ✅ compiles (was broken, fixed — [B2](#b2)); the 8×8 frames render right (were drawn as 8×16 pairs, fixed — [B4](#b4)) |
| bin `unbricked_rustboy` | ✅ assembles and links with RGBDS 1.0.4 (was: "`wCurKeys` already defined", fixed — [B3](#b3)); Paddle and Ball each draw their own tile ([B4](#b4)) |
| bin `unbricked_std` | ✅ assembles and links with RGBDS 1.0.4; paddle bounce fixed ([B5](#b5)) |
| bin `fosdem` | ✅ assembles; the 16×16 player moves as one block and stops at its limits (it collapsed at screen edges, fixed — [B6](#b6)) |
| Output determinism | ✅ every bin prints the same `.asm` on every run (was random, fixed — [B13](#b13)) |
| Generated labels | ✅ a key check or move can be used any number of times and inside an `If`, and two sprites can share an animation name (fixed — [B7](#b7), [B25](#b25)); unit tests check the labels with the RGBDS scope rules (`gb_asm::label_check`) |
| Start-up code | ✅ `gb.init()` code runs after the variables (animation variables included) and palettes are set, so what it sets survives (was overwritten, fixed — [B11](#b11)); the OAM is always cleared and `rOBP1` is set (fixed — [B28](#b28)); unit tests run the start-up code on `gb_asm::test_cpu` |
| Animations | ✅ any number of animated sprites and animations assemble (the dispatcher's `jr` went out of range from 3 sprites × 4 animations, fixed — [B9](#b9)); `Loop`, `PingPong` and `Once` all work (`PingPong`/`Once` played as `Loop`, fixed — [B10](#b10)); unit tests run the generated code frame by frame (`gb_asm::test_cpu`) |
| Functions and routines | ✅ `build()` emits each builtin and user function the generated code refers to (`call`, `jp`, `Call`, `IfCall`, function bodies, raw code; a builtin reached through `Call`/`IfCall`/a function body was missing, fixed — [B26](#b26)), once, with the variables it needs, and only those (unused user functions were emitted, fixed — [B24](#b24)); `RustBoy::keep_function` forces one, `RustBoy::external_symbol` declares one defined outside (an `INCLUDE`d file, which `build()` does not read); one `GetTileByPixel` in the library, with one contract (fixed — [B23](#b23)); empty raw tile data is not copied (fixed — [B27](#b27)) |
| CI | ✅ GitHub Actions: fmt, clippy `-D warnings`, tests (stable and Rust 1.85), every example assembled with RGBDS 1.0.4, and the whole-program unit tests linked with it (`RGBDS_LINK_CHECK`, since [B26](#b26)) |
| Committed build artifacts | ✅ none (the 12 `*.gb` / `*.o` files were untracked; `.gitignore` covers them) |

---

## 2. Layer map

| Layer | Path | LOC | Role |
|---|---|---|---|
| **L1 `gb_asm`** | `src/gb_asm/` (`instr.rs`, `asm.rs`, `codegen.rs`, `labels.rs`) | ~930 | `Instr`/`Operand`/`Register` enums, fluent `Asm` builder, `Chunk` buckets, `Display` → RGBDS text, `LabelAllocator` (since [B7](#b7)) |
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
- **Snippet labels** (since [B7](#b7)): code that can be emitted more than once (key checks, limited
  moves) uses *local* labels numbered by a `gb_asm::LabelAllocator` (`.check_left_2`,
  `.sprite0_left_limit_0_end`). Clones of an allocator share one counter: `RustBoy` owns one and shares
  it with its `SpriteManager`; `gb_std` callers pass one to `check_key` and `Sprite::move_*_limit`
  (in a `RustBoy` program, `gb.labels()`).
  Global labels are left to routines and functions (`Memcopy`, `Anim_{sprite}_{animation}`, user
  functions) and to the once-per-program `EntryPoint`, `ClearOam`, `Main`, `AnimEnd`.
- **Chunks** (`src/gb_asm/asm.rs:10-30`): `Header, Constants, Init, MainLoop, Main(legacy), Functions,
  Tiles, Tilemap, Data`, printed in that fixed order by `Asm::to_asm` (`src/gb_asm/codegen.rs:18-28`).
- **Scratch-`Asm` idiom**: most `gb_std`/`rust_boy` helpers create a fresh `Asm`, emit into its default
  `Chunk::Main` and return `asm.get_main_instrs()`.

### What `RustBoy::build()` emits (`src/rust_boy/rustboy.rs:414-569`, `build` prints what `build_asm` returns)

1. **Header**: `INCLUDE "hardware.inc"`, `SECTION "Header", ROM0[$100]`, `jp EntryPoint`, `ds $150 - @, 0`.
   Everything after this stays in that one ROM0 section (no further `SECTION` for code/data).
2. **Constants**: `DEF name EQU value` for each `define_const*`.
3. **Init**: `EntryPoint:` → `call WaitVBlank` → LCD off → `Memcopy` every non-empty tile/tilemap blob to VRAM ([B27](#b27)) →
   clear the whole OAM (always, with `gb_std`'s `initialize_objects_screen` + `clear_objects_screen`) and write
   the initial sprites → `rBGP`, `rOBP0`, `rOBP1` = `%11100100` → create animation variables → **variable
   initialisation** → **user `init()` code** → LCD on (`LCDCF_ON|BGON|OBJON` + `OBJ8`, or `OBJ16` after
   `set_sprite_size(Size8x16)`). (Since [B11](#b11)/[B28](#b28); before, user code ran before the variables were
   set, the palettes after LCD on, `rOBP1` was never set and the OAM was cleared only when sprites existed.)
4. **MainLoop**: `Main:` → `call WaitNotVBlank` → `call WaitVBlank` → animation dispatcher → user main-loop
   code (incl. `UpdateKeys` + key checks) → `jp Main`.
5. **Main (legacy)**: whatever was written through `RustBoy::raw()` (unreachable unless labelled, [B15](#b15)).
6. **Functions** (since [B24](#b24)/[B26](#b26), worked out once all the code is known, before the variables): the
   builtins, then the user functions, that the code and the animation functions refer to, directly or through
   other functions, plus the ones forced with `use_function` / `keep_function`; then the `Anim_*` functions. The
   variables an emitted builtin needs (`wCurKeys`, `wNewKeys` for `UpdateKeys`) are then created, unless the program defines them, so the
   variable initialisation (step 3) and the Data chunk include them.
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
   `$9800` in three places, VRAM bases in `tiles.rs`, LCDC flags written as strings in each layer
   (until [B4](#b4) they disagreed: OBJ16 forced in `rust_boy`, not in `gb_std`). `MemoryRegion`/`MemoryAllocator` (`src/rust_boy/memory.rs`) exist but are unused.
4. **L3 re-implements L2 instead of using it.** `src/rust_boy/functions.rs:152-311` (at `4601a5c`) duplicated Memcopy,
   WaitVBlank, WaitNotVBlank, UpdateKeys and GetTileByPixel from `gb_std`, and they had already
   diverged ([B23](#b23)). *Since B23* `rust_boy` emits the `gb_std` routines (only `Delay` is its own), and its
   tile copies use `gb_std`'s `cp_in_memory`. L2 still contains its own `SpriteManager` that duplicates L3's.
5. **No layer owns labels.** `gb_std` hardcodes global labels (`Left`, `CheckLeft`, `ClearOam`), `rust_boy`
   builds them with `format!`, `If` uses local labels — they collide and break scoping ([B7](#b7), [B25](#b25)).
   *Since B7/B25:* the asm layer has a `LabelAllocator` that numbers the local labels of snippets, and
   animation labels are namespaced by sprite; `If` still numbers its labels with its own counter and
   `ClearOam` stays a fixed global label (emitted once). Making the allocator the only source is Phase 2.

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
(`src/rust_boy/rustboy.rs:487-488`); `VariableManager::create_var` (`src/rust_boy/variables.rs:146`) never
rejects duplicates, so `wCurKeys: db` is emitted twice → rgbasm "already defined". The same would happen
with `wFrameCounter` as soon as the example adds an animation. *Fix:* make `create_var` idempotent for
same name+type (or error on conflict); remove the manual creation from the example.
**Status: fixed** on `refactor-p1-duplicate-vars`: creating an existing name returns that variable (first
initial value and section kept), a different type panics; the example no longer creates the input variables.

#### B4
**All sprites are forced to 8×16.** `src/rust_boy/rustboy.rs:313` (at `4601a5c`) always sets `LCDCF_OBJ16` (added in
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
**Status: fixed** on `refactor-p1-sprite-limits`: a composite move tests only its leading sprite (the leftmost,
rightmost, topmost or lowest one at creation; the first on a tie), clamps it as in [B8](#b8), then puts every
other sprite at the offset from it that it was created with, so the composite moves as one block or not at all
(and a split composite is put back together on the next move). `fosdem` keeps its stop positions (limits
written as `1`/`149`, see B8); checked in an emulator (PyBoy): before, both halves ended at X 1, then 149;
now the halves stay 8 px apart. Regression test `test_composite_moves_as_one_block`.

### P1

#### B7
**Reusable snippets emit fixed global labels.** (Line numbers at `7b54900`, after [B8](#b8).)
- `check_key` → `CheckLeft` / `CheckLeftEnd` (`src/gb_std/inputs.rs:129-140`): two bindings on the same
  button → duplicate label.
- `move_*_limit` → `Sprite{N}LeftLimitStore` / `Sprite{N}LeftLimitEnd` etc., built at
  `src/rust_boy/sprites.rs:668` (one sprite) and `:540` (a composite, which uses its leading sprite's labels,
  the same as that sprite's own move), with the suffixes added by `move_coord_limit`
  (`src/gb_std/graphics/sprites.rs:53-54`): the same move used twice (e.g. two buttons) → duplicate label.
- `gb_std` `Sprite::move_*` → `Left`/`LeftEnd` (`src/gb_std/graphics/sprites.rs:169-211`) and
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
`If` keeps its own counter (`.end_if_N`), and `ClearOam`/`AnimEnd` stay global (emitted once, outside user
code); one allocator for everything is the Phase 2 item.

#### B8
**`move_*_limit` only stops on exact equality.** `cp limit` + `jp z` (`src/rust_boy/sprites.rs:490, 510,
530, 550`; `src/gb_std/graphics/sprites.rs:132, 146, 160, 174`). With distance 2 from x=24 toward limit 15
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
**`jr` out of range in the animation dispatcher.** `jr c, AnimEnd` (`src/rust_boy/sprites.rs:661`, label at
`:702`) jumps over the whole dispatch block: 5 bytes + 7 per animated sprite + 9 per animation. FOSDEM
(2 sprites × 4 animations) = 91 bytes; **3 sprites × 4 animations = 134 bytes > 127** → rgbasm error.
Two 16×16 animated characters are enough. *Fix:* `jp`, or a jump table.
**Status: fixed** on `refactor-p1-animations`. Every jump whose distance grows with the number of sprites or
animations is a `jp`: `jp c, AnimEnd`, a sprite's `jp z, .animEnd_{sprite}` (disabled) and `jp .animEnd_{sprite}`
after each call (with 16 animations on one sprite these two were out of range too). The only `jr` left,
`jr nz, .skip_{sprite}_{animation}`, always skips 6 bytes (`call` + `jp`). The dispatcher grows by 1 byte per
`jp` (`fosdem` +11 bytes, `coin-anim` +3: their only change, same animation in an emulator). A jump table was not
chosen: it needs `jp hl`, which the typed ISA does not have yet (Phase 2). Test
`test_animation_dispatch_jumps_stay_in_range` checks every `jr` with `gb_asm::label_check::jr_range_errors`, which
knows instruction sizes and agrees with RGBDS 1.0.4 (127 and -128 accepted, 128 and -129 rejected); with the old
code it reports the offsets 285, 144 and 135 that rgblink reports for the same program.

#### B10
**`AnimationType::PingPong` and `::Once` are ignored.** `Animation.anim_type`
(`src/rust_boy/animations.rs:17` at `4601a5c`) was never read; `generate_loop_func` (`:23-61`) always looped. Advertised in
the `add_animation` docs (`src/rust_boy/sprites.rs:118`) and in `docs/animations.md` (documentations branch).
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
`src/rust_boy/rustboy.rs:297`, then creates the animation variables (`:300-306`) and emits variable
initialisation at `:310`, which writes every variable's initial value. `gb.init(lives.set(3))` ends with
`wLives = 0`; `gb.init(gb.sprites.enable_animation(coin, 0))` is reset to 255 (disabled). LCDC/palette
writes in init are likewise overwritten by `:313-320`. *Fix:* emit variable init (and hardware defaults)
**before** user init code.
**Status: fixed** on `refactor-p1-init-order`. The start-up code now runs: LCD off → VRAM copies → OAM clear and
initial sprites → default palettes → every variable set to its initial value (the animation variables
`wFrameCounter`, `wAnim_{sprite}_Current` and `wAnim_{sprite}_Dir` are created first, so they are included) →
**user `init()` code** → LCD on (`src/rust_boy/rustboy.rs:434-476`, put together at `:560-563`). So `gb.init(lives.set(3))`,
`gb.init(gb.sprites.enable_animation(coin, 0))`, a `PingPong` direction or a palette set in `init()` survive.
One difference from the fix above: **`rLCDC` stays after the user code**, because turning the LCD on ends
the start-up, and `init()` code keeps running with the LCD off, so it can still write VRAM and OAM freely; an
`rLCDC` value written in `init()` is still replaced (documented on `RustBoy::init`; LCDC flags belong to the
Phase 2 `RustBoyConfig`). Also documented there, and not new: `init()` code must not wait for VBlank, because with
the LCD off `rLY` stays 0 and the wait never ends. Tests run the start-up code (`RustBoy::build_asm`, the `Init` chunk) on
`gb_asm::test_cpu`, which now models the register pairs `bc`/`de`/`hl` as symbolic addresses
(`ld hl, _OAMRAM` + `ld [hli], a`; one address has one name, `_OAMRAM+4+1` is `_OAMRAM+5`), stubbed routines
(`Memcopy`) and an ordered trace of writes and calls; what it cannot know panics when read (the 8-bit halves of a
pair loaded with an address, and every register, pair and flag after a stub):
`test_init_code_runs_after_variable_initialisation` (a variable, the animation, the `PingPong` direction and
`rBGP` set in `init()`) and `test_startup_order` (the order above).

#### B12
**OAM is accessed directly, without shadow OAM + DMA.** Sprite moves, `get_x/get_y/get_pivot` and the
animation functions read-modify-write `_OAMRAM+n` from the main loop (`src/rust_boy/sprites.rs:445-612`,
`src/rust_boy/animations.rs:86-201`, the `Loop`, `Once` and `PingPong` bodies since [B10](#b10); loop at `src/rust_boy/rustboy.rs:479-495`). OAM is only accessible in
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
**`build()` is not idempotent.** `build(&mut self)` (`src/rust_boy/rustboy.rs:315`, through `build_asm`) creates `wFrameCounter`
and the `wAnim_*_Current` variables on every call (`:368-375`) → a second `build()` emits duplicate labels.
*Fix:* `build(&self)`, all registration done up front.
**Status: fixed** on `refactor-p1-duplicate-vars`: with B3 fixed, the variables created again by a second
`build()` are the existing ones, so two builds give the same output (tested). Making `build` take `&self`
stays in Phase 2.

#### B15
**`RustBoy::raw()` silently drops code.** The closure runs on `self.asm` (`src/rust_boy/rustboy.rs:196-202`)
but `build()` copies only its `Chunk::Main` (`:434-438`); anything written after `asm.chunk(Chunk::Functions)`
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
  `sprite.y + 16` / `sprite.x + 8` (`:425, :429`), `x + 8` (`src/rust_boy/rustboy.rs:577`),
  `tile_count() as u8` (`:518`), `oam_index * 4` (many sites).
- `cp_imm(abs_end + self.frame_step)` (`src/rust_boy/animations.rs:104` since [B10](#b10), `:50` at `4601a5c`) overflows when the last frame is tile
  254/255 → in release `cp 0`, the animation freezes on its first frame. (Since [B10](#b10) only `Loop` does this.)
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
**Duplicated routines have diverged.** (Lines at `4601a5c`.) `rust_boy`'s `GetTileByPixel` appends `ld a, [hl]`
(`src/rust_boy/functions.rs:307`), the `gb_std` one does not (`src/gb_std/graphics/utility.rs:109-150`): one
label, two contracts. Memcopy, WaitVBlank, WaitNotVBlank and UpdateKeys are also duplicated
(`src/rust_boy/functions.rs:152-266` vs `src/gb_std/graphics/utility.rs`, `src/gb_std/inputs.rs`), and
`src/bin/unbricked.rs` has a third copy. *Fix:* one routine registry.
**Status: fixed** on `refactor-p1-builtins`. There is one `GetTileByPixel`, in `gb_std`
(`src/gb_std/graphics/utility.rs:129`), with the contract of the `rust_boy` copy, which `unbricked_rustboy` and
the `Call` doc example rely on: in, `b` = X and `c` = Y (pixels on the `$9800` map, as `get_pivot` loads them);
out, `hl` = the address of the tile and `a` = the tile index (`[hl]`); it changes `bc` and the flags and keeps
`de` (documented on the function). `BuiltinFunction::generate` (`src/rust_boy/functions.rs:60`) returns the
`gb_std` routine for Memcopy, WaitVBlank, WaitNotVBlank, UpdateKeys and GetTileByPixel (the other four were
identical), and `TileManager` copies with `gb_std`'s `cp_in_memory`; only `Delay` is `rust_boy`'s own. `gb_std`'s
Memcopy passes its registers typed instead of as strings (same text). Every caller follows the contract:
`unbricked_std` drops its four `ld a, [hl]` after `call GetTileByPixel` (its only change: −4 bytes, +1 in the
routine; the same game in an emulator, 3000 frames compared, see the PR). **Pending, the maintainer's choice:**
`src/bin/unbricked.rs` still has its own copies of the routines, and its `GetTileByPixel` (`:345`) keeps the old
contract (`hl` only, no `a`; its callers load `[hl]` themselves). It is the tutorial written instruction by
instruction with `gb_asm` alone (README), a program of its own and not a layer, and it never links with the
library's routines; switching it to `gb_std`'s routine would make it a `gb_std` example. Tests run the routine on `gb_asm::test_cpu`
for 81 pixel positions (`test_get_tile_by_pixel_returns_the_address_and_the_tile`), the `gb_std` callers'
`get_pivot` → `GetTileByPixel` → tile test, and the `unbricked_rustboy` brick handler, which tests `a`
(`IfConst`, `IfA`) and blanks the brick through `hl` (`TileRef`); `test_builtins_are_the_gb_std_routines`
checks that each builtin is the `gb_std` routine. For them the test CPU now models numbers in register pairs
(`ld bc, $9800`, or both halves known), `add hl, rr`, 16-bit `inc`/`dec`, `srl`, `adc`, `or`, and 16-bit
constants (`TestCpu::consts16`).

#### B24
**All user functions are emitted, even unused.** (Lines at `4601a5c`.) `generate_all` (`src/rust_boy/functions.rs:133-147`) emits
every `user_functions` body; `used_user_functions` (`:71`) is written but never read. Note: filtering on it
today would break linking, because calls made through `Call`/`IfCall` are not tracked ([B26](#b26)) — fix B26 first.
**Status: fixed** on `refactor-p1-builtins`, with B26: `FunctionRegistry::generate_used`
(`src/rust_boy/functions.rs:275`) emits only the user functions the program refers to, from the start-up code,
the main loop, `raw()` code or the animation functions, then from those functions, and so on; in registration
order. A function called only from raw code (`raw()`, or an `Asm::raw` line, of one or several lines) needs
nothing. To emit one that only code `build()` does not
see calls (asm appended to its output, an `INCLUDE`d file), **`RustBoy::keep_function(name)`**
(`src/rust_boy/rustboy.rs:242`; it takes a builtin name too, and panics on an unknown name, like `call`);
`use_function(BuiltinFunction)` still forces a builtin. `used_user_functions` is gone. A function is found by
its label, so `define_function(name, body)` panics if `body` does not define the label `name` (a body labelled
otherwise used to be emitted anyway and could be called by its own label); another global label in a body (a
second entry point) also finds the function. `define_function` and `define_function_from` panic on a name that
is not an RGBDS identifier. `build()` panics if a user function's name is also defined elsewhere in the
program: a variable, a constant or label of its code (`define_const`, a `DEF`, a label in raw code), or an external
symbol (a function is defined either with `define_function`, or outside with `external_symbol`, not both); such a
function used to be dropped silently, and `call Jump` reached the constant or the variable (on `refactor`, rgbasm
reported the name defined twice). A user function that defines a builtin's name, as its own name or as a second
entry point (a routine bundle with an `UpdateKeys:` inside), always replaces the builtin, also when `use_function`
forces the builtin; the builtin's variables are then not created. `keep_function` on a function `build()`
generates (an animation) does nothing, since it is always emitted. No example changes (every example function is used, under its own name). Tests:
`test_only_used_user_functions_are_emitted` (unused, an unused cycle, recursive, through another function, by
address, by `jp`, from raw code, from a raw line after a comment and after a `;` in a string),
`test_keep_function_emits_a_function_nothing_calls`, `test_keep_function_needs_a_function`,
`test_define_function_needs_its_label`, `test_function_name_must_be_an_identifier`,
`test_a_second_entry_point_of_a_function_is_found`, `test_a_function_named_like_a_constant_panics` (and
`_a_variable_`, `_a_raw_label_or_def_`, `test_a_function_cannot_be_external`),
`test_a_user_function_replaces_a_forced_builtin`, `test_a_second_entry_point_replaces_a_builtin`,
`test_keep_function_accepts_a_generated_function`, `test_redefining_a_function_moves_its_second_entry_point`.

#### B25
**Animation labels are not namespaced by sprite.** `Anim_{name}` and `.skip_{name}`
(`src/rust_boy/sprites.rs:632, 685-686`; `:744, 797-798` at `7b54900`): two sprites that both have a `"Spin"`
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
(`src/gb_std/flow/emittable.rs:48`) and `IfCall::emit` (`src/gb_std/flow/flow_if.rs:903-943`) just emit
`call X`; only `RustBoy::call`/`call_args`/`use_function` register builtins (`src/rust_boy/functions.rs:101-115`),
and `define_function_from`/`add_to_main_loop` (`src/rust_boy/rustboy.rs:235, 457`) don't scan bodies. Following
the `Call` doc example alone (`Call::with_args("GetTileByPixel", ..)`) → `call GetTileByPixel` with no routine
→ rgblink "undefined symbol". `unbricked_rustboy` works only because it also calls `gb.call_args("GetTileByPixel", ..)`.
*Fix:* routines as values with dependencies, or scan emitted `Call` targets in `build()`.
**Status: fixed** on `refactor-p1-builtins` by scanning in `build()` (routines as values stay in Phase 2). The
Functions chunk is worked out once all the code is known (`src/rust_boy/rustboy.rs:512-550`): the global
symbols of every other chunk and of the animation functions are looked up (`symbols`,
`src/rust_boy/functions.rs:395`, reads the text of each instruction line by line as RGBDS does, with
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
**`RustBoy::external_symbol(name)`** (`src/rust_boy/rustboy.rs:277`): a function of that name is never emitted,
nor the variables of a builtin of that name. Then the variables the emitted builtins need
(`BuiltinFunction::variables`: `wCurKeys`, `wNewKeys` for `UpdateKeys`) are created, unless the program already
defines them (as variables, of any type, in raw code, or as external symbols), before the variable initialisation
and the Data chunk are emitted (`:552-566`), so `UpdateKeys` called without `add_inputs` links too. The scan is
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

#### B27
**`Memcopy` with length 0 copies 64 KiB.** (Lines at `4601a5c`.) Memcopy is a do-while loop (`src/gb_std/graphics/utility.rs:54-70`;
`src/rust_boy/functions.rs:152-170`); `BC = 0` wraps to `$FFFF` and overwrites WRAM, the stack, I/O and IE.
Reachable through `cp_in_memory` (`src/gb_std/graphics/utility.rs:45`) or `generate_memcopy_calls`
(`src/rust_boy/tiles.rs:252`) with an empty tile set / empty `.2bpp`. *Fix:* skip empty blobs at generation
time, or test `BC` before the first copy.
**Status: fixed** on `refactor-p1-builtins` at generation time: `TileManager::generate_memcopy_calls`
(`src/rust_boy/tiles.rs:284`) skips an empty blob of raw data (`from_raw` with no tiles, a tilemap with no rows).
Its labels are still emitted, with nothing between them, so code that names them still links; with no copy left,
`Memcopy` is not emitted at all (B26). A file blob (`INCBIN`) is always copied whole, as before: its size is only
known once assembled, so `TileSource::from_file(path, 0)` now panics (a user error) instead of being taken for an
empty blob, and so does adding a `TileSource::File(path, 0)` built directly (`add_sprite`, `add_background`). `Memcopy` itself is unchanged, so no ROM changes. Not covered: `gb_std`'s `cp_in_memory` only knows
labels and cannot see an empty blob (its doc and `memcopy`'s now say the length must be at least 1), and an empty
`.2bpp` file, or one shorter than the tile count given to `from_file`, is not checked ([B17](#b17)); testing `BC`
in `Memcopy` would cover these, at 3 bytes and a few cycles per call. Tests: `test_empty_blobs_are_not_copied` runs
the start-up code with the real `Memcopy` on `gb_asm::test_cpu` (blob lengths from `TestCpu::consts16`): before, the
first empty blob made it copy past its data; `test_a_tile_file_needs_tiles`,
`test_a_tile_file_built_directly_needs_tiles`.

#### B28
**OBP1 never initialised; OAM not cleared without sprites.** (Lines at `4601a5c`.) Only `rBGP` and `rOBP0` are written
(`src/rust_boy/rustboy.rs:316-320`), so sprites with the OBP1 palette flag get a random palette on DMG. OAM is
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
- `src/gb_std/inputs.rs:54` says `wCurKeys` "0 = pressed"; after the `xor` it is 1 = pressed.
- `RustBoy::call` doc example `gb.add_to_main_loop(gb.call("X"))` (`src/rust_boy/rustboy.rs:202, 206` at `4601a5c`; the corrected example is at `:257, 262`) does
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
| `Functions`/`Tiles` chunks have no `SECTION` and land in the wrong section | Only reachable via `raw()` opening a section → covered by [B15](#b15). Sections become first-class in Phase 2. |
| `Asm::ds` always emits a fill byte, so RAM can't be reserved | True, but a missing feature, not a bug (Phase 2: sections / `ds n` without fill). |
| Everything in one `ROM0[$100]` section fails above 16 KB | Tile data is copied to VRAM anyway; ROM banking is a feature (Phase 3: MBC). |
| `If` comparisons are unsigned while `i8` vars exist | Native `cp` semantics, documented in `flow_if.rs:8-15`; only a doc note ([B29](#b29)). |
| `enable_animation` on a sprite without animations references an undefined variable | API misuse; covered by "fail loudly" in [B20](#b20). |
| 8×16 tile indices not aligned after an odd-sized 8×8 sprite | Only happens when mixing sizes; since [B4](#b4) the size is set once, before the first sprite. |
| `0x05` constants require RGBDS ≥ 0.9 | The project toolchain is RGBDS 1.0; handled by pinning the version in CI. |
| `TileSource::from_file` tile count not checked against the file | Caller error; became a feature (derive the count from the file size). |

---

## 5. Missing features (summary)

Detailed list in [`Task.md`](Task.md) Phase 3. Biggest gaps:

- **Audio: nothing at all** (no APU registers, no sound effects, no music driver).
- **Graphics:** no shadow OAM/DMA, no scrolling, no window layer, no palette API/fades, no
  metasprites beyond 16×16, no text/numbers, no `$9C00` map.
- **Animation:** global speed only; no events (`Loop`, `PingPong` and `Once` work since [B10](#b10)).
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
