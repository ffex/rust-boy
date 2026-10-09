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
pub use sprites::{ANIM_DISABLED, CompositeSpriteId, SpriteId, SpriteManager, SpriteSize};
pub use tiles::{TileId, TileManager, TileSource};
pub use variables::{Var, VarId, VarType, VariableManager};

/// The message `f` panics with, for tests that check several panics in one go;
/// panics if `f` returns
#[cfg(test)]
pub(crate) fn panic_message<R>(f: impl FnOnce() -> R) -> String {
    let Err(err) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) else {
        panic!("it did not panic");
    };
    err.downcast_ref::<String>()
        .cloned()
        .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default()
}
