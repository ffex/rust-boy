//! The types most programs use, in one import: `use rust_boy::prelude::*;`
//!
//! It brings the engine ([`RustBoy`], its settings, its errors with what they hold
//! (`Definition`, `MemoryRegion`), sprites, tiles, variables, inputs), the control flow
//! and routines of [`stdlib`](crate::stdlib) (`If`, `IfA`, `IfConst`, `IfCall`, `Call`,
//! `Routine`, `Regs`, `PadButton`), the building blocks of [`asm`](crate::asm) for code
//! written by hand (`Block`, `Expr`, `R8`, `R16`, `Mem`, `Condition`, `Section`, and
//! `Emittable` with the `LabelAllocator` its `emit` takes), and the [`hw`]
//! module itself (`hw::LCDC`, ...).
//! Everything else is in its layer: [`asm`](crate::asm), [`stdlib`](crate::stdlib),
//! [`engine`](crate::engine).
//!
//! # Example
//! ```
//! use rust_boy::prelude::*;
//!
//! fn main() -> Result<(), Error> {
//!     let mut gb = RustBoy::with_config(RustBoyConfig::default().sprite_size(SpriteSize::Size8x8));
//!     let ball = gb.add_sprite("Ball", TileSource::from_raw(&[["$FF"; 8]]), 80, 72, 0);
//!     let speed = gb.vars.create_hram_u8("hSpeed", 1);
//!
//!     let mut inputs = InputManager::new();
//!     inputs.on_press(PadButton::Right, gb.sprites.move_right_limit(ball, 1, 160));
//!     gb.add_inputs(inputs);
//!
//!     // Every frame: if hSpeed is 1, make it 2
//!     gb.add_to_main_loop(speed.get());
//!     gb.add_to_main_loop(IfA::eq(1, speed.set(2)));
//!
//!     let out = gb.build()?;
//!     assert!(out.contains("ldh [hSpeed], a"));
//!     Ok(())
//! }
//! ```

pub use crate::asm::{
    Asm, Block, Condition, Emittable, Expr, Instr, LabelAllocator, Mem, R8, R16, Section,
};
pub use crate::engine::{
    ANIM_DISABLED, AnimationType, BuiltinFunction, Chunk, CompositeSpriteId, Definition, Error,
    InputManager, Layout, Lcdc, MemoryRegion, Palettes, RustBoy, RustBoyConfig, SpriteId,
    SpriteSize, TileSource, TilemapArea, Var, VarType,
};
pub use crate::hw;
pub use crate::stdlib::flow::{Call, If, IfA, IfCall, IfConst};
pub use crate::stdlib::inputs::PadButton;
pub use crate::stdlib::routine::{Regs, Routine};
