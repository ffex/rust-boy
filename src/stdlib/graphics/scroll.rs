//! Background camera snippets. Coordinates wrap on the 256×256 pixel tilemap.
//!
//! These write SCX/SCY immediately. Use them during LCD-off initialization or in a
//! VBlank handler for a stable whole-frame camera; main-loop writes can tear.
//! They change A and flags, preserving all other registers. Screen coordinates
//! are world coordinates minus the scroll, modulo 256. Sprites are not moved.
//! [`scroll_by`] reads the current SCX/SCY: when an `on_stat` handler changes them mid-frame
//! (a split screen), VBlank code reads the value of the last split, not the camera.
//! `GetTileByPixel` takes map pixels: add SCX/SCY to screen coordinates while scrolled.
//! `RustBoy`'s start-up resets SCX/SCY to zero on power-on and soft reset; init code can call
//! [`set_scroll`] to choose a different starting position.

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::routine::Regs;

    fn text(code: &[Instr]) -> Vec<String> {
        code.iter().map(|instr| instr.to_string()).collect()
    }

    #[test]
    fn test_set_scroll_writes_both_registers() {
        assert_eq!(
            text(&set_scroll(250, 3)),
            ["ld a, 250", "ldh [rSCX], a", "ld a, 3", "ldh [rSCY], a"]
        );
    }

    #[test]
    fn test_scroll_by_skips_zero_deltas_and_wraps_negative_ones() {
        assert!(scroll_by(0, 0).is_empty());
        // -5 is added as 251: the 8-bit sum wraps to SCY - 5
        assert_eq!(
            text(&scroll_by(0, -5)),
            ["ldh a, [rSCY]", "add a, 251", "ldh [rSCY], a"]
        );
        assert_eq!(
            text(&scroll_by(10, 0)),
            ["ldh a, [rSCX]", "add a, 10", "ldh [rSCX], a"]
        );
    }

    #[test]
    fn test_follow_point_subtracts_the_anchor() {
        assert_eq!(
            text(&follow_point(80, 72)),
            [
                "ld a, b",
                "sub a, 80",
                "ldh [rSCX], a",
                "ld a, c",
                "sub a, 72",
                "ldh [rSCY], a"
            ]
        );
    }

    #[test]
    fn test_snippets_change_only_a_and_flags() {
        for code in [set_scroll(1, 2), scroll_by(-3, 4), follow_point(80, 72)] {
            let written = Regs::written_by(&code).expect("plain code");
            assert_eq!(written | Regs::AF, Regs::AF, "{:?}", text(&code));
        }
    }
}
