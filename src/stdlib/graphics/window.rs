//! Window snippets. Emit during LCD-off initialization or VBlank for stable frames.
//! The window shares background tiles and BGP, ignores SCX/SCY, and needs LCDC bit 0
//! on DMG. These snippets change A and flags only; LCDC operations preserve other bits.

use crate::asm::{Block, Expr, Instr, Mem, R8};
use crate::hw;

/// The hardware `(WX, WY)` of the window's top-left screen pixel `(x, y)`: WX = X + 7.
/// X=159 is excluded because WX=166 glitches on DMG.
///
/// # Panics
/// If X is not 0..=158 or Y not 0..=143.
pub fn screen_to_raw(x: u8, y: u8) -> (u8, u8) {
    assert!(x < hw::SCRN_X.value - 1, "window X must be 0..=158");
    assert!(y < hw::SCRN_Y.value, "window Y must be 0..=143");
    (x + hw::WINDOW_X_OFFSET, y)
}

/// Set the window's top-left screen pixel (see [`screen_to_raw`]).
/// Use [`set_position_raw`] for clipping or off-screen placement.
///
/// # Panics
/// If X is not 0..=158 or Y not 0..=143.
pub fn set_position(x: u8, y: u8) -> Vec<Instr> {
    let (wx, wy) = screen_to_raw(x, y);
    set_position_raw(wx, wy)
}

/// Write hardware WX/WY directly. WX=7 means screen X=0; WY>=144 or WX>=167
/// hides the window. WX=0 and WX=166 have hardware rendering quirks on DMG.
pub fn set_position_raw(wx: u8, wy: u8) -> Vec<Instr> {
    let mut code = Block::new();
    code.ld_a(wx).ldh(Mem::addr(hw::WX), R8::A);
    code.ld_a(wy).ldh(Mem::addr(hw::WY), R8::A);
    code.into_instrs()
}

/// Enable or disable the window (LCDC bit 5), preserving every other LCDC bit.
/// Enabling does not change bit 0: the background/window must also be enabled on DMG.
/// Enabling writes neither WX/WY nor the window map: without a position set before, the
/// window shows where WX/WY are (WX=0, WY=0 after the boot ROM), from the map LCDC bit 6 selects.
pub fn set_enabled(enabled: bool) -> Vec<Instr> {
    let mut code = Block::new();
    code.ldh(R8::A, Mem::addr(hw::LCDC));
    if enabled {
        code.or(hw::LCDCF_WINON);
    } else {
        code.and(Expr::low(!Expr::from(hw::LCDCF_WINON)));
    }
    code.ldh(Mem::addr(hw::LCDC), R8::A);
    code.into_instrs()
}

/// Select the window map without changing the background map or other LCDC bits.
pub fn select_tilemap(map_9c00: bool) -> Vec<Instr> {
    let mut code = Block::new();
    code.ldh(R8::A, Mem::addr(hw::LCDC))
        .and(Expr::low(!Expr::from(hw::LCDCF_WIN9C00)));
    if map_9c00 {
        code.or(hw::LCDCF_WIN9C00);
    }
    code.ldh(Mem::addr(hw::LCDC), R8::A);
    code.into_instrs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::routine::Regs;

    #[test]
    fn positions_reject_unstable_or_offscreen_visible_coordinates() {
        for (x, y) in [(159, 0), (255, 0), (0, 144), (0, 255)] {
            assert!(std::panic::catch_unwind(|| set_position(x, y)).is_err());
        }
        assert!(!set_position(158, 143).is_empty());
    }

    #[test]
    fn lcdc_masks_are_symbolic() {
        let text =
            |code: Vec<Instr>| -> Vec<String> { code.iter().map(|i| i.to_string()).collect() };
        assert_eq!(
            text(set_enabled(false)),
            [
                "ldh a, [rLCDC]",
                "and a, LOW(~LCDCF_WINON)",
                "ldh [rLCDC], a"
            ]
        );
        assert_eq!(
            text(select_tilemap(true)),
            [
                "ldh a, [rLCDC]",
                "and a, LOW(~LCDCF_WIN9C00)",
                "or a, LCDCF_WIN9C00",
                "ldh [rLCDC], a"
            ]
        );
    }

    #[test]
    fn snippets_preserve_registers_except_a_and_flags() {
        for code in [
            set_position(0, 112),
            set_position_raw(255, 255),
            set_enabled(true),
            set_enabled(false),
            select_tilemap(true),
            select_tilemap(false),
        ] {
            assert_eq!(Regs::written_by(&code).unwrap() | Regs::AF, Regs::AF);
        }
    }
}
