# Variables

rust-boy manages WRAM (Work RAM) variables automatically. You declare a variable once and get back a handle (`Var`) that generates load/store instructions wherever you use it.

## Creating Variables

All variable creation is done through `gb.vars`:

```rust
use rust_boy::rust_boy::RustBoy;

let mut gb = RustBoy::new();

let score      = gb.vars.create_u8("wScore", 0);       // unsigned 8-bit, initial 0
let health     = gb.vars.create_u8("wHealth", 3);      // unsigned 8-bit, initial 3
let vel_x      = gb.vars.create_i8("wVelX", 1);        // signed 8-bit, initial 1
let vel_y      = gb.vars.create_i8("wVelY", -1i8);     // signed 8-bit, initial -1
let big_count  = gb.vars.create_u16("wBigCount", 0);   // unsigned 16-bit
```

Available types:

| Method | Rust analog | Game Boy size |
|--------|-------------|---------------|
| `create_u8(name, init)` | `u8` | 1 byte |
| `create_i8(name, init)` | `i8` | 1 byte (signed) |
| `create_u16(name, init)` | `u16` | 2 bytes |
| `create_i16(name, init)` | `i16` | 2 bytes (signed) |

Names must be unique and follow RGBDS label conventions (alphanumeric + underscore, typically prefixed with `w`).

---

## Reading and Writing

The `Var` handle exposes two methods that return `Vec<Instr>`:

```rust
score.set(5)   // store the literal value 5 into the variable
score.get()    // load the variable's value into the A register
```

Pass these to `add_to_main_loop`, `on_press`, or any other instruction consumer.

### Example: Increment a counter

```rust
use rust_boy::gb_std::flow::InstrOps;

let frame = gb.vars.create_u8("wFrame", 0);

// Load current value, add 1, store back
let mut tick = frame.get();
tick.extend(tick.clone().plus(1)); // A = A + 1 (using InstrOps)
tick.extend(frame.set(/* from A */ ));
gb.add_to_main_loop(tick);
```

### Example: Reverse momentum on bounce

```rust
let vel_x = gb.vars.create_i8("wVelX", 1);

// When ball hits a wall: flip velocity to -1
gb.add_to_main_loop(IfCall::is_true("IsWallTile", vel_x.set(-1)));
```

---

## Constants

For values that never change at runtime, use `define_const`:

```rust
gb.define_const("BRICK_LEFT",  "0x05");
gb.define_const("BLANK_TILE",  "0x08");
gb.define_const_hex("SCORE_DISPLAY", 0x9870); // hex address, no quotes
```

Constants are emitted as RGBDS `DEF` statements and can be referenced by name in `IfConst`, `IfA`, and label-based instructions.

---

## Memory Layout

Variables are automatically allocated sequentially starting at `$C000` (WRAM0). The `VariableManager` emits:

1. A WRAM section definition for the variable slots.
2. Initialization code that runs once at startup (writing the initial values you passed to `create_*`).

You never need to compute addresses or offsets manually.

---

## Notes

- The `Var` handle is `Copy`-like in usage — you can call `.set()` and `.get()` multiple times to produce independent instruction vectors at different points in the program.
- Variable names appear verbatim in the generated RGBDS assembly, which makes debugging in an emulator straightforward.
- If you need a variable outside WRAM0 (e.g., HRAM for interrupt code), use the mid-level `VariableManager::create_in_section` from `gb_std`.
