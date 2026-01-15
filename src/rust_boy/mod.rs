//! High-level Game Boy development API
//!
//! This module provides a complete abstraction over assembly generation,
//! hiding all low-level details from the developer.

mod functions;
mod memory;
mod rustboy;
mod sprites;
mod tiles;
mod variables;

pub use functions::BuiltinFunction;
pub use memory::MemoryRegion;
pub use rustboy::RustBoy;
pub use sprites::{SpriteId, SpriteManager};
pub use tiles::{TileId, TileManager, TileSource};
pub use variables::{VarId, VarType, VariableManager};
