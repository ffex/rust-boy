# API Levels

rust-boy exposes three abstraction layers. You can mix them freely or stay entirely at one level.

```
rust_boy      ← recommended starting point
  gb_std      ← utilities and DSL helpers
    gb_asm    ← raw instruction generation
```

---

## `gb_asm` — Raw Assembly Generation

Direct mapping from Rust method calls to Game Boy Z80 instructions. Everything compiles down to this layer eventually.

**When to use:** when you need precise control over the generated assembly, are porting existing Game Boy code, or want to understand exactly what the library emits.

```rust
use rust_boy::gb_asm::{Asm, Chunk, Condition};

let mut asm = Asm::new();

asm.section("Main", "ROM0")
    .label("EntryPoint")
    .ld_a(0x91)
    .ldh_label("[$FF40]", "a")   // enable LCD
    .label("Loop")
    .call("WaitVBlank")
    .jp("Loop");

asm.chunk(Chunk::Functions)
    .label("WaitVBlank")
    .ld_a_label("[$FF44]")
    .cp_imm(144)
    .jr_cond(Condition::NZ, "WaitVBlank")
    .ret();

println!("{}", asm.to_asm());
```

Key types: `Asm`, `Chunk`, `Condition`, `Instr`, `Register`, `Operand`.

---

## `gb_std` — Standard Library

Pre-built instruction sequences for common tasks. Every function returns `Vec<Instr>` that you emit into an `Asm` instance.

**When to use:** building a game at the assembly level but want to avoid writing boilerplate for input reading, V-blank synchronization, memory copy, or conditional logic.

```rust
use rust_boy::gb_std::{
    inputs::{update_keys, check_key, PadButton},
    graphics::utility::{turn_on_screen, wait_vblank, cp_in_memory},
    flow::If,
};

asm.emit_all(turn_off_screen());
asm.emit_all(cp_in_memory("Tiles", 0x9000));
asm.emit_all(turn_on_screen());

// In main loop
asm.emit_all(update_keys());
asm.emit_all(check_key(PadButton::Left, move_left_instrs));
asm.emit_all(wait_vblank());
```

Key modules: `gb_std::inputs`, `gb_std::graphics`, `gb_std::flow`, `gb_std::variables`.

---

## `rust_boy` — High-Level API

Full game framework. Manages VRAM layout, WRAM allocation, OAM indexing, V-blank sync, and code organization automatically. You describe your game in terms of sprites, variables, and logic — the library generates all the assembly.

**When to use:** starting a new game or demo. This is the recommended default.

```rust
use rust_boy::rust_boy::{RustBoy, InputManager, TileSource, AnimationType};
use rust_boy::gb_std::{inputs::PadButton, flow::{IfCall, boxed}};

let mut gb = RustBoy::new();

gb.define_const("MAX_SPEED", "0x04");

let speed = gb.vars.create_i8("wSpeed", 1);
let player = gb.add_sprite("Player", TileSource::from_file("player.2bpp", 2), 80, 72, 0);

gb.sprites.add_animation(player, "Walk", 0, 3, AnimationType::Loop);

let mut inputs = InputManager::new();
inputs.on_press(PadButton::Left,  gb.sprites.move_left_limit(player, 1, 15));
inputs.on_press(PadButton::Right, gb.sprites.move_right_limit(player, 1, 145));
gb.add_inputs(inputs);

println!("{}", gb.build());
```

Key types: `RustBoy`, `InputManager`, `TileSource`, `AnimationType`, `SpriteId`, `Var`.

---

## Escape Hatch

Any level can drop down to the one below. Inside `RustBoy` you can call `gb.raw(|asm| { … })` to emit arbitrary assembly:

```rust
gb.raw(|asm| {
    asm.label("DebugBreak");
    asm.ld_a(0xFF);
});
```

This makes it possible to prototype quickly at the high level and optimize specific hot paths at the assembly level later.

---

## Choosing a Level

| Situation | Recommended level |
|-----------|------------------|
| New game / prototype | `rust_boy` |
| Porting existing GB assembly | `gb_asm` |
| Game needing custom memory layout | `gb_std` + `gb_asm` |
| Learning how GB hardware works | `gb_asm` |
| Need full control over one function | escape hatch from `rust_boy` |
