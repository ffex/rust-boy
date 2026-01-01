# rust-boy

A Game Boy assembly code generator library written in Rust. Build Game Boy programs using a high-level, idiomatic Rust API that generates clean, valid Game Boy assembly code compatible with RGBDS (Rednex Game Boy Developers Suite).

## What is rust-boy?

rust-boy is a Domain-Specific Language (DSL) and code generation framework that allows you to write Game Boy programs in Rust instead of writing assembly directly. The library provides:

- **High-level abstractions** for Game Boy assembly instructions
- **Standard library** for common game development tasks (graphics, sprites, variables, control flow)
- **Fluent API** with method chaining for clean, readable code
- **Type-safe** instruction generation with compile-time guarantees
- **Zero dependencies** - uses only Rust standard library

## Features

### Core Assembly Generation (`gb_asm`)

- Complete support for Game Boy Z80-based CPU instruction set
- All standard instructions: `ld`, `add`, `sub`, `inc`, `dec`, `and`, `or`, `xor`, `cp`, etc.
- Jump and call instructions with conditional flags (Z, NZ, C, NC)
- Bit manipulation: `srl`, `swap`, and more
- Assembler directives: `section`, `def`, `db`, `dw`, `ds`, `incbin`, `include`
- Organized code chunks: Main, Functions, Data, Tiles, Tilemap

### Game Boy Standard Library (`gb_std`)

- **Variables and Constants**: Helper functions for memory management
- **Control Flow**: High-level if/else statements with comparison operators (==, !=, <, >, <=, >=)
- **Sprite System**: OAM manipulation with movement helpers
- **Graphics Utilities**: Tile and tilemap loading, screen control, VBlank waiting
- **Memory Operations**: Fast memory copy routines

## Quick Start

Add rust-boy to your project:

```toml
[dependencies]
rust-boy = { path = "path/to/rust-boy" }
```

### Basic Example

```rust
use rust_boy::gb_asm::{Asm, Chunk, Condition};

fn main() {
    let mut asm = Asm::new();

    // Hardware setup
    asm.include_hardware()
        .section("Header", "ROM0[$100]")
        .raw("nop")
        .raw("jp EntryPoint");

    // Main code
    asm.section("Main", "ROM0")
        .label("EntryPoint")
        .comment("Initialize display")
        .ld_a(0x91)
        .ldh_label("[$FF40]", "a");

    // Main game loop
    asm.label("MainLoop")
        .ld_bc(160)
        .call("WaitVBlank")
        .jp("MainLoop");

    // VBlank function
    asm.chunk(Chunk::Functions)
        .label("WaitVBlank")
        .comment("Wait for vertical blank")
        .ld_a_label("[$FF44]")
        .cp_imm(144)
        .jr_cond(Condition::NZ, "WaitVBlank")
        .ret();

    // Generate assembly
    println!("{}", asm.to_asm());
}
```

### Running Examples

The project includes two example programs:

```bash
# Basic usage example
cargo run --bin basic_usage

# Complete breakout game (Unbricked)
cargo run --bin unbricked
```

## Project Structure

```
src/
├── gb_asm/          # Core assembly generation
│   ├── asm.rs       # Main Asm struct and API
│   ├── instr.rs     # Instruction definitions
│   └── codegen.rs   # Code generation logic
│
├── gb_std/          # Game Boy standard library
│   ├── variables.rs # Variable and constant helpers
│   ├── flow/        # Control flow abstractions
│   └── graphics/    # Sprite and graphics utilities
│
└── bin/             # Example programs
    ├── basic_usage.rs
    └── unbricked.rs
```

## Architecture

rust-boy uses a fluent builder pattern to construct assembly programs:

1. **Instruction Enumeration**: All Game Boy instructions are represented as Rust enums
2. **Code Generation**: Instructions are converted to assembly strings via Display trait
3. **Chunk Organization**: Code is organized into logical sections (Main, Functions, Data, Tiles, Tilemap)
4. **Method Chaining**: Fluent API allows natural, readable code construction

## Game Boy Hardware

The generated assembly targets the Game Boy's Sharp LR35902 processor (Z80-like instruction set) and is compatible with:

- Original Game Boy (DMG)
- Game Boy Pocket
- Game Boy Color (backward compatible mode)
- Modern emulators and flash cartridges

## Building for Game Boy

The library generates assembly code in RGBDS format. To compile for actual hardware:

1. Generate assembly with rust-boy
2. Assemble with RGBDS (`rgbasm`)
3. Link with RGBDS (`rgblink`)
4. Fix ROM header (`rgbfix`)
5. Test in emulator or on hardware

```bash
# Example workflow
cargo run --bin your_game > game.asm
rgbasm -L -o game.o game.asm
rgblink -o game.gb game.o
rgbfix -v -p 0xFF game.gb
```

## Development Status

rust-boy is under active development. Current branch: `gbz80-std`

Recent additions:
- Movement system implementation
- Variable and flow control support
- Sprite handling
- Advanced if/else conditionals
- Binary include support (`incbin`)

## Requirements

- RGBDS toolchain (for assembling generated code)

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
