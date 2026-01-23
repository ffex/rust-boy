//! High-level Game Boy development API
//!
//! This module provides a complete abstraction over assembly generation,
//! hiding all low-level details from the developer.

mod animations;
mod functions;
mod inputs;
mod memory;
mod rustboy;
mod sprites;
mod tiles;
mod variables;

pub use animations::AnimationType;
pub use functions::BuiltinFunction;
pub use inputs::InputManager;
pub use memory::MemoryRegion;
pub use rustboy::RustBoy;
pub use sprites::{ANIM_DISABLED, CompositeSpriteId, SpriteId, SpriteManager};
pub use tiles::{TileId, TileManager, TileSource};
pub use variables::{VarId, VarType, VariableManager};
