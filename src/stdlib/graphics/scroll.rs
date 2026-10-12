//! Background camera snippets. Coordinates wrap on the 256×256 pixel tilemap.
//!
//! These write SCX/SCY immediately. Use them during LCD-off initialization or in a
//! VBlank handler for a stable whole-frame camera; main-loop writes can tear.
//! They change A and flags, preserving all other registers. Screen coordinates
//! are world coordinates minus the scroll, modulo 256. Sprites are not moved.

use crate::asm::{Block, Instr, Mem, R8};
use crate::hw;

/// Set the top-left world pixel of the background viewport.
pub fn set_scroll(x: u8, y: u8) -> Vec<Instr> {
    let mut code = Block::new();
    code.ld_a(x).ldh(Mem::addr(hw::SCX), R8::A);
    code.ld_a(y).ldh(Mem::addr(hw::SCY), R8::A);
    code.into_instrs()
}

/// Move the viewport by signed pixel deltas, wrapping at the map edges.
/// Positive X moves the camera right, positive Y moves it down.
pub fn scroll_by(dx: i8, dy: i8) -> Vec<Instr> {
    let mut code = Block::new();
    for (register, delta) in [(hw::SCX, dx), (hw::SCY, dy)] {
        if delta != 0 {
            code.ldh(R8::A, Mem::addr(register))
                .add(delta as u8)
                .ldh(Mem::addr(register), R8::A);
        }
    }
    code.into_instrs()
}

/// Keep world pixel (B = X, C = Y) at a screen anchor (in pixels).
/// For a centered target, use `(hw::SCRN_X.value / 2, hw::SCRN_Y.value / 2)`.
/// The target uses world pixels, without the OAM coordinate offsets.
pub fn follow_point(anchor_x: u8, anchor_y: u8) -> Vec<Instr> {
    let mut code = Block::new();
    code.ld(R8::A, R8::B)
        .sub(anchor_x)
        .ldh(Mem::addr(hw::SCX), R8::A);
    code.ld(R8::A, R8::C)
        .sub(anchor_y)
        .ldh(Mem::addr(hw::SCY), R8::A);
    code.into_instrs()
}
