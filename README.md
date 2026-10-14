# rust-boy

A Rust DSL that generates Game Boy assembly for [RGBDS](https://rgbds.gbdev.io/). You describe the game
in Rust, `cargo run` prints a `.asm` file, and RGBDS turns it into a ROM.

```text
your_game.rs ──cargo run──▶ main.asm ──rgbasm / rgblink / rgbfix──▶ main.gb
```

> **Status:** experimental, and being refactored. The plan is in [Task.md](Task.md); the design notes and
> the list of known bugs are in [CONTEXT.md](CONTEXT.md).

## Three levels

| Module | Level | What it gives you |
|---|---|---|
| `rust_boy::engine` | engine | `RustBoy`: sprites (OAM), tiles (VRAM), variables (WRAM and HRAM), joypad bindings, animations and functions. `build()` writes the whole program. |
| `rust_boy::stdlib` | routines | Ready-made routines as values (Memcopy, WaitVBlank, UpdateKeys, GetTileByPixel, …, each a `Routine` with its dependencies and calling convention) and control flow (`If`, `IfConst`, `IfA`, `IfCall`). |
| `rust_boy::asm` | assembly | `Asm` (a whole program) and `Block` (a piece of code): one method per instruction or directive, with typed operands (`R8`, `R16`, `Mem`) and expressions (`Expr`), printed in RGBDS syntax. |

Each level is built on the one below it, and you can mix them. Beside them, `rust_boy::hw` holds the hardware facts
as plain data: the registers, their flags, the memory map, the OAM layout and the screen sizes, each with its
`hardware.inc` name (`hw::LCDC` is `rLCDC`, `$FF40`). `use rust_boy::prelude::*` brings the types most programs use
from every level, and `hw`.

The [changelog](CHANGELOG.md) lists every breaking change of the refactoring, with how to migrate.

## Requirements

- Rust 1.85 or newer (edition 2024). One Rust dependency: the pure-Rust [`png`](https://crates.io/crates/png)
  decoder, for `TileSource::from_png` (it runs in the generating program, not on the Game Boy).
- [RGBDS](https://rgbds.gbdev.io/) 0.9 or newer to build ROMs (CI uses 1.0.4).

To use it in your own project:

```toml
[dependencies]
rust-boy = { git = "https://github.com/ffex/rust-boy" }
```

## Quick start (`RustBoy`)

A sprite that moves with the D-pad:

```rust
use rust_boy::prelude::*;

fn main() -> Result<(), Error> {
    // Sprites are 8x8 by default; this program uses 8x16 ones
    let mut gb = RustBoy::with_config(RustBoyConfig::default().sprite_size(SpriteSize::Size8x16));

    // An 8x16 sprite (two tiles from a .2bpp file) at screen position (80, 72)
    let player = gb.add_sprite("Player", TileSource::from_file("player.2bpp", 2), 80, 72, 0);

    // Move one pixel per frame while a direction is held.
    // Limits are OAM coordinates (screen X + 8, screen Y + 16) and are included: a move
    // stops exactly on its limit, so here the sprite stays fully on screen.
    let mut inputs = InputManager::new();
    inputs.on_press(PadButton::Left, gb.sprites.move_left_limit(player, 1, 8));
    inputs.on_press(PadButton::Right, gb.sprites.move_right_limit(player, 1, 160));
    inputs.on_press(PadButton::Up, gb.sprites.move_up_limit(player, 1, 16));
    inputs.on_press(PadButton::Down, gb.sprites.move_down_limit(player, 1, 144));
    gb.add_inputs(inputs);

    // The program as RGBDS assembly; an `Err` says what is wrong with the program as a
    // whole (a name defined twice, variables that do not fit in WRAM, ...)
    println!("{}", gb.build()?);
    Ok(())
}
```

`on_press` keeps its original meaning: the action runs every frame the button is held. For edge-triggered actions,
use `on_just_pressed` or `on_release`; several bindings can target the same button. `on_repeat(button, delay, interval,
action)` runs once on press, then repeats after `delay` held frames and every `interval` frames. Both repeat counts must
be between 1 and 255.

## Routines

A routine is a value, `stdlib::routine::Routine`: its name, its code, the routines it depends on, the WRAM variables
it needs, and its calling convention, the registers it reads, returns and clobbers (every other register is kept).
The `stdlib` routines are `Routine`s (`memcopy()`, `wait_vblank()`, `update_keys()`, `get_tile_by_pixel()`,
`delay()`, …), and so is a routine of your own. `RustBoy` emits a routine, with its dependencies and variables, when the
program uses it:

```rust
use rust_boy::asm::{Block, R16};
use rust_boy::stdlib::graphics::utility::memcopy;
use rust_boy::stdlib::routine::{Regs, Routine};
use rust_boy::engine::RustBoy;

// Memcopy reads bc, de and hl, returns them moved past the copy, and clobbers a and the flags
assert_eq!(memcopy().clobbers(), Regs::A | Regs::F);

// A routine that needs Memcopy
let mut body = Block::new();
body.label("CopyScore")
    .ld(R16::DE, "wScore")
    .ld(R16::HL, "wSavedScore")
    .ld(R16::BC, 2)
    .call("Memcopy")
    .ret();
let copy_score = Routine::new("CopyScore", body)
    .with_dep(memcopy())
    .with_clobbers(Regs::ALL);

let mut gb = RustBoy::new();
gb.vars.create_u16("wScore", 0);
gb.vars.create_u16("wSavedScore", 0);
let call = gb.call_routine(&copy_score); // `call CopyScore`, and the program gets the routine
gb.add_to_main_loop(call);
let out = gb.build()?;
assert!(out.contains("CopyScore:") && out.contains("Memcopy:"));
# Ok::<(), rust_boy::engine::Error>(())
```

Control flow uses the same model: `If` uses `a`, `b` and the flags (`If::clobbers()`; the bodies start with `a` = left
and `b` = right), `IfConst` `a` and the flags, `IfA` and `IfCall` the flags. Left code that may change `b` (a call, for
example) is wrapped in `push bc` / `pop bc`, so the compare always reads the right value.

## Frames and interrupts (`RustBoy`)

A `RustBoy` program runs once a frame, woken by the VBlank interrupt:

- the start-up code turns interrupts off, sets everything up, enables the interrupts that have a handler in `rIE`,
  drops the requests made meanwhile (`rIF` = 0) and turns interrupts on (`ei`);
- the **VBlank handler** runs at the start of each VBlank (line 144): with sprites, first the OAM DMA that copies the
  shadow OAM to OAM (see [Sprites](#sprites-and-the-shadow-oam-rustboy)), then the optional VRAM queue flush,
  then the code given to `on_vblank`, then it
  tells the main loop that a frame started;
- the **main loop** waits for that asleep (`call WaitFrame`, a `halt`), then runs the animations and the
  `add_to_main_loop` code, so it starts in VBlank too, once a frame (how much of VBlank is left depends on the
  handler: see [Queued VRAM writes](#queued-vram-writes-rustboy)'s cost);
- the **other handlers** (`on_stat`, `on_timer`, `on_joypad`, or `on_interrupt` for any of them) run when their
  interrupt fires. Every handler saves the registers its code changes (what the code writes, or a routine's calling
  convention with `on_interrupt_routine`) and returns with `reti`, so the code it interrupts is not disturbed.

```rust
use rust_boy::prelude::*;

let mut gb = RustBoy::new();
let ticks = gb.vars.create_u8("wTicks", 0);

// 64 times a second: 64 steps of the 4096 Hz clock, from 192 to 255
let mut tick = Block::new();
tick.ld_a_addr_def(ticks.name()).inc(R8::A).ld_addr_def_a(ticks.name());
gb.on_timer(Timer::new(TimerClock::Hz4096, 192), tick);

// On line 100: change the background scroll for the bottom of the screen
let mut split = Block::new();
split.ld_a(8).ldh(Mem::addr(hw::SCX), R8::A);
gb.on_stat(Stat::new().line(100), split);

// Hold A + B + Select + Start to start the game again
gb.set_soft_reset(true);

let out = gb.build()?;
assert!(out.contains("TimerHandler:\n    push af\n    ld a, [wTicks]"));
# Ok::<(), rust_boy::engine::Error>(())
```

`enable_interrupt` / `disable_interrupt` give the code that switches one on or off while the program runs, and
`stdlib::interrupts` has the pieces for a program written with `stdlib` (vectors, handlers, `WaitFrame`, the
`rSTAT` and timer set-up). The **soft reset** jumps to the entry point (`$0100`), so the whole start-up runs again,
as at power-on: every variable is set to its initial value, the `init()` code runs again; it happens once per press
(holding the four buttons does not reset the game again and again).

## PNG and binary tile assets (`RustBoy`)

Use `TileSource::from_png(path, &options)` to load tile artwork directly. Palette slots
are pixel indices 0..=3; default colors are `[255,255,255]`, `[170,170,170]`, `[85,85,85]`,
and black. Exact matching rejects unknown colors; nearest RGB matching is explicit.
Fully transparent pixels map to index zero by default, and partial alpha is rejected.

```no_run
use rust_boy::prelude::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = PngOptions::default()
        .mapping(PngColorMapping::Nearest)
        .tile_order(TileOrder::Sprite8x16);
    let artwork = TileSource::from_png("player.png", &options)?;
    let mut gb = RustBoy::with_config(RustBoyConfig::default().sprite_size(SpriteSize::Size8x16));
    gb.add_sprite("Player", artwork, 80, 72, 0);
    println!("{}", gb.build()?);
    Ok(())
}
```

PNG dimensions must be multiples of eight; 8×16 ordering additionally requires a
height divisible by sixteen and emits each vertical pair consecutively. Indexed,
grayscale, RGB, alpha, and interlaced static PNGs are supported. Packed samples expand
to 8 bits; 16-bit samples use their high byte. Mapping does not apply gamma correction,
dithering, deduplication, padding or cropping. Animated PNGs are rejected.

`png_to_2bpp(&png_bytes, &options)` returns binary bytes that you can write as a `.2bpp`
file. `TileSource::from_2bpp(path)` reads a nonempty multiple of sixteen bytes and derives
the count; `from_2bpp_bytes(&bytes)` does the same for memory. Loaded sources own their
data and emit DW pairs, so no asset INCBIN include path is needed. Paths resolve relative
to the generating program's current directory; use `CARGO_MANIFEST_DIR` for crate assets.
All loaders return `AssetError`; allocation limits remain the tile manager's existing
128 background / 256 sprite tiles. The legacy `from_file(path,count)` remains available
for deferred RGBDS INCBIN, where you supply the correct count and include directory.

## Sprites and the shadow OAM (`RustBoy`)

The PPU reads OAM, the sprite table, during most of each visible line: a CPU write to OAM then is lost. So a `RustBoy`
program never writes OAM itself: its sprites live in a copy in WRAM, the **shadow OAM** (`wShadowOAM`, 160 bytes on the
last page of WRAM0), which the moves, the animations and `get_x` / `get_y` / `get_pivot` read and write at any time. At
the start of each VBlank the VBlank handler copies it to OAM with an **OAM DMA** (160 M-cycles), through the usual
routine run from HRAM (`hOAMDMA`, the top 8 bytes of the HRAM variables' 64), which the start-up copies there. A sprite
moved in one frame shows from the frame after next; the start-up runs one DMA before the LCD is turned on, so the
first frame shows the sprites. A program without sprites has neither. `stdlib::graphics::oam` has the same pieces
for a `stdlib` program (`Sprite::in_shadow_oam`, `init_oam_dma`, `run_oam_dma`, the sections).
Two limits: the stack (from `$FFFE` down) has 62 bytes above `hOAMDMA` (`$FFB8`-`$FFBF`), so code nested more than 31
calls or pushes deep overwrites the DMA routine; and the DMA runs when the VBlank handler does, so an interrupt handler
(`on_stat`, `on_timer`) that delays it past VBlank makes it run during the next frame's drawing, where it fails for
that frame.

### Typed sprite attributes

```rust
use rust_boy::prelude::*;

let mut gb = RustBoy::new();
let flags = SpriteFlags::new().flip_x(true)
    .palette(ObjectPalette::Palette1)
    .priority(SpritePriority::BehindBackground);
let sprite = gb.add_sprite("Player", TileSource::from_raw(&[["$ff"; 8]]), 24, 32, flags);
gb.add_to_main_loop(gb.sprites.set_flags(sprite, flags.flip_x(false).flip_y(true)));
# gb.build().unwrap();
```

Runtime attributes live in shadow OAM and appear after the next DMA; they leave
position and tile indices intact. Raw `u8` flag values remain accepted. Behind-background
priority hides the sprite behind nonzero background/window colour indices; colour 0
stays behind the sprite. These flips affect individual hardware sprites, not the
arrangement of a whole composite: a 16x16 composite flipped in X mirrors each
half in place, so swap the halves' tiles too. Change a composite's attributes per member sprite
(`get_composite_sprites`).

For tile collision after scrolling, use `GetTileByScreenPixel` with a screen-space position. It adds the current
SCX/SCY values and then performs the same active-map lookup as `GetTileByPixel`; call it in VBlank or with the LCD
off so VRAM is readable:

```rust,ignore
gb.call_args("GetTileByScreenPixel", gb.sprites.get_pivot(ball, 0, 1));
```

Startup resets the camera to (0, 0), including after soft reset. Put a custom `set_background_scroll` call in `init`
when the game should start elsewhere.

### Arbitrary metasprites

A metasprite is a list of hardware parts relative to a screen anchor. Parts may have
negative offsets and independent tile data/attributes. They take OAM entries in list order.
On DMG, where two parts overlap, the one with the smaller X draws on top and the list order only breaks ties; at most 10 parts show on one line, the first ones in list order.

```rust
use rust_boy::prelude::*;

let mut gb = RustBoy::new(); // 8x8 hardware parts; 8x16 is also supported
let tiles = TileSource::from_raw(&[["$ff"; 8]]);
let parts = [
    MetaspritePart::new(tiles.clone(), -8, 0, 0u8),
    MetaspritePart::new(tiles.clone(), 0, 0, 0u8),
    MetaspritePart::new(tiles.clone(), 8, 0, 0u8),
    MetaspritePart::new(tiles, 0, 8, 0u8),
];
let actor = gb.add_metasprite("Actor", 80, 72, &parts);
gb.add_to_main_loop(gb.sprites.move_composite_left_limit(actor, 3, hw::OAM_X_OFFSET));
# gb.build().unwrap();
```

The leading part stops at the included OAM limit and the others keep their offsets.
Every initial part position must fit the existing screen coordinate range (X 0..=247,
Y 0..=239); the constructor checks layout and capacity before registration. Hardware
parts share the configured size, and each 8x16 frame requires two aligned tiles.
The older `add_sprite_16x16` helper keeps its existing labels and output.

### Per-animation timing

The global animation period is eight game frames by default. Override a particular
animation while leaving other sprites on that shared clock:

```rust
use rust_boy::prelude::*;
let mut gb = RustBoy::new();
let actor = gb.add_sprite("Actor", TileSource::from_raw(&[["$00"; 8]; 4]), 40, 40, 0);
let walk = gb.sprites.add_animation(actor, "Walk", 0, 3, AnimationType::PingPong);
gb.sprites.set_animation_period(actor, walk, 3);
gb.sprites.set_initial_animation(actor, walk);
let asm = gb.build()?;
# Ok::<(), rust_boy::engine::Error>(())
```

Periods are 1..=255 frames. A sprite with any override uses its own clock; animations
on that sprite without an override use the global period through that clock. Switching
selection resets the clock when the dispatcher observes it. Repeated enable calls for
the selected animation keep its progress; a disabled frame stops/resets it. Existing
frame stepping is unchanged, so selecting an animation does not immediately replace
the displayed tile. Direct calls to animation functions bypass dispatcher timing.

Use `set_composite_animation_period(composite, index, frames)` to synchronize all
metasprite parts. Clear overrides with `clear_animation_period` or
`clear_composite_animation_period`. These are generation-time settings, with two WRAM
bytes per independently timed sprite. Programs without overrides retain global timing.

### Animation completion and replay

Subscribe to a polling flag, a named callback, or both. Callbacks run in the main loop
after all sprites have stepped, before other game logic; BC/DE/HL are preserved.

```rust
use rust_boy::prelude::*;
let mut gb = RustBoy::new();
let actor = gb.add_sprite("Actor", TileSource::from_raw(&[["$00"; 8]; 4]), 40, 40, 0);
let attack = gb.sprites.add_animation(actor, "Attack", 0, 2, AnimationType::Once);
let idle = gb.sprites.add_animation(actor, "Idle", 3, 3, AnimationType::Loop);
gb.sprites.set_animation_period(actor, attack, 2);
gb.sprites.set_animation_callback(actor, attack, "AttackDone");
gb.sprites.set_initial_animation(actor, attack);
// Configure subscriptions/timing before generating restart snippets.
gb.define_function_from("AttackDone", gb.sprites.restart_animation(actor, idle));
let observed = gb.vars.create_u8("wAttackObserved", 0);
gb.add_to_main_loop(gb.sprites.take_animation_event(actor, attack));
gb.add_to_main_loop(IfA::eq(1, observed.set(1)));
let asm = gb.build()?;
# Ok::<(), rust_boy::engine::Error>(())
```

`Once` emits once when it reaches its last frame, including a first update that is
already on that frame. Changing selection away and back rearms it, but preserves the
existing tile/replay rules. `Loop` emits on last→first wrap; `PingPong` on returning to
the first frame. Single-frame loops/ping-pong emit every update after the first (entering one
from another tile is not a completion); single-frame Once
emits once. Events describe shadow OAM, which becomes visible at the following DMA.

`enable_animation_events` enables polling without a callback. `take_animation_event`
returns A = 0/1 and Z for no event, then clears the flag; multiple pending completions
coalesce. Callbacks do not clear that polling flag. Callback functions can be defined
after binding, or declared external. Use the composite versions of these methods for
one event/callback per logical metasprite.

`restart_animation` and `restart_composite_animation` immediately show the first frame
and clear that animation's pending event/callback and Once latch. Direction and any
independent timer reset; shared-clock sprites advance on the next global tick. Repeated
enable calls keep their existing behavior and do not replay a finished Once. Event
producers, polling and restart code belong in init/main, with one main-loop consumer.
Handler calls to event-producing animation functions are rejected; raw RAM access and
poll/restart snippets are caller contracts.

### Sprite visibility and reusable OAM slots

Static sprites have `gb.sprites.hide(id)` and `show_at(id, x, y)` snippets; composite
helpers apply to every part. Hiding sets OAM Y to 0 and preserves X/tile/flags.
Show supplies a screen position; changing Y while hidden can make a sprite visible,
so keep logical positions in your game state. Runtime operations keep startup data intact.

```rust
use rust_boy::prelude::*;

let mut gb = RustBoy::new();
let pool = gb.create_sprite_pool("Actors", 8);
let mut spawn = Block::new();
let full = gb.labels().local("pool_full");
spawn.emit_all(pool.allocate()).jr_cond(Condition::C, &full)
    .ld_b(40).ld_c(60).ld_d(0).ld_e(0)
    .emit_all(pool.write()).label(&full);
gb.init(spawn);
# gb.build().unwrap();
```

Register tile data separately before displaying pooled sprites. Allocation returns A
as a relative slot index and HL as its cleared, hidden shadow-OAM entry. Full pools
return A=255 and carry. `write` uses A=index, B=X, C=Y, D=tile, E=attributes; `hide`
keeps the slot allocated, while `release` clears it and permits reuse. Invalid/free
indices and invalid coordinates set carry without writes; 8x16 writes reject odd tiles.
All pool operations belong in init/main, with one producer; build rejects handler
calls. Allocation changes AF/BC/HL, other pool operations AF/HL; DE and IME are kept.
Pool writes publish Y last but aren't atomic to DMA: use VBlank for coherent updates.

### Entity pools

`create_entity_pool` gives a sprite pool entity names: one hardware sprite per entity, its
index in A, the game keeping any other state.

```rust
use rust_boy::prelude::*;

let mut gb = RustBoy::new();
let enemies = gb.create_entity_pool("Enemies", 4);
let mut spawn = Block::new();
let full = gb.labels().local("no_enemy");
spawn.emit_all(enemies.spawn()).jr_cond(Condition::C, &full) // A = the new entity
    .ld_b(80).ld_c(40).ld_d(0).ld_e(0)
    .emit_all(enemies.set_sprite()).label(&full);
gb.init(spawn);
# gb.build().unwrap();
```

`spawn` returns a hidden entity (carry when the pool is full), `set_sprite` writes and shows
it (A = entity, B/C = position, D/E = tile/flags), `hide` keeps it alive and `despawn` frees it.
Registers, carry results and the init/main-only rule are the pool's (above).

## WRAM arrays (`RustBoy`)

```rust
use rust_boy::prelude::*;

let mut gb = RustBoy::new();
let scores = gb.vars.create_array("wScores", ArrayElement::U16, 300, 100);
gb.init(scores.set(299, 65535)); // static index and value checked here
let mut code = Block::new();
code.ld(R16::BC, 256u16).ld(R16::DE, 1234u16)
    .emit_all(scores.set_indexed(gb.labels()));
gb.add_to_main_loop(code); // carry set if BC >= 300; nothing written on rejection
# gb.build().unwrap();
```

Arrays occupy contiguous WRAM0 bytes and share the variable allocator. Initialization
repeats the supplied value with a compact loop. Byte elements use `U8`/`I8`, words
`U16`/`I16` and little-endian storage. Runtime getters return A for bytes or HL for words;
setters take E or DE, preserve BC/DE, and change A/HL/flags. `address_indexed` returns
HL and carry status for custom operations; branch on carry before dereferencing it.
Static operations panic on invalid indices/values; runtime bounds rejection sets carry.

## Window (`RustBoy`)

Configure a window with `Window::new(map, x, y)`; use `Window::from_raw(map, wx, wy)` for clipping or off-screen
coordinates. It is visible at startup by default, or use `.visible_on_startup(false)` to show it later with
`show_window(true)`. On DMG, `Lcdc::background(false)` disables both the background and window. Startup configuration
owns the final LCDC value; run runtime visibility changes in VBlank.

## Queued VRAM writes (`RustBoy`)

Tilemap edits and score digits can be queued from game code without waiting for the PPU:

```rust
use rust_boy::prelude::*;

let mut gb = RustBoy::new();
let write = gb.queue_vram_write(hw::SCRN0, 7); // put tile 7 at the map's first cell
gb.add_to_main_loop(write);
let out = gb.build()?;
assert!(out.contains("FlushVramQueue:"));
# Ok::<(), Error>(())
```

The first call enables a FIFO of **12 pending byte writes**, using 41 bytes of WRAM0 without moving user
variables. The VBlank handler flushes it **after OAM DMA and before user `on_vblank` code**. Entries are written
in order, so the last queued write to one address wins. `queue_vram_write_from_registers()` takes the destination
in `hl` and the byte in `a`, useful after `TileRef::load_address()` or a runtime calculation.

Both helpers generate code that changes all registers and flags and preserves IME. Carry is clear when accepted;
carry is set when the queue is full or a runtime address is outside VRAM. A rejected write is not queued: retry it
later if it must arrive. Constant addresses outside VRAM and values outside a byte panic at the API call. Producer
code must run only in initialization or the main loop, **never in an interrupt handler**: `build()` returns
`Error::MainLoopOnly` when handler code reaches `QueueVramWrite`, directly or through a function (labels of `raw()`
code are not followed). `FlushVramQueue` has one caller, the VBlank handler: do not call it. VBlank can safely
interrupt it: the write cursor is published only when all entry bytes are ready.

The consumer begins writes only on lines 144–152, leaving the last VBlank line as headroom. A late handler defers
entries rather than writing during display rendering. On the test emulator, a full queue plus OAM DMA takes
967 M-cycles of handler work, within the 1,140 M-cycle VBlank window; arbitrary user handlers can still exceed it.
That time comes out of the main loop's VBlank. Measured on the test emulator, the main loop starts with about 920 M-cycles of VBlank left with a sprite
and an empty queue, but only about 150 with 12 queued writes: main-loop code that touches VRAM directly (`GetTileByPixel`,
`TileRef::set_tile`) then runs while the screen is drawn, where VRAM reads return `$FF` and writes are lost. A program
that fills the queue should send all its VRAM writes through it: the `on_vblank` code runs after the flush, so it has
no more VBlank left than the main loop.
`stdlib::graphics::vram_queue` provides the routines, initialization and RAM section for standalone programs.
Programs that never request the queue emit no queue code or storage.

## Text and scores (`RustBoy`)

Load one 8×8 tile per consecutive ASCII character, then call each printer's `step` in
the main loop. Long strings resume across frames using the VBlank VRAM queue (12 pending
writes). Carry set means retry next frame; carry clear means queued, with display updates
at a subsequent VBlank. Enable the background/window for the selected map in your config.

```rust
use rust_boy::prelude::*;
let mut gb = RustBoy::new();
// Supply real glyph tiles here, or TileSource::from_file("digits.2bpp", 10).
let font = gb.load_font("Digits", TileSource::from_raw(&[["$00"; 8]; 10]), b'0');
let text = gb.create_text_printer("Message", font, TilemapArea::Map9800, 0, 0, "0123456789");
let score = gb.create_number_printer("Score", font, TilemapArea::Map9800, 0, 1, NumberFormat::DecimalU16);
let mut init = Block::new();
init.ld(R16::DE, 12345).emit_all(score.set_value(gb.labels()));
gb.init(init);
gb.add_to_main_loop(text.step()).add_to_main_loop(score.step());
let asm = gb.build()?;
# Ok::<(), rust_boy::engine::Error>(())
```

Coordinates are tiles in the 32×32 map. Wrapping/newlines return to the initial X column;
unsupported characters and writes beyond the map panic. Decimal uses five digits, packed
BCD four. Invalid BCD keeps the old score and returns carry. Set numbers only when they
change; resetting/replacing retains pending writes, so partial values can briefly display.
Printer steps belong in init/main code; interrupt handlers cannot produce queue entries.

## Background tile animation (`RustBoy`)

Preload tile artwork, then animate its IDs at fixed cells on either background/window map.
Each frame supplies one tile per cell, in cell order. Call `step` once per game frame:

```rust
use rust_boy::prelude::*;
let mut gb = RustBoy::new();
gb.tiles.add_background("WaterTiles", TileSource::from_raw(&[["$00"; 8]; 3]));
let config = BackgroundAnimationConfig::new(
    TilemapArea::Map9800, &[(2, 3), (3, 3)],
    &[vec![0, 1], vec![1, 2], vec![2, 0]],
).delay(8).playback(AnimationType::PingPong);
let water = gb.create_background_animation("Water", config);
gb.add_to_main_loop(water.step());
let asm = gb.build()?;
# Ok::<(), rust_boy::engine::Error>(())
```

Loop/ping-pong/once support 1..=256 frames. Holds are 1..=255 calls after a frame is
fully queued (default 1). A full VBlank queue pauses advancement, preserving the current
frame's cursor. Carry set means queue full. Cells can become visible in pieces as the
queue drains. Pause/resume retain progress; restart replays frame zero and keeps pending
writes. `finished()` loads A with 1 once Once's final frame is queued, otherwise 0.
The selected layer must be enabled separately; tile patterns are loaded beforehand.

## Palettes and fades (`RustBoy`)

DMG palettes map four colour indices to shades. The background and window share BGP;
object index 0 stays transparent. Startup `Palettes` accepts typed `Palette` values or raw `u8` hardware bytes.
Use typed palettes for runtime writes and `PaletteFade` for nonblocking fade-in/out:

```rust
use rust_boy::prelude::*;

let mut gb = RustBoy::new();
let step = gb.vars.create_u8("wFadeProgress", 0);
let fade = PaletteFade::fade_in(Palette::default(), Shade::White);
let mut apply = Block::new();
apply.emit_all(step.get())
    .emit_all(gb.fade_palette(PaletteRegister::Background, fade));
gb.on_vblank(apply);
// Advance wFadeProgress from 0 to 3 at your chosen frame interval.
// Values >=3 keep the final palette; reset to 0 to restart.
# gb.build().unwrap();
```

`PaletteFade::fade_out(palette, Shade::Black)` fades to black; either direction also
supports white or grey. Runtime snippets change A and flags only; a direct
`gb.set_palette(register, palette)` write preserves flags. Emit in init or VBlank
for a stable whole-frame palette. Each colour moves at most one shade per stage.
A fade leaves the palette byte in A, not the progress: to fade several registers,
load the progress again before each `fade_palette`.

## Low level (`asm`)

The same building blocks the engine uses, one instruction at a time. An `Asm` is a program: it is printed in the
order it is written, and knows instructions and sections, not how a game is laid out (the engine puts its parts in
order with `engine::Layout`, by `Chunk`):

```rust
use rust_boy::asm::{Asm, Condition, Section};
use rust_boy::hw;

fn main() {
    let mut asm = Asm::new();

    // Cartridge header at $100-$14F: rgbfix fills in the logo and checksums
    asm.include_hardware()
        .section(Section::rom0("Header").at(0x0100))
        .raw("nop")
        .raw("jp EntryPoint")
        .ds_fill("$150 - @", "0");

    asm.section(Section::rom0("Main"))
        .label("EntryPoint")
        .label("MainLoop")
        .call("WaitVBlank")
        .jp("MainLoop")
        .blank_line(); // a blank line in the output

    asm.label("WaitVBlank")
        .ld_a_addr_def(hw::LY) // ld a, [rLY]
        .cp_imm(hw::SCRN_Y.value) // cp a, 144: the first line of VBlank
        .jr_cond(Condition::NZ, "WaitVBlank")
        .ret();

    println!("{}", asm.to_asm());
}
```

Operands are typed: registers are `R8` (`a` … `l`, and `[hl]`) and `R16`, memory is `Mem` (`[bc]`, `[de]`,
`[hli]`, `[hld]`, `[c]`, `[address]`), and values are `Expr`s: numbers, symbols (`hardware.inc` names, labels,
variables, `DEF` constants) and arithmetic on them. A Rust integer or a symbol name can be passed directly, and so
can a `hw` symbol, which is written by its `hardware.inc` name:

```rust
use rust_boy::asm::{Block, Expr, Mem, R8, R16};
use rust_boy::hw;

// A Block is a piece of code, built with the same methods as an Asm
let mut asm = Block::new();
asm.ld(R16::HL, Expr::from(hw::OAMRAM) + 4) // ld hl, _OAMRAM+4
    .ld(R8::A, Mem::addr("wScore")) // ld a, [wScore]
    .add(R8::B) // add a, b
    .cp("BRICK_LEFT") // cp a, BRICK_LEFT
    .ld(Mem::Hli, R8::A) // ld [hli], a
    .ld(R8::A, Expr::from(hw::LCDCF_ON) | hw::LCDCF_BGON) // ld a, LCDCF_ON | LCDCF_BGON
    .sub(-1); // sub a, -1
let text: Vec<String> = asm.iter().map(|i| i.to_string()).collect();
assert_eq!(text[0], "ld hl, _OAMRAM+4");
assert_eq!(text[5], "ld a, LCDCF_ON | LCDCF_BGON");
```

A value is never a destination, so `asm.ld(1, 2)` does not compile. Operands that make no SM83 instruction
(`ld [hl], [hl]`, `ld b, [de]`), a value that does not fit (`ld a, 300`) and a register written as text
(`asm.cp("b")`) panic with a clear message. `Expr::raw("…")` passes any other RGBDS expression through as it is.

### Sections

Sections are typed: a `Section` has a name and a memory type (`ROM0`, `ROMX`, `VRAM`, `SRAM`, `WRAM0`, `WRAMX`,
`OAM`, `HRAM`), and optionally a fixed address, a bank, an alignment and the `UNION` / `FRAGMENT` modifiers. What
RGBDS would reject panics when the section is built: a bank on a memory type without banks (`ROM0`, `WRAM0`, `OAM`,
`HRAM`) or out of its range, an address outside the memory type, an alignment no address of it has, `UNION` in ROM.
A RAM section (every type but `ROM0` and `ROMX`) holds no code or data, it only reserves space: labels and `ds n`
(`asm.ds("2")`; `ds_fill(n, fill)` fills ROM). Code or data in a RAM section, or a section name used twice (except by
`UNION`s or `FRAGMENT`s of one memory type), panics where it is written (`Asm::emit`; `Asm::try_emit` returns it as an
`Err`); in a `RustBoy` program, `build()` returns `Error::Section`.

```rust
use rust_boy::asm::{Asm, MemoryType, Section};

let mut asm = Asm::new();
asm.section(Section::romx("Level 2").bank(2).align(8)) // SECTION "Level 2", ROMX, BANK[2], ALIGN[8]
    .label("Level2Map")
    .db("1, 2, 3");
asm.section(Section::wram0("Variables").at(0xC100)) // SECTION "Variables", WRAM0[$C100]
    .label("wScore")
    .ds("1");
asm.section(Section::new("Scratch", MemoryType::Hram).union()) // SECTION UNION "Scratch", HRAM
    .label("hTemp")
    .ds("2");
let text = asm.to_asm();
assert!(text.contains("SECTION \"Variables\", WRAM0[$C100]"));
assert!(text.contains("    ds 1\n"));
```

### Labels and jumps

Generated code never clashes with itself: every label it makes up (the `.end_if_N` of an `If`, key checks, sprite
moves, the animation dispatcher, …) is a local label numbered by one `LabelAllocator` per program, which the program's
`Asm` owns: `asm.labels()`, or `gb.labels()` in a `RustBoy` program (pass it to the `stdlib` snippets that take one,
such as `check_key`). Control flow (`If`, `IfConst`, `IfA`, `IfCall`) is `Emittable`: `asm.emit_code(code)` emits it
with the program's labels, and `RustBoy` does the same in `init`, `add_to_main_loop` and `define_function_from`.

Write `jr` where you expect a short jump: when the program is printed (`Asm::to_asm`, `RustBoy::build`), each `jr`
that cannot be shown to reach its target becomes a `jp`: more than 127 bytes ahead or 128 behind, in another section,
a symbol the program does not define as a label, or behind a line whose size only RGBDS knows (a raw line, an
`INCLUDE`). A `jr` that grows can push another one out of range, so this is repeated until every `jr` left reaches
its target. A `jp` is never shortened. The target of a jump written from `@` (`jr nz, @+4`, `jp @-3`) keeps
pointing at the same instruction: its offset is written again when a jump grows. If that cannot be worked out (an
offset into the middle of an instruction, a target such as `Label + 2`), or `@` is used anywhere else (a raw line,
`db`/`dw`, an operand such as `ld hl, @ + 5`; the padding `ds $150 - @` is fine), no jump is changed and the program
is printed as written, so rgbasm reports a `jr` out of range as it always did. A `jp` costs one byte and one cycle
more than a `jr`, so code whose size or timing is fixed should write jumps that reach.

```rust
use rust_boy::asm::{Asm, Block, Section};
use rust_boy::stdlib::flow::IfA;

let mut body = Block::new();
for _ in 0..200 {
    body.nop();
}
let mut asm = Asm::new();
asm.section(Section::rom0("Code")).label("Main").ld_a(1);
asm.emit_code(IfA::eq(1, body)); // its labels come from asm.labels()
asm.jr("Main"); // more than 200 bytes back
let text = asm.to_asm();
assert!(text.contains(".end_if_0:"));
assert!(text.contains("jp Main"), "printed as a jp");
```

## Building a ROM

One command builds an example (or any binary of the crate) into a ROM, with its symbol and map files, and with
`--run` opens it in an emulator:

```bash
scripts/run.sh fosdem          # target/examples/fosdem/main.gb, .sym, .map and .asm
scripts/run.sh --run fosdem    # ... then runs it
```

`--run` uses `$GB_EMULATOR` when it is set (a command, such as `GB_EMULATOR="wine $HOME/bgb/bgb.exe"`; a leading `~` works too), or else the
first emulator it finds: `sameboy`, `mgba-qt`, `mgba`, `gambatte_qt`, `gambatte_sdl`, `bgb`, `bgb64`, `bgb.exe` or
`bgb64.exe` through `wine`, `pyboy`; with none, it prints where the files are. `--emulator <command>` picks one for
one run. The `.sym` file sits next to the ROM, so emulators with a debugger (SameBoy, bgb, Emulicious) show the
program's labels. The steps it runs, by hand:

```bash
cargo run --bin fosdem > main.asm
rgbasm -I include -I examples/fosdem -o main.o main.asm   # include/: hardware.inc, examples/fosdem: .2bpp assets
rgblink -n main.sym -m main.map -o main.gb main.o
rgbfix -v -p 0xFF main.gb
```

Or build every example at once into `target/examples/<bin>/` (under `$CARGO_TARGET_DIR` if set):

```bash
scripts/assemble-examples.sh
```

CI does the same on every pull request and keeps the ROMs, with their `.sym` and `.map` files, as a downloadable
artifact (`example-roms`).
Open them in any Game Boy emulator.

The assembly each example generates is committed as `examples/<bin>/main.asm`, next to the example's assets.
`cargo test` compares it with what the binary prints (`tests/snapshots.rs`) and fails with a diff when they
differ. When a change of the generated code is intended, rewrite the snapshots and commit them with the change:

```bash
UPDATE_SNAPSHOTS=1 cargo test --test snapshots
```

### Headless-emulator tests

`tests/emulator.rs` builds ROMs with RGBDS and runs them on a small headless Game Boy written for the tests
(`tests/support/gameboy.rs`, no dependency): the SM83 CPU M-cycle by M-cycle, interrupts, `halt`, the timer, the PPU's timing
(modes, `LY`, `STAT`), OAM DMA, the joypad, MBC1/MBC5 ROM banking, but no picture. It blocks what the hardware
blocks (OAM in modes 2 and 3, VRAM in mode 3) and logs every OAM write with its PPU mode, and every OAM DMA; what it
does not model is
listed at the top of `gameboy.rs`. Each example is run
with scripted input and checked through its memory: the Unbricked ball moves and bounces and the paddle follows
the joypad, the FOSDEM player walks with its animation, the coin animates on A and stops on B. A test of your own:

```rust,ignore
// In tests/: `mod support;` gives the helpers; `game` is a RustBoy program
let rom = Rom::build(&game.build()?, &[]);    // rgbasm + rgblink + rgbfix, and the symbols
let mut gb = rom.boot();                      // a GameBoy at $0100
gb.run_script(&[(30, &[]), (20, &[Button::Right])]);  // 30 frames, then 20 holding Right
assert_eq!(gb.read_symbol("wScore"), 0);
assert!(gb.oam_entry(0)[1] > 24);             // OAM entry 0 (Y, X, tile, attributes) moved right
```

They need RGBDS (`RGBDS_LINK_CHECK=1`) and return early without it. The emulator itself is checked against
Blargg's test ROMs (`cpu_instrs`, `instr_timing`, `mem_timing`), fetched at a pinned commit:

```bash
scripts/fetch-test-roms.sh /tmp/gb-test-roms
RGBDS_LINK_CHECK=1 GB_TEST_ROMS=/tmp/gb-test-roms cargo test --test emulator
```

## Examples

| Binary | Level | What it shows |
|---|---|---|
| `basic_usage` | `asm` | A minimal program: header, main loop, VBlank wait |
| `unbricked` | `asm` | The [gbdev.io](https://gbdev.io/gb-asm-tutorial/) "Unbricked" tutorial, written instruction by instruction |
| `unbricked_std` | `stdlib` | The same game with `stdlib` routines and `If` |
| `unbricked_rustboy` | `engine` | The same game with `RustBoy` |
| `fosdem` | `engine` | A 16×16 walking character with four animations (FOSDEM demo) |
| `coin-anim` | `engine` | An animated coin: A starts the animation, B stops it |
| `music` | `engine` | A two-note song played by the pinned, offline hUGEDriver |

## What is supported

- **Instructions** (`asm`): the whole SM83 instruction set, printed in RGBDS syntax: loads (`ld`, `ldh`,
  `ld [hli]`/`[hld]`, `ld hl, sp + e`, `push`/`pop`), the 8-bit ALU on `a` (`add`, `adc`, `sub`, `sbc`, `and`, `xor`,
  `or`, `cp`, one source each: `asm.cp(144)` prints `cp a, 144`), `inc`/`dec`, `add hl, r16`,
  `add sp, e`, the rotates and shifts (`rlca`… and `rlc`, `rrc`, `rl`, `rr`, `sla`, `sra`, `swap`, `srl` on a register
  or `[hl]`, `R8`), `bit`/`set`/`res`, `daa`, `cpl`, `scf`/`ccf`, `nop`, `halt`, `stop`, `di`/`ei`, `jp`, `jr`,
  `call` and `ret` (all four also with the `z`/`nz`/`c`/`nc` conditions), `jp hl`, `reti`, `rst`, plus the directives
  `SECTION` (typed, see [Sections](#sections)), `INCLUDE`, `INCBIN`, `DEF … EQU`, `db`, `dw`, `ds` (with or without a
  fill value), labels, comments and raw lines. Operands are
  typed (see above): an operand the instruction does not take either does not compile (`ld 1, 2`, `inc 5`,
  `and a, hl`) or panics with a clear message (`ld [hl], [hl]`, `bit 8`, `rst $09`). Generated labels are unique,
  and a `jr` out of range becomes a `jp` (see [Labels and jumps](#labels-and-jumps)).
- **Engine** (`RustBoy`): VRAM layout for sprite and background tiles and tilemaps (`$9800`, `$9C00`), tiles
  converted from PNG or read from `.2bpp` files in Rust (see [PNG and binary tile assets](#png-and-binary-tile-assets-rustboy)), typed sprite attributes (flips,
  palette, priority; see [Typed sprite attributes](#typed-sprite-attributes)), runtime palette
  writes and fades (see [Palettes and fades](#palettes-and-fades-rustboy)), text and score printing (see
  [Text and scores](#text-and-scores-rustboy)), background tile animation (see
  [Background tile animation](#background-tile-animation-rustboy)), WRAM and HRAM
  variables (`u8`/`i8`/`u16`/`i16`; the HRAM ones read and written with `ldh`; each variable section at a fixed address)
  and WRAM arrays of them (see [WRAM arrays](#wram-arrays-rustboy)), sprites (8×8, or 8×16 with `set_sprite_size`) in a shadow OAM copied to OAM by an OAM DMA every VBlank, metasprites of any layout
  (see [Arbitrary metasprites](#arbitrary-metasprites); 16×16 ones with `add_sprite_16x16`), runtime sprite and
  entity pools (see [Sprite visibility and reusable OAM slots](#sprite-visibility-and-reusable-oam-slots) and
  [Entity pools](#entity-pools)), animations (looping, ping-pong or played once, each with its own period if you want, and
  completion events or callbacks; see [Per-animation timing](#per-animation-timing) and
  [Animation completion and replay](#animation-completion-and-replay)), joypad bindings, a main loop woken once a frame by
  the VBlank interrupt, handlers for the VBlank, STAT, timer, serial and joypad interrupts, the `rSTAT` conditions and
  the timer, a soft reset (see [Frames and interrupts](#frames-and-interrupts-rustboy)), and routines (the builtins and
  your own, see [Routines](#routines)) that are included only when used, with what they need. Memory is checked: too
  many tiles or sprites (40) panic with a clear message where they are added, as do unknown sprite ids and animation
  names; variables that do not fit in WRAM0 or HRAM make `build()` return `Error::MemoryFull` (a problem of the whole program,
  like a name defined twice: `build()` returns an `Error` for those, and a wrong call panics where it is made). The output is deterministic: things appear in the order you created them.
- **Known limits:** there is no sound yet, and the engine puts everything in one ROM bank (`asm` programs can open `ROMX` sections, but
  nothing switches banks yet). The full list, with fixes planned, is in
  [CONTEXT.md](CONTEXT.md).

## Project structure

```text
src/
├── asm/           # Instr, typed operands and Expr, typed sections, the Asm and Block builders, unique labels, jr → jp relaxation, RGBDS output
├── stdlib/        # routines as values (Routine: dependencies, calling convention), graphics, inputs, variables, flow control (If, …)
├── engine/        # RustBoy: sprites, tiles, variables, functions, animations, inputs, the program layout (chunks)
├── hw.rs          # hardware facts as data: registers, flags, memory map, OAM layout, screen sizes (hardware.inc names and values)
├── prelude.rs     # the common types of every layer, for `use rust_boy::prelude::*`
├── bin/           # the example programs
└── lib.rs
tests/                        # integration tests: snapshots.rs (the examples' generated asm), emulator.rs
                              # (ROMs run headless; the test emulator is support/gameboy.rs)
include/hardware.inc          # hardware definitions for RGBDS (v4.x)
examples/<bin>/               # per example: main.asm, its generated asm (the snapshot), and its assets
                              # (.2bpp, .png, .aseprite); examples/unbricked/originals/ is the tutorial's own source
scripts/run.sh                # build one binary into a ROM (.gb, .sym, .map) and run it in an emulator
scripts/assemble-examples.sh  # build every example into a ROM
scripts/fetch-test-roms.sh    # Blargg's test ROMs, which check the test emulator
Task.md, CONTEXT.md, AGENTS.md, CLAUDE.md
```

## Development

Run the same checks as CI before pushing:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                      # with the snapshot tests; UPDATE_SNAPSHOTS=1 rewrites the snapshots
scripts/assemble-examples.sh
scripts/fetch-test-roms.sh /tmp/gb-test-roms
RGBDS_LINK_CHECK=1 GB_TEST_ROMS=/tmp/gb-test-roms cargo test --lib --test emulator
```

Work happens on branches merged through pull requests into `refactor`; see [AGENTS.md](AGENTS.md) and
[CLAUDE.md](CLAUDE.md).

## Inspirational Projects

This project was inspired by and builds upon the excellent work of the Game Boy development community:

### Tools & Toolchains
- **[RGBDS](https://github.com/gbdev/rgbds)** - The Rednex Game Boy Developers Suite, the assembler toolchain that processes the generated assembly
- **[GBDK-2020](https://github.com/gbdk-2020/gbdk-2020)** - Game Boy Development Kit, a C compiler for Game Boy that inspired high-level development approaches
- **[rustboy](https://github.com/VelocityRa/rustboy)** - A Game Boy emulator written in Rust, demonstrating Rust's capability in retro gaming
- **[cranelift-z80](https://github.com/zlfn/cranelift-z80)** - Z80 backend for Cranelift, exploring code generation for Z80-based systems
- **[rust-gb](https://github.com/zlfn/rust-gb)** - Another Rust-based Game Boy project exploring similar concepts
- **[gbdev.io](https://gbdev.io/)** - Central hub for Game Boy development resources and documentation
- **[retroshield-z80-workbench](https://github.com/ajokela/retroshield-z80-workbench)** - Z80 development workbench, showing alternative approaches to retro development

Special thanks to all the developers who have contributed to Game Boy homebrew tooling and documentation over the years.

## License

MIT, see [LICENSE](LICENSE).

### Audio startup

```rust
use rust_boy::prelude::*;
let mut gb = RustBoy::with_config(RustBoyConfig::default().audio(AudioConfig::On(Mixer::default())));
// Stereo volume 0..=7; route channel 1 to the left output only.
gb.init(rust_boy::stdlib::audio::set_mixer(Mixer::new(4, 4, hw::AUDTERM_1_LEFT.value)));
let asm = gb.build()?;
# Ok::<(), rust_boy::engine::Error>(())
```

`AudioConfig::Off` powers off at startup. `On` resets existing channels, powers on
with the selected mixer, and leaves channels silent until triggered. `Inherit` is
the default and leaves boot audio state unchanged. Configuration runs before user
init on cold startup and soft reset. Runtime `power_off`, `power_on` and `set_mixer`
snippets preserve IME and change AF only. Wave RAM is retained. Volume zero is quiet,
not mute; use zero routing or power off to mute. `hw::NR10` through `NR52`,
`AUD3WAVERAM`, and `AUD*` flags name the hardware.inc symbols.

### One-shot sound effects

```rust
use rust_boy::prelude::*;
let mut gb = RustBoy::with_config(RustBoyConfig::default().audio(AudioConfig::On(Mixer::default())));
let beep = Sfx::Pulse2(PulseEffect::new(
    1750, 32, Duty::Half, Envelope::new(12, EnvelopeDirection::Decrease, 2),
));
gb.init(gb.play_sfx(&beep));
let asm = gb.build()?;
# Ok::<(), rust_boy::engine::Error>(())
```

`Sfx::Pulse1` adds a `Sweep`; `Pulse2` has none. `Sfx::Wave` takes a `WaveEffect`
with a `WavePattern` (32 samples 0..=15, or 16 packed bytes) and `WaveLevel`.
`Sfx::Noise` takes a `NoiseEffect` with a seven- or fifteen-bit LFSR, shift 0..=13
and divider 0..=7 (zero means 0.5). Pulse/wave period is the hardware 11-bit value,
not Hz: pulse frequency is `131072 / (2048 - period)`, wave frequency is half that.
Constructors panic for out-of-range fields. Sweep addition overflow can silence CH1;
zero-volume decreasing envelopes disable the DAC. Wave hardware first plays sample 1.

Every effect replaces its channel and enables a hardware length timer: pulse/noise
length 1..=64, wave length 1..=256 ticks at 256 Hz, with tick phase affecting duration.
Envelopes use 64 Hz ticks (pace zero disables them); sweep uses 128 Hz ticks.
`play_sfx` changes AF only and returns carry clear when programmed, carry set with no
writes if the APU is off. It preserves power, mixer, other channels and IME.
`stdlib::audio::stop_channel(Channel::Pulse2)` disables just that channel's DAC.
The standalone `stdlib::audio::play_sfx(labels, &effect)` uses the supplied label allocator.
Serialize audio writes with music/interrupt code; reserve a channel before replacing it.
CH3 stops before uploading wave RAM. DAC toggles may pop, and this API does not
reset pulse phase or provide a software effect sequence. See
[Pan Docs registers](https://gbdev.io/pandocs/Audio_Registers.html) and
[APU details](https://gbdev.io/pandocs/Audio_details.html) for hardware behavior.

### Tracker music (hUGEDriver 6.1.1)

```rust
use rust_boy::prelude::*;
let song = Song::from_rgbds(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/music/song.asm")), "DemoSong");
let mut gb = RustBoy::new();
gb.enable_music(song, MusicClock::MainFrame);
let asm = gb.build()?;
# Ok::<(), rust_boy::engine::Error>(())
```

Export from hUGETracker 1.0.1 as RGBDS assembly and use its descriptor name.
`Song::from_rgbds_file(path, descriptor)` reads the file at generation time;
`from_rgbds` accepts its text. Data must be self-contained except the standard
`hUGE.inc` include, which is expanded. Internal globals and section names are
prefixed per descriptor, so `add_music_song` accepts additional exports with the
same pattern/instrument names. Descriptors stay public and must be unique. ROM0
and ROMX placement is retained: keep a ROMX song's bank mapped for its whole tick.
Other export formats/driver versions and automatic bank switching are not supported.

`enable_music` runs once, starts the first song before user init, and changes an
inherited audio policy to `AudioConfig::On(Mixer::default())`. Explicit audio
policies stay in effect. Soft reset initializes the driver/song again. The pinned
public-domain driver lives in `vendor/hugedriver`; generated output embeds its
code/data and needs only the normal hardware.inc include. It reserves 100 WRAM0
bytes at the top of available RAM, plus two control bytes, without moving user
variables. Read the pinned version/license/transform details in its README.

`MusicClock::MainFrame` ticks after `WaitFrame` and before game logic. Long game
frames slow playback. `MusicClock::VBlank` ticks after DMA/VRAM queue flush, saving
all registers; complex songs can extend the handler beyond VBlank, so account for
that before adding timing-sensitive user VBlank code. An off APU skips ticks and
keeps song position. The engine is the single tick owner; do not tick the driver
again. Song callbacks must return promptly and must not reenter music controls.

Emit `start_music("OtherDescriptor")` to restart/switch and take all four channels;
old channels are silenced and mute reservations reset. `pause_music` freezes song
position and cuts music-owned channels. `resume_music` continues a pause (cut notes
return at the next note row), returning carry set if stopped. `stop_music` requires
`start_music` before a later resume. Controls preserve IME; start/reserve/release
may change all registers, pause/stop/resume change AF. User interrupt handlers
cannot call them, directly or through a registered function; `build` reports
`MainLoopOnly`. Initialization publishes its paused flag before touching driver
state, so the generated VBlank tick skips incomplete initialization.

With music enabled, `play_sfx` returns carry set without writes unless the channel
is reserved. Emit `reserve_music_channel(Channel::Wave)` before its effect and
`release_music_channel` afterward. Reserved channels survive pause/stop; release
returns ownership at the next music note and invalidates CH3's wave cache so the
song wave is reloaded. Serialize game audio writers and avoid driver-wide mixer
changes in song effects when sharing channels. Playback is tested as linked SM83
execution, song state and register/wave RAM writes; the test emulator does not
synthesize audio. Build/run the demo with `scripts/run.sh --run music`.
