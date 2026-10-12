//! DMG palettes and nonblocking fades. Emit writes in init or VBlank for stable frames.
//! Object colour index 0 remains transparent regardless of its shade. Background and
//! window share BGP. Fade progress is provided by the game, so timing and concurrent
//! fades require no hidden variables or frame waits.

use crate::asm::{Block, Condition, Instr, LabelAllocator, Mem, R8};
use crate::hw;

/// A DMG shade, ordered from white to black.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Shade {
    /// White.
    White = 0,
    /// Light grey.
    Light = 1,
    /// Dark grey.
    Dark = 2,
    /// Black.
    Black = 3,
}

/// Four colour-index-to-shade mappings, packed as a hardware palette byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette(u8);

impl Palette {
    /// Pack shades for colour indices 0, 1, 2 and 3.
    pub const fn new(shades: [Shade; 4]) -> Self {
        Self(
            shades[0] as u8
                | (shades[1] as u8) << 2
                | (shades[2] as u8) << 4
                | (shades[3] as u8) << 6,
        )
    }

    /// Every colour index mapped to one shade.
    pub const fn solid(shade: Shade) -> Self {
        Self((shade as u8) * 0x55)
    }

    /// The hardware byte to use with startup `Palettes` or raw assembly.
    pub const fn byte(self) -> u8 {
        self.0
    }

    /// Shade for a colour index, which must be 0..=3.
    ///
    /// # Panics
    /// If `index` is 4 or more.
    #[track_caller]
    pub fn shade(self, index: u8) -> Shade {
        assert!(index < 4, "palette colour index must be 0..=3");
        match (self.0 >> (index * 2)) & 3 {
            0 => Shade::White,
            1 => Shade::Light,
            2 => Shade::Dark,
            _ => Shade::Black,
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new([Shade::White, Shade::Light, Shade::Dark, Shade::Black])
    }
}
impl From<u8> for Palette {
    fn from(byte: u8) -> Self {
        Self(byte)
    }
}
impl From<Palette> for u8 {
    fn from(palette: Palette) -> Self {
        palette.byte()
    }
}

/// A DMG hardware palette register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteRegister {
    /// Background and window.
    Background,
    /// Sprites without OAMF_PAL1.
    Object0,
    /// Sprites with OAMF_PAL1.
    Object1,
}
impl PaletteRegister {
    /// Hardware register symbol.
    pub const fn register(self) -> hw::Symbol<u16> {
        match self {
            Self::Background => hw::BGP,
            Self::Object0 => hw::OBP0,
            Self::Object1 => hw::OBP1,
        }
    }
}

/// Write a palette. Changes A; keeps flags and every other register.
pub fn set_palette(register: PaletteRegister, palette: impl Into<Palette>) -> Vec<Instr> {
    let mut code = Block::new();
    code.ld_a(palette.into().byte())
        .ldh(Mem::addr(register.register()), R8::A);
    code.into_instrs()
}

/// Four fade stages, from a solid shade into a palette or back out.
/// Each colour index changes by at most one shade per stage. Progress >=3 is the
/// final stage, so a completed fade does not wrap back to its beginning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteFade([Palette; 4]);
impl PaletteFade {
    /// Fade from a solid shade to the target palette.
    /// ```
    /// use rust_boy::prelude::*;
    /// let mut gb = RustBoy::new();
    /// let step = gb.vars.create_u8("wFadeStep", 0);
    /// let fade = PaletteFade::fade_in(Palette::default(), Shade::White);
    /// let mut code = Block::new();
    /// code.emit_all(step.get()).emit_all(gb.fade_palette(PaletteRegister::Background, fade));
    /// gb.on_vblank(code);
    /// # gb.build().unwrap();
    /// ```
    pub fn fade_in(target: impl Into<Palette>, from: Shade) -> Self {
        Self::between(Palette::solid(from), target.into())
    }

    /// Fade from a palette to a solid shade.
    pub fn fade_out(source: impl Into<Palette>, to: Shade) -> Self {
        Self::between(source.into(), Palette::solid(to))
    }

    fn between(source: Palette, target: Palette) -> Self {
        let stages = std::array::from_fn(|step| {
            let shades = std::array::from_fn(|index| {
                let start = source.shade(index as u8) as i16;
                let dest = target.shade(index as u8) as i16;
                let delta = (dest - start).clamp(-(step as i16), step as i16);
                match start + delta {
                    0 => Shade::White,
                    1 => Shade::Light,
                    2 => Shade::Dark,
                    _ => Shade::Black,
                }
            });
            Palette::new(shades)
        });
        Self(stages)
    }

    /// Palette at this progress, clamped to stage 3.
    pub fn at(self, progress: u8) -> Palette {
        self.0[usize::from(progress.min(3))]
    }

    /// Apply the stage selected by A (0..=3; larger values select stage 3).
    /// Changes A and flags; preserves other registers. Uses local allocated labels.
    /// A holds the palette byte afterwards: reload the progress before another fade.
    pub fn emit(self, labels: &LabelAllocator, register: PaletteRegister) -> Vec<Instr> {
        let end = labels.local("fade_end");
        let mut code = Block::new();
        for step in 0..3 {
            let next = labels.local("fade_next");
            code.cp(step)
                .jr_cond(Condition::NZ, &next)
                .emit_all(set_palette(register, self.at(step)))
                .jr(&end)
                .label(&next);
        }
        code.emit_all(set_palette(register, self.at(3))).label(&end);
        code.into_instrs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::test_cpu::TestCpu;

    #[test]
    fn shades_order_from_white_to_black() {
        assert!(Shade::White < Shade::Light && Shade::Light < Shade::Dark);
        assert!(Shade::Dark < Shade::Black);
    }

    #[test]
    fn every_palette_fades_smoothly_to_and_from_every_solid_shade() {
        for byte in 0..=255 {
            for solid in [Shade::White, Shade::Light, Shade::Dark, Shade::Black] {
                let palette = Palette::from(byte);
                for (fade, first, last) in [
                    (
                        PaletteFade::fade_in(palette, solid),
                        Palette::solid(solid),
                        palette,
                    ),
                    (
                        PaletteFade::fade_out(palette, solid),
                        palette,
                        Palette::solid(solid),
                    ),
                ] {
                    assert_eq!(fade.at(0), first);
                    assert_eq!(fade.at(3), last);
                    assert_eq!(fade.at(255), last);
                    for step in 1..=3 {
                        for index in 0..4 {
                            let before = fade.at(step - 1).shade(index) as i16;
                            let after = fade.at(step).shade(index) as i16;
                            let destination = last.shade(index) as i16;
                            assert!((before - after).abs() <= 1);
                            assert!((after - destination).abs() <= (before - destination).abs());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn emitted_fade_clamps_progress_and_preserves_registers() {
        let fade = PaletteFade::fade_in(Palette::from(0x1b), Shade::Black);
        for register in [
            PaletteRegister::Background,
            PaletteRegister::Object0,
            PaletteRegister::Object1,
        ] {
            for progress in [0, 1, 2, 3, 4, 255] {
                let mut cpu = TestCpu::default();
                (cpu.a, cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l) =
                    (progress, 12, 34, 56, 78, 90, 123);
                cpu.run(&fade.emit(&LabelAllocator::new(), register));
                assert_eq!(cpu.mem[register.register().name], fade.at(progress).byte());
                assert_eq!(
                    (cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l),
                    (12, 34, 56, 78, 90, 123)
                );
            }
        }
    }

    #[test]
    fn typed_palette_packs_and_roundtrips_all_bytes() {
        assert_eq!(Palette::default().byte(), 0xe4);
        for byte in 0..=255 {
            let palette = Palette::from(byte);
            assert_eq!(
                Palette::new(std::array::from_fn(|index| palette.shade(index as u8))).byte(),
                byte
            );
        }
        assert!(std::panic::catch_unwind(|| Palette::default().shade(4)).is_err());
    }
}
