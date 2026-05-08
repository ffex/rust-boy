# Animations

rust-boy supports frame-based sprite animations through `AnimationType` and the sprite manager.

## Adding an Animation

Animations are registered on a sprite after the sprite is created. You specify a range of frames (tile indices relative to the sprite's base tile) and a playback mode.

```rust
use rust_boy::rust_boy::{RustBoy, TileSource, AnimationType};

let mut gb = RustBoy::new();

// Load 7 frames from a .2bpp file
let coin = gb.add_sprite("Coin", TileSource::from_file("coin.2bpp", 7), 80, 72, 0);

// Animate frames 0 through 6 in a loop
gb.sprites.add_animation(coin, "CoinSpin", 0, 6, AnimationType::Loop);
```

For 16x16 sprites (two stacked 8x8 tiles per frame), use `add_animation_with_step` with `frame_step: 2`:

```rust
gb.sprites.add_animation_with_step(player, "Walk", 0, 7, AnimationType::Loop, 2);
```

---

## Animation Types

| Type | Behavior |
|------|----------|
| `AnimationType::Loop` | Restarts at frame 0 after the last frame |
| `AnimationType::PingPong` | Plays forward then backward, repeating |
| `AnimationType::Once` | Plays once and stops on the last frame |

---

## Controlling Animations at Runtime

Animations can be enabled and disabled from button actions or game logic.

```rust
// Start disabled
gb.sprites.disable_animation("CoinSpin");

// Enable on button press, disable on another
let mut inputs = InputManager::new();
inputs.on_press(PadButton::A, gb.sprites.enable_animation("CoinSpin"));
inputs.on_press(PadButton::B, gb.sprites.disable_animation("CoinSpin"));
gb.add_inputs(inputs);
```

Both `enable_animation` and `disable_animation` return `Vec<Instr>` and can be used anywhere an instruction sequence is accepted.

---

## Animation Speed

Set a global frame delay (in game frames) before the first sprite is added:

```rust
gb.set_animation_delay(4); // advance one animation frame every 4 game frames
```

The default is 1 (advances every game frame).

---

## Multiple Animations on One Sprite

A sprite can have multiple named animations; only one plays at a time.

```rust
gb.sprites.add_animation(player, "Idle",    0, 1, AnimationType::Loop);
gb.sprites.add_animation(player, "Walk",    2, 5, AnimationType::Loop);
gb.sprites.add_animation(player, "Jump",    6, 7, AnimationType::Once);

// Switch on input
inputs.on_press(PadButton::Right, {
    let mut v = gb.sprites.enable_animation("Walk");
    v.extend(gb.sprites.disable_animation("Idle"));
    v
});
```

---

## Notes

- Frame indices are relative to the sprite's base tile in VRAM, not absolute tile numbers.
- Animation state is stored in WRAM and updated once per game frame inside the main loop.
- `add_animation` returns a `u8` animation index that can be used with lower-level sprite methods if needed.
