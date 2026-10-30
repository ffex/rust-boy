# Sprite Movement

Sprites on the Game Boy are 8x8 pixel objects stored in OAM (Object Attribute Memory). rust-boy provides helpers to move them at all three API levels.

## High-Level API (`rust_boy`)

Add a sprite with `add_sprite`, then use the helpers returned by `gb.sprites`.

```rust
use rust_boy::rust_boy::{RustBoy, TileSource};

let mut gb = RustBoy::new();

let player = gb.add_sprite(
    "Player",              // label used in generated assembly
    TileSource::from_file("player.2bpp", 1),
    80,                    // initial X position
    72,                    // initial Y position
    0,                     // OAM flags (0 = no flip, palette 0)
);
```

### Basic Movement

Each method returns a `Vec<Instr>` that can be passed to `add_to_main_loop` or `on_press`.

```rust
gb.sprites.move_left(player, 2)    // move 2 pixels left
gb.sprites.move_right(player, 2)   // move 2 pixels right
gb.sprites.move_up(player, 2)      // move 2 pixels up
gb.sprites.move_down(player, 2)    // move 2 pixels down
```

### Bounded Movement (Wall Limits)

Prevent a sprite from leaving the screen using limit variants.

```rust
// Arguments: (sprite_id, pixels_per_frame, boundary_pixel)
gb.sprites.move_left_limit(player, 1, 15)    // stop at x=15
gb.sprites.move_right_limit(player, 1, 105)  // stop at x=105
gb.sprites.move_up_limit(player, 1, 16)      // stop at y=16
gb.sprites.move_down_limit(player, 1, 128)   // stop at y=128
```

### Movement by Variable

Move a sprite by an amount stored in a WRAM variable (useful for physics/momentum).

```rust
let momentum_x = gb.vars.create_i8("wMomentumX", 1);
let momentum_y = gb.vars.create_i8("wMomentumY", -1i8);

gb.add_to_main_loop(gb.sprites.move_x_var(player, "wMomentumX"));
gb.add_to_main_loop(gb.sprites.move_y_var(player, "wMomentumY"));
```

### Reading Sprite Position

Use `get_x` and `get_y` to load the sprite's current position into the A register. These are commonly used inside `If` comparisons.

```rust
use rust_boy::gb_std::flow::{If, InstrOps};

// Bounce when ball hits the paddle height
let bounce = If::eq(
    gb.sprites.get_y(paddle),
    gb.sprites.get_y(ball).plus(5),  // ball Y + 5
    momentum_y.set(-1),
);
gb.add_to_main_loop(bounce);
```

### Pivot Points

Get the position of an offset from a sprite's origin — useful for collision detection.

```rust
// Returns Vec<Instr> that loads the tilemap address at (sprite_x + dx, sprite_y + dy)
gb.sprites.get_pivot(ball, 0, 1)   // 1 pixel below center
gb.sprites.get_pivot(ball, -1, 0)  // 1 pixel to the left
```

---

## Mid-Level API (`gb_std`)

When working directly with `Asm`, use `SpriteManager` from `gb_std`.

```rust
use rust_boy::gb_std::graphics::sprites::SpriteManager;

let mut manager = SpriteManager::new();
let id = manager.add_sprite(80, 72, 0, 0); // x, y, tile_index, flags

let sprite = manager.get_sprite_mut(id);

// Generate movement instructions
let move_left_instrs  = sprite.move_left(1);
let move_right_instrs = sprite.move_right(1);
let move_up_instrs    = sprite.move_up(1);
let move_down_instrs  = sprite.move_down(1);

// With boundary
let bounded = sprite.move_left_limit(1, 15);
```

Emit the result with `asm.emit_all(instrs)`.

---

## OAM Coordinate System

The Game Boy OAM uses an offset coordinate system:

| What you write | Screen position |
|----------------|-----------------|
| Y = 16         | top of screen   |
| Y = 160        | bottom of screen |
| X = 8          | left of screen  |
| X = 168        | right of screen |

rust-boy's movement helpers work in screen pixels — the library handles the offsets internally.

---

## Notes

- OAM is updated during V-blank. `RustBoy` handles synchronization automatically.
- For 16x16 sprites (two stacked 8x8 tiles), use `CompositeSpriteId` and the composite-sprite helpers on `SpriteManager`.
- Sprite flags byte controls horizontal/vertical flip and which palette to use.
