# Button Actions

rust-boy provides two ways to handle Game Boy joypad input depending on which API level you're using.

## Available Buttons

```rust
use rust_boy::gb_std::inputs::PadButton;

// D-Pad
PadButton::Up
PadButton::Down
PadButton::Left
PadButton::Right

// Face buttons
PadButton::A
PadButton::B
PadButton::Start
PadButton::Select
```

---

## High-Level API (`rust_boy`)

The recommended approach. Use `InputManager` to bind actions to buttons and pass it to `RustBoy`.

```rust
use rust_boy::rust_boy::{InputManager, RustBoy};
use rust_boy::gb_std::inputs::PadButton;

let mut gb = RustBoy::new();

// Create a sprite
let player = gb.add_sprite("Player", source, 80, 72, 0);

// Bind button actions
let mut inputs = InputManager::new();
inputs.on_press(PadButton::Left,  gb.sprites.move_left(player, 1));
inputs.on_press(PadButton::Right, gb.sprites.move_right(player, 1));
inputs.on_press(PadButton::Up,    gb.sprites.move_up(player, 1));
inputs.on_press(PadButton::Down,  gb.sprites.move_down(player, 1));

gb.add_inputs(inputs);
println!("{}", gb.build());
```

`on_press` accepts any `Vec<Instr>` — you can pass movement helpers, variable assignments, or any other instruction sequence.

### Combining Multiple Actions on One Button

```rust
use rust_boy::gb_std::flow::InstrOps;

let mut inputs = InputManager::new();

// Chain multiple instruction vectors into one
let mut jump_action = player_var.set(1); // mark as jumping
jump_action.extend(gb.sprites.move_up(player, 2));

inputs.on_press(PadButton::A, jump_action);
gb.add_inputs(inputs);
```

### Animations on Button Press

```rust
use rust_boy::rust_boy::AnimationType;

gb.sprites.add_animation(coin, "SpinAnim", 0, 6, AnimationType::Loop);
gb.sprites.disable_animation("SpinAnim"); // off by default

let mut inputs = InputManager::new();
inputs.on_press(PadButton::A, gb.sprites.enable_animation("SpinAnim"));
inputs.on_press(PadButton::B, gb.sprites.disable_animation("SpinAnim"));
gb.add_inputs(inputs);
```

---

## Mid-Level API (`gb_std`)

Use `update_keys()` and `check_key()` directly when working with raw assembly (`Asm`).

```rust
use rust_boy::gb_std::inputs::{update_keys, check_key, PadButton};
use rust_boy::gb_asm::Asm;

let mut asm = Asm::new();

// In the main loop: poll the joypad
// Must be called once per frame before any check_key calls
asm.emit_all(update_keys());

// React to a specific button
asm.emit_all(check_key(PadButton::Left, my_left_instructions));
asm.emit_all(check_key(PadButton::A,    my_a_instructions));
```

`update_keys()` writes the pressed state into two WRAM variables:
- `wCurKeys` — buttons held this frame
- `wNewKeys` — buttons newly pressed this frame

`check_key` reads `wNewKeys` (newly pressed only, not held).

---

## Notes

- Button reads happen once per frame, before the main game logic.
- `InputManager` calls `update_keys()` automatically when `add_inputs()` is used.
- The Game Boy hardware registers buttons and D-pad separately; `update_keys()` handles the two-step read internally.
