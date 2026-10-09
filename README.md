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
| `rust_boy::rust_boy` | engine | `RustBoy`: sprites (OAM), tiles (VRAM), variables (WRAM), joypad bindings, animations and functions. `build()` writes the whole program. |
| `rust_boy::gb_std` | routines | Ready-made routines (Memcopy, WaitVBlank, UpdateKeys, GetTileByPixel, …) and control flow (`If`, `IfConst`, `IfA`, `IfCall`). |
| `rust_boy::gb_asm` | assembly | `Asm`: one method per instruction or directive, with typed operands (`R8`, `R16`, `Mem`) and expressions (`Expr`), printed in RGBDS syntax. |

Each level is built on the one below it, and you can mix them.

## Requirements

- Rust 1.85 or newer (edition 2024). No other Rust dependencies.
- [RGBDS](https://rgbds.gbdev.io/) 0.9 or newer to build ROMs (CI uses 1.0.4).

To use it in your own project:

```toml
[dependencies]
rust-boy = { git = "https://github.com/ffex/rust-boy" }
```

## Quick start (`RustBoy`)

A sprite that moves with the D-pad:

```rust
use rust_boy::gb_std::inputs::PadButton;
use rust_boy::rust_boy::{InputManager, RustBoy, SpriteSize, TileSource};

fn main() {
    let mut gb = RustBoy::new();

    // Sprites are 8x8 by default; choose 8x16 before adding any sprite
    gb.set_sprite_size(SpriteSize::Size8x16);

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

    println!("{}", gb.build());
}
```

## Low level (`gb_asm`)

The same building blocks the engine uses, one instruction at a time:

```rust
use rust_boy::gb_asm::{Asm, Chunk, Condition};

fn main() {
    let mut asm = Asm::new();

    // Cartridge header at $100-$14F: rgbfix fills in the logo and checksums
    asm.include_hardware()
        .section("Header", "ROM0[$100]")
        .raw("nop")
        .raw("jp EntryPoint")
        .ds("$150 - @", "0");

    asm.section("Main", "ROM0")
        .label("EntryPoint")
        .label("MainLoop")
        .call("WaitVBlank")
        .jp("MainLoop");

    asm.chunk(Chunk::Functions)
        .label("WaitVBlank")
        .ld_a_addr_def("rLY")
        .cp_imm(144)
        .jr_cond(Condition::NZ, "WaitVBlank")
        .ret();

    println!("{}", asm.to_asm());
}
```

Operands are typed: registers are `R8` (`a` … `l`, and `[hl]`) and `R16`, memory is `Mem` (`[bc]`, `[de]`,
`[hli]`, `[hld]`, `[c]`, `[address]`), and values are `Expr`s: numbers, symbols (`hardware.inc` names, labels,
variables, `DEF` constants) and arithmetic on them. A Rust integer or a symbol name can be passed directly:

```rust
use rust_boy::gb_asm::{Asm, Expr, Mem, R8, R16};

let mut asm = Asm::new();
asm.ld(R16::HL, Expr::sym("_OAMRAM") + 4) // ld hl, _OAMRAM+4
    .ld(R8::A, Mem::addr("wScore")) // ld a, [wScore]
    .add(R8::B) // add a, b
    .cp("BRICK_LEFT") // cp a, BRICK_LEFT
    .ld(Mem::Hli, R8::A) // ld [hli], a
    .ld(R8::A, Expr::sym("LCDCF_ON") | "LCDCF_BGON") // ld a, LCDCF_ON | LCDCF_BGON
    .sub(-1); // sub a, -1
let text: Vec<String> = asm.get_main_instrs().iter().map(|i| i.to_string()).collect();
assert_eq!(text[0], "ld hl, _OAMRAM+4");
assert_eq!(text[5], "ld a, LCDCF_ON | LCDCF_BGON");
```

A value is never a destination, so `asm.ld(1, 2)` does not compile. Operands that make no SM83 instruction
(`ld [hl], [hl]`, `ld b, [de]`), a value that does not fit (`ld a, 300`) and a register written as text
(`asm.cp("b")`) panic with a clear message. `Expr::raw("…")` passes any other RGBDS expression through as it is.

## Building a ROM

```bash
cargo run --bin fosdem > main.asm
rgbasm -I include -I examples/fosdem -o main.o main.asm   # include/: hardware.inc, examples/fosdem: .2bpp assets
rgblink -o main.gb main.o
rgbfix -v -p 0xFF main.gb
```

Or build every example at once into `target/examples/<bin>/main.gb` (under `$CARGO_TARGET_DIR` if set):

```bash
scripts/assemble-examples.sh
```

CI does the same on every pull request and keeps the ROMs as a downloadable artifact (`example-roms`).
Open them in any Game Boy emulator.

## Examples

| Binary | Level | What it shows |
|---|---|---|
| `basic_usage` | `gb_asm` | A minimal program: header, main loop, VBlank wait |
| `unbricked` | `gb_asm` | The [gbdev.io](https://gbdev.io/gb-asm-tutorial/) "Unbricked" tutorial, written instruction by instruction |
| `unbricked_std` | `gb_std` | The same game with `gb_std` routines and `If` |
| `unbricked_rustboy` | `rust_boy` | The same game with `RustBoy` |
| `fosdem` | `rust_boy` | A 16×16 walking character with four animations (FOSDEM demo) |
| `coin-anim` | `rust_boy` | An animated coin: A starts the animation, B stops it |

## What is supported

- **Instructions** (`gb_asm`): the whole SM83 instruction set, printed in RGBDS syntax: loads (`ld`, `ldh`,
  `ld [hli]`/`[hld]`, `ld hl, sp + e`, `push`/`pop`), the 8-bit ALU on `a` (`add`, `adc`, `sub`, `sbc`, `and`, `xor`,
  `or`, `cp`, one source each: `asm.cp(144)` prints `cp a, 144`), `inc`/`dec`, `add hl, r16`,
  `add sp, e`, the rotates and shifts (`rlca`… and `rlc`, `rrc`, `rl`, `rr`, `sla`, `sra`, `swap`, `srl` on a register
  or `[hl]`, `R8`), `bit`/`set`/`res`, `daa`, `cpl`, `scf`/`ccf`, `nop`, `halt`, `stop`, `di`/`ei`, `jp`, `jr`,
  `call` and `ret` (all four also with the `z`/`nz`/`c`/`nc` conditions), `jp hl`, `reti`, `rst`, plus the directives
  `SECTION`, `INCLUDE`, `INCBIN`, `DEF … EQU`, `db`, `dw`, `ds`, labels, comments and raw lines. Operands are
  typed (see above): an operand the instruction does not take either does not compile (`ld 1, 2`, `inc 5`,
  `and a, hl`) or panics with a clear message (`ld [hl], [hl]`, `bit 8`, `rst $09`).
- **Engine** (`RustBoy`): VRAM layout for sprite and background tiles and tilemaps (`$9800`, `$9C00`), WRAM variables
  (`u8`/`i8`/`u16`/`i16`), OAM sprites (8×8, or 8×16 with `set_sprite_size`), 16×16 composite sprites
  (in 8×16 mode), animations (looping, ping-pong or played once), joypad bindings, and builtin routines that are included only when
  used. Memory is checked: too many tiles, sprites (40) or variables panic with a clear message, as do unknown
  sprite ids and animation names. The output is deterministic: things appear in the order you created them.
- **Known limits:** the only composite sprite is 16×16 (two 8×16 sprites), all animations share one speed,
  there is no sound yet, and everything lives in one ROM bank. The full list, with fixes planned, is in
  [CONTEXT.md](CONTEXT.md).

## Project structure

```text
src/
├── gb_asm/        # Instr, typed operands and Expr, the Asm builder, unique labels, RGBDS output
├── gb_std/        # routines (graphics, inputs, variables) and flow control (If, …)
├── rust_boy/      # RustBoy: sprites, tiles, variables, functions, animations, inputs
├── hw.rs          # hardware facts as data (VRAM, WRAM and OAM layout, hardware.inc names)
├── bin/           # the example programs
└── lib.rs
include/hardware.inc          # hardware definitions for RGBDS (v4.x)
examples/                     # example assets (.2bpp, .png, .aseprite) and reference .asm files
scripts/assemble-examples.sh  # build every example into a ROM
Task.md, CONTEXT.md, CLAUDE.md
```

## Development

Run the same checks as CI before pushing:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
scripts/assemble-examples.sh
```

Work happens on branches merged through pull requests into `refactor`; see [CLAUDE.md](CLAUDE.md).

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
