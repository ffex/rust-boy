# Graphics

rust-boy handles the three layers of Game Boy graphics: tiles (raw pixel data), the tilemap (which tile goes where on screen), and sprites (moving objects via OAM).

---

## Tiles

### Loading Tiles — High-Level

`TileSource` describes where tile data comes from. Pass it to `add_sprite` or `gb.tiles.add_*`.

```rust
use rust_boy::rust_boy::TileSource;

// From a raw hex array defined in Rust (see examples for format)
TileSource::from_raw(MY_TILES_CONST)

// From a compiled .2bpp file (tile_count = number of 8x8 tiles in the file)
TileSource::from_file("player.2bpp", 4)
```

### VRAM Allocation

`TileManager` allocates VRAM automatically in two regions:

| Region | Address range | Use |
|--------|--------------|-----|
| Sprite tiles | `$8000–$8FFF` | OAM sprites |
| Background tiles | `$9000–$97FF` | Background/window tiles |

```rust
let mut gb = RustBoy::new();

// Background tile set (goes to $9000)
gb.tiles.add_background("Tiles", TileSource::from_raw(BG_TILES));

// Tilemap data ($9800)
gb.tiles.add_tilemap("Tilemap", TILEMAP_DATA);

// Sprite tiles are allocated automatically when you call add_sprite
let player = gb.add_sprite("Player", TileSource::from_file("player.2bpp", 2), 80, 72, 0);
```

---

## Tilemap

The Game Boy background is a 32×32 grid of tile indices stored in VRAM at `$9800`. Each cell holds one byte — an index into the tile set.

### Setting Tiles at Runtime

`TileRef` lets you read and write individual tilemap cells:

```rust
use rust_boy::gb_std::graphics::tile_ref::TileRef;

// Create a reference to absolute tilemap address
let cell = TileRef::new(0x9870);

// Or from (column, row) coordinates (0–31)
let cell = TileRef::from_coords(5, 3, 0x9800);

// Or from pixel-aligned coordinates using the default tilemap
let cell = TileRef::from_xy(40, 24); // pixel (40,24) → tile cell

// Write a tile index
TileRef::set_tile(0x00)               // immediate index
TileRef::set_tile_label("BLANK_TILE") // from a defined constant

// Read the current tile into A
TileRef::get_tile()

// Navigate the grid
TileRef::next_tile()  // HL += 1  (move right)
TileRef::prev_tile()  // HL -= 1  (move left)
TileRef::next_row()   // HL += 32 (move down one row)
TileRef::prev_row()   // HL -= 32 (move up one row)
```

All methods return `Vec<Instr>` to embed in your game logic.

---

## Screen Control

```rust
use rust_boy::gb_std::graphics::utility::{
    turn_on_screen,
    turn_off_screen,
    wait_vblank,
    wait_not_vblank,
};

// Must turn off LCD before writing to VRAM
turn_off_screen()

// Copy tile data (then turn back on)
turn_on_screen()

// Sync with the display — call once per frame in the main loop
wait_vblank()
wait_not_vblank()
```

`RustBoy::build()` inserts V-blank waiting and OAM DMA automatically.

---

## Memory Copy

To copy tile data or tilemaps into VRAM at init time:

```rust
use rust_boy::gb_std::graphics::utility::cp_in_memory;

// Copy data from label to VRAM address
cp_in_memory("Tiles", 0x9000)
cp_in_memory("Tilemap", 0x9800)
```

This emits a DMA-style copy loop. The high-level `RustBoy` API handles this automatically from the tile and tilemap registrations.

---

## Tile Collision Helpers

Check what tile is at a given screen pixel position:

```rust
use rust_boy::gb_std::graphics::utility::{get_tile_by_pixel, is_specific_tile};

// Loads the tilemap cell at (pixel_x, pixel_y) → address in HL, tile in A
get_tile_by_pixel()

// Check if A matches any of a set of tile indices
is_specific_tile("IsWallTile", &["$00", "$01", "$02"])
```

`is_specific_tile` generates a named function you can then call with `IfCall::is_true`.

---

## Sprite Flags

The flags byte in `add_sprite(name, source, x, y, flags)` controls OAM attributes:

| Bit | Meaning |
|-----|---------|
| 7 | BG/Window over sprite |
| 6 | Y flip |
| 5 | X flip |
| 4 | Palette (OBP0 / OBP1) |

Common values:
- `0x00` — no flip, palette 0
- `0x20` — X flip
- `0x40` — Y flip
- `0x10` — palette 1

---

## Notes

- VRAM is only writable when the LCD is off or during V-blank. `RustBoy` schedules all VRAM writes correctly.
- The Game Boy has one 160×144 pixel screen backed by a 256×256 scrollable background map.
- Sprites are always in front of the background unless bit 7 of the flags byte is set.
