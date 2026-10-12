//! The shadow OAM and the OAM DMA (the fix of B12, CONTEXT.md)
//!
//! The PPU reads OAM in modes 2 and 3 of each visible line: then a CPU write to OAM is
//! dropped and a read gives `$FF`. Code that writes OAM directly works only while it runs
//! in VBlank or HBlank (or with the LCD off). So the sprites live in a copy of OAM in WRAM,
//! the **shadow OAM** ([`SHADOW_OAM`], 160 bytes on a page of its own), which game code
//! reads and writes at any time, and once a frame, in VBlank, an **OAM DMA** copies it to
//! OAM in 160 M-cycles.
//!
//! During a DMA the CPU can reach only HRAM: a fetch from ROM reads `$FF`. So the code that
//! starts it runs from HRAM: [`init_oam_dma`] (`InitOamDma`) copies it there at start-up,
//! to [`OAM_DMA`] (`hOAMDMA`, [`oam_dma_size`] bytes), and [`run_oam_dma`] calls it with the
//! shadow OAM's page in `a`. It writes `rDMA`, then waits 160 M-cycles in a loop of 40
//! `dec a` / `jr nz` (4 M-cycles each), and returns once the copy is over. It must run
//! with interrupts off (an interrupt would jump to ROM), which a VBlank handler does.
//!
//! [`OamTarget`] says which copy sprite code reads and writes: `RustBoy` uses the shadow
//! OAM for every sprite, and a `stdlib` program chooses ([`Sprite::in_shadow_oam`]).
//!
//! [`Sprite::in_shadow_oam`]: super::sprites::Sprite::in_shadow_oam

use crate::asm::{Block, Condition, Expr, Instr, Mem, R8, R16, Section};
use crate::hw;
use crate::stdlib::routine::{Regs, Routine};

/// The label of the shadow OAM: 160 bytes of WRAM, aligned to a page (`ALIGN[8]`), in the
/// layout of OAM (40 entries of Y, X, tile, flags)
pub const SHADOW_OAM: &str = "wShadowOAM";

/// The label of the OAM DMA routine in HRAM ([`oam_dma_size`] bytes), which
/// [`init_oam_dma`] copies there
pub const OAM_DMA: &str = "hOAMDMA";

/// The label of the OAM DMA routine's code in ROM, inside `InitOamDma` (it is copied to
/// HRAM, never run there); it ends at `OamDmaCodeEnd`
pub const OAM_DMA_CODE: &str = "OamDmaCode";

/// The label that ends the OAM DMA routine's code in ROM, after [`OAM_DMA_CODE`]
pub const OAM_DMA_CODE_END: &str = "OamDmaCodeEnd";

/// Which copy of OAM sprite code reads and writes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum OamTarget {
    /// OAM itself, `_OAMRAM`: the code must run in VBlank or HBlank, or with the LCD off
    /// (B12, CONTEXT.md)
    #[default]
    Oam,
    /// The shadow OAM in WRAM, [`SHADOW_OAM`]: at any time; an OAM DMA in VBlank shows it
    Shadow,
}

impl OamTarget {
    /// The address of entry 0: `_OAMRAM` or `wShadowOAM`
    pub fn base(self) -> Expr {
        match self {
            OamTarget::Oam => Expr::from(hw::OAMRAM),
            OamTarget::Shadow => Expr::sym(SHADOW_OAM),
        }
    }

    /// The address of byte `byte` (`hw::OAMA_Y`, `hw::OAMA_X`, ...) of entry `index`:
    /// `_OAMRAM+5`, `wShadowOAM+5` for entry 1, X
    ///
    /// # Panics
    /// If `index` is past OAM (40 entries) or `byte` is not one of the `OAMA_*` bytes
    /// ([`hw::oam_offset`]).
    #[track_caller]
    pub fn entry(self, index: u8, byte: hw::Symbol<u8>) -> Expr {
        self.base() + hw::oam_offset(index, byte)
    }
}

/// The code of the OAM DMA routine, from its label [`OAM_DMA_CODE`] to `OamDmaCodeEnd`:
/// start a DMA from page `a` (`ldh [rDMA], a`), wait the 160 M-cycles of the copy (40 times
/// `dec a` + `jr nz`, 4 M-cycles each), return. Only a relative jump, so it runs anywhere.
fn oam_dma_code() -> Vec<Instr> {
    let mut asm = Block::new();
    asm.label(OAM_DMA_CODE);
    asm.ldh(Mem::addr(hw::DMA), R8::A);
    // 160 M-cycles: 40 loops of `dec a` (1) and a taken `jr nz` (3)
    asm.ld_a((hw::OAM_DMA_MCYCLES / 4) as u8);
    asm.label(".wait");
    asm.dec(R8::A);
    asm.jr_cond(Condition::NZ, ".wait");
    asm.ret();
    asm.label(OAM_DMA_CODE_END);
    asm.into_instrs()
}

/// The size in bytes of the OAM DMA routine, which [`OAM_DMA`] holds in HRAM
pub fn oam_dma_size() -> u16 {
    oam_dma_code()
        .iter()
        .map(|instr| instr.size().expect("the OAM DMA code has a known size"))
        .sum::<usize>() as u16
}

/// The `InitOamDma` routine: copy the OAM DMA routine to HRAM, at [`OAM_DMA`]
///
/// Call it once at start-up, before the first [`run_oam_dma`]. The routine's code follows
/// it in ROM, at [`OAM_DMA_CODE`]: the copy goes through `ldh [c], a`.
/// - Reads and returns nothing.
/// - Clobbers: `a`, `bc`, `hl` and the flags.
/// - Needs: the `hOAMDMA` bytes in HRAM ([`oam_dma_section`]).
pub fn init_oam_dma() -> Routine {
    let end = Expr::sym(OAM_DMA_CODE_END);
    let mut asm = Block::new();
    asm.comment("Copy the OAM DMA routine to HRAM: during a DMA the CPU reaches only HRAM");
    asm.label("InitOamDma");
    asm.ld(R16::HL, OAM_DMA_CODE);
    asm.ld(R8::B, end - OAM_DMA_CODE);
    asm.ld(R8::C, Expr::low(OAM_DMA));
    asm.label(".copy");
    asm.ld(R8::A, Mem::Hli);
    asm.ldh(Mem::C, R8::A);
    asm.inc(R8::C);
    asm.dec(R8::B);
    asm.jr_cond(Condition::NZ, ".copy");
    asm.ret();
    asm.comment("The OAM DMA routine (run from HRAM): a = the source page");
    asm.emit_all(oam_dma_code());
    Routine::new("InitOamDma", asm).with_clobbers(Regs::A | Regs::BC | Regs::HL | Regs::F)
}

/// Copy the shadow OAM to OAM with an OAM DMA: `ld a, HIGH(wShadowOAM)`, `call hOAMDMA`
///
/// It takes 176 M-cycles with the call: run it in VBlank (or with the LCD off), with
/// interrupts off. Changes `a` and the flags.
pub fn run_oam_dma() -> Vec<Instr> {
    let mut asm = Block::new();
    asm.ld(R8::A, Expr::high(SHADOW_OAM));
    asm.call(OAM_DMA);
    asm.into_instrs()
}

/// The WRAM section of the shadow OAM: `wShadowOAM: ds 160`, at `address` (which must be
/// a page, `$XX00`), or floating with `ALIGN[8]`
///
/// # Panics
/// If `address` is not the start of a page.
#[track_caller]
pub fn shadow_oam_section(address: Option<u16>) -> Vec<Instr> {
    let section = Section::wram0("Shadow OAM");
    let section = match address {
        Some(address) => {
            assert!(
                address.trailing_zeros() >= u32::from(hw::OAM_DMA_SOURCE_ALIGN),
                "the shadow OAM at ${:04X}: an OAM DMA copies a page, so it must start at $XX00",
                address
            );
            section.at(address)
        }
        None => section.align(hw::OAM_DMA_SOURCE_ALIGN),
    };
    let mut asm = Block::new();
    asm.section(section);
    asm.label(SHADOW_OAM);
    asm.ds(&hw::OAM_SIZE.to_string());
    asm.into_instrs()
}

/// The HRAM section of the OAM DMA routine: `hOAMDMA: ds 8`, at `address` or floating
pub fn oam_dma_section(address: Option<u16>) -> Vec<Instr> {
    let section = Section::hram("OAM DMA");
    let section = match address {
        Some(address) => section.at(address),
        None => section,
    };
    let mut asm = Block::new();
    asm.section(section);
    asm.label(OAM_DMA);
    asm.ds(&oam_dma_size().to_string());
    asm.into_instrs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(code: &[Instr]) -> Vec<String> {
        code.iter().map(|instr| instr.to_string()).collect()
    }

    #[test]
    fn test_the_dma_routine_waits_160_m_cycles() {
        // ldh [rDMA], a (2 bytes), ld a, 40 (2), dec a (1), jr nz (2), ret (1)
        assert_eq!(oam_dma_size(), 8);
        let code = text(&oam_dma_code());
        assert!(code.contains(&"ldh [rDMA], a".to_string()), "{:?}", code);
        assert!(code.contains(&"ld a, 40".to_string()), "{:?}", code);
        // The loop: 40 x (dec a: 1 M-cycle, jr nz taken: 3) = 160 M-cycles, minus the last
        // jr not taken (2): the copy is over when it returns
        assert_eq!(40 * 4, hw::OAM_DMA_MCYCLES);
    }

    #[test]
    fn test_target_addresses() {
        assert_eq!(OamTarget::Oam.entry(1, hw::OAMA_X).to_string(), "_OAMRAM+5");
        assert_eq!(
            OamTarget::Shadow.entry(39, hw::OAMA_FLAGS).to_string(),
            "wShadowOAM+159"
        );
        assert_eq!(OamTarget::default(), OamTarget::Oam);
    }

    #[test]
    fn test_sections() {
        assert_eq!(
            text(&shadow_oam_section(Some(0xCF00)))[..2],
            [
                "SECTION \"Shadow OAM\", WRAM0[$CF00]".to_string(),
                "wShadowOAM:".to_string()
            ]
        );
        assert_eq!(
            text(&shadow_oam_section(None))[0],
            "SECTION \"Shadow OAM\", WRAM0, ALIGN[8]"
        );
        assert_eq!(
            text(&oam_dma_section(Some(0xFFB8))),
            ["SECTION \"OAM DMA\", HRAM[$FFB8]", "hOAMDMA:", "ds 8"]
        );
    }

    #[test]
    #[should_panic(expected = "must start at $XX00")]
    fn test_the_shadow_oam_is_on_a_page() {
        shadow_oam_section(Some(0xC0A0));
    }
}
