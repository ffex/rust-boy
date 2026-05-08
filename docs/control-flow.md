# Control Flow

rust-boy provides high-level `If` constructs that hide the complexity of Game Boy conditional jumps and label management.

All types live in `gb_std::flow`.

## `If` — Compare Two Values

Use when both sides are computed at runtime.

```rust
use rust_boy::gb_std::flow::If;

// If left == right { then_body }
let check = If::eq(left_instrs, right_instrs, then_body);

// With else branch
let check = If::eq(left_instrs, right_instrs, then_body)
    .or_else(else_body);
```

Available comparison operators:

| Method | Meaning |
|--------|---------|
| `If::eq(l, r, body)` | `l == r` |
| `If::ne(l, r, body)` | `l != r` |
| `If::lt(l, r, body)` | `l < r` |
| `If::ge(l, r, body)` | `l >= r` |
| `If::le(l, r, body)` | `l <= r` |
| `If::gt(l, r, body)` | `l > r` |

All three arguments accept anything that implements `Emittable`: `Vec<Instr>`, another `If`, a `Call`, or a boxed mix.

### Nesting

```rust
use rust_boy::gb_std::flow::{If, InstrOps};

// If ball_y == paddle_y+5 AND ball_x >= paddle_x+16: bounce
let bounce = If::eq(
    gb.sprites.get_y(paddle),
    gb.sprites.get_y(ball).plus(5),
    If::ge(
        gb.sprites.get_x(ball),
        gb.sprites.get_x(paddle).plus(16),
        momentum_y.set(-1),
    ),
);
gb.add_to_main_loop(bounce);
```

---

## `IfConst` — Compare a Value Against a Constant

More efficient than `If` when the right-hand side is a compile-time constant or label.

```rust
use rust_boy::gb_std::flow::IfConst;

// If get_tile() == BRICK_LEFT
let check = IfConst::eq(
    Call::with_args("GetTileByPixel", pivot_instrs),
    "BRICK_LEFT",
    erase_brick_instrs,
);
```

The first argument is any `Emittable` whose result lands in the `A` register. The second is a constant name defined with `define_const`.

---

## `IfA` — Branch on the Current Value of A

Use when A is already loaded and you want to chain comparisons without reloading.

```rust
use rust_boy::gb_std::flow::IfA;

// A is already set by the previous IfConst; now check another tile value
IfConst::eq(tile_load, "BRICK_LEFT", erase_left)
// then immediately check A again:
// A still holds the result of tile_load here
let second_check = IfA::eq("BRICK_RIGHT", erase_right);
```

`IfA` skips the value-loading step and immediately emits a `CP` against the constant.

---

## `IfCall` — Branch on a Function's Return

Use when the condition is the result of calling a function.

```rust
use rust_boy::gb_std::flow::IfCall;

// If IsWallTile() is true
gb.add_to_main_loop(IfCall::is_true("IsWallTile", bounce_instrs));

// With setup code run before the call
IfCall::is_true("IsWallTile", bounce_instrs)
    .with_setup(load_pivot_instrs)

// Other comparators
IfCall::is_false("IsWallTile", body)
IfCall::is_less("CompareScore", body)
IfCall::is_greater_eq("CompareScore", body)
```

---

## Arithmetic on Instruction Sequences (`InstrOps`)

The `InstrOps` extension trait adds arithmetic to any `Vec<Instr>`, letting you write expressions inline.

```rust
use rust_boy::gb_std::flow::InstrOps;

// Load sprite Y, then add 5 to the result in A
gb.sprites.get_y(ball).plus(5)

// Load sprite X, then subtract 8
gb.sprites.get_x(paddle).minus(8)
```

These are typically passed as arguments to `If`:

```rust
If::lt(
    gb.sprites.get_x(ball),
    gb.sprites.get_x(paddle).minus(8),
    body,
)
```

---

## Mixing Types with `boxed`

When a `Vec<dyn Emittable>` needs to hold different `Emittable` types, use `boxed()`:

```rust
use rust_boy::gb_std::flow::boxed;

gb.define_function_from(
    "HandleBrick",
    vec![
        boxed(IfConst::eq(load_tile, "BRICK_LEFT", erase_left)),
        boxed(IfA::eq("BRICK_RIGHT", erase_right)),
    ],
);
```

---

## Notes

- Labels for jump targets are generated automatically with a monotonic counter; you never write them manually.
- The Game Boy Z80 only has `Z` and `C` flags. `LE` and `GT` compile to a double-check (`CP` + two branches) which is slightly larger.
- `If` bodies accept `Vec<Instr>` or nested `If`s/`Call`s — there is no depth limit.
