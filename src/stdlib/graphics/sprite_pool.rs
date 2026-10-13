//! Runtime allocation of a reserved range in shadow OAM. Occupancy is one WRAM byte
//! per slot (0 free, 1 allocated), initially zero. Use one producer in init/main;
//! interrupt handlers may DMA the shadow but must not allocate/free/write this pool.
//! Slots use relative indices, lowest free first. Release clears OAM before publishing
//! a slot free; writes publish Y last. No operation changes IME.

use crate::asm::{Block, Condition, Expr, Mem, R8, R16, R16Stack};
use crate::hw;
use crate::stdlib::graphics::oam::OamTarget;
use crate::stdlib::routine::{Regs, Routine};

fn pointer(code: &mut Block, base: Expr, stride: u8) {
    code.push(R16Stack::BC)
        .ld(R8::C, R8::A)
        .ld_b(0)
        .ld(R16::HL, base);
    for _ in 0..stride {
        code.add_hl(R16::BC);
    }
    code.pop(R16Stack::BC);
}

fn validate(code: &mut Block, count: u8, occupancy: &str) {
    code.cp(count)
        .jr_cond(Condition::NC, ".invalid")
        .push(R16Stack::AF);
    pointer(code, Expr::sym(occupancy), 1);
    code.ld(R8::A, R8::AtHl)
        .and(R8::A)
        .jr_cond(Condition::NZ, ".allocated")
        .pop(R16Stack::AF)
        .jr(".invalid")
        .label(".allocated")
        .pop(R16Stack::AF);
}

fn finish(mut code: Block, name: &str, reads: Regs, returns: Regs, clobbers: Regs) -> Routine {
    code.and(R8::A).ret().label(".invalid").scf().ret();
    Routine::new(name, code)
        .with_reads(reads)
        .with_returns(returns)
        .with_clobbers(clobbers)
}

/// Four pool routines: allocate, release, write and hide. Relative slot index is A.
/// `first`/`count` must describe a nonempty OAM range; occupancy must have count bytes
/// initialized to zero. `size_8x16` makes writes reject odd tile indices.
/// Routine names are `Pool{name}Allocate`, `Release`, `Write`, `Hide`.
///
/// Allocation returns A=index and HL=the cleared, hidden OAM entry, carry clear;
/// full returns A=255 and carry set (HL unspecified). It changes AF/BC/HL, keeps DE.
/// Other operations reject invalid/free indices with carry and no memory writes.
/// Release clears every entry byte then frees it; hide writes Y=0 without freeing.
/// Write takes B=screen X, C=screen Y, D=tile, E=flags, preserves BC/DE and writes Y
/// last. X>247/Y>239 are rejected rather than wrapping. Release/write/hide change AF/HL.
///
/// # Panics
/// If `count` is 0 or the range `first..first + count` goes past the 40 OAM entries.
#[track_caller]
pub fn routines(
    name: &str,
    first: u8,
    count: u8,
    occupancy: &str,
    size_8x16: bool,
) -> Vec<Routine> {
    assert!(
        count > 0 && u16::from(first) + u16::from(count) <= u16::from(hw::OAM_COUNT.value),
        "invalid OAM pool range"
    );
    let base = OamTarget::Shadow.entry(first, hw::OAMA_Y);
    let allocate = format!("Pool{name}Allocate");
    let mut code = Block::new();
    code.label(&allocate)
        .ld(R16::HL, Expr::sym(occupancy))
        .ld_b(count)
        .ld_c(0)
        .label(".search")
        .ld(R8::A, R8::AtHl)
        .and(R8::A)
        .jr_cond(Condition::Z, ".found")
        .inc(R16::HL)
        .inc(R8::C)
        .dec(R8::B)
        .jr_cond(Condition::NZ, ".search")
        .ld_a(255)
        .scf()
        .ret()
        .label(".found")
        .ld(R8::AtHl, 1)
        .ld(R8::A, R8::C);
    pointer(&mut code, base.clone(), hw::OAM_ENTRY_SIZE.value);
    code.xor(R8::A);
    for _ in 0..hw::OAM_ENTRY_SIZE.value {
        code.ld(Mem::Hli, R8::A);
    }
    for _ in 0..hw::OAM_ENTRY_SIZE.value {
        code.dec(R16::HL);
    }
    code.ld(R8::A, R8::C).and(R8::A).ret();
    let allocate = Routine::new(&allocate, code)
        .with_returns(Regs::A | Regs::F | Regs::HL)
        .with_clobbers(Regs::BC);

    let release = format!("Pool{name}Release");
    let mut code = Block::new();
    code.label(&release);
    validate(&mut code, count, occupancy);
    code.push(R16Stack::HL);
    pointer(&mut code, base.clone(), hw::OAM_ENTRY_SIZE.value);
    code.xor(R8::A);
    for _ in 0..hw::OAM_ENTRY_SIZE.value {
        code.ld(Mem::Hli, R8::A);
    }
    code.pop(R16Stack::HL).ld(R8::AtHl, R8::A);
    let release = finish(code, &release, Regs::A, Regs::F, Regs::A | Regs::HL);

    let write = format!("Pool{name}Write");
    let mut code = Block::new();
    code.label(&write);
    validate(&mut code, count, occupancy);
    code.push(R16Stack::AF)
        .ld(R8::A, R8::B)
        .cp(u8::MAX - hw::OAM_X_OFFSET + 1)
        .jr_cond(Condition::NC, ".bad_position")
        .ld(R8::A, R8::C)
        .cp(u8::MAX - hw::OAM_Y_OFFSET + 1)
        .jr_cond(Condition::NC, ".bad_position");
    if size_8x16 {
        code.bit(0, R8::D).jr_cond(Condition::NZ, ".bad_position");
    }
    code.pop(R16Stack::AF);
    pointer(&mut code, base.clone(), hw::OAM_ENTRY_SIZE.value);
    code.inc(R16::HL)
        .ld(R8::A, R8::B)
        .add(hw::OAM_X_OFFSET)
        .ld(Mem::Hli, R8::A)
        .ld(R8::A, R8::D)
        .ld(Mem::Hli, R8::A)
        .ld(R8::AtHl, R8::E);
    for _ in 0..hw::OAM_ENTRY_SIZE.value - 1 {
        code.dec(R16::HL);
    }
    code.ld(R8::A, R8::C)
        .add(hw::OAM_Y_OFFSET)
        .ld(R8::AtHl, R8::A)
        .and(R8::A)
        .ret()
        .label(".bad_position")
        .pop(R16Stack::AF)
        .jr(".invalid");
    let write = finish(
        code,
        &write,
        Regs::A | Regs::BC | Regs::DE,
        Regs::F,
        Regs::A | Regs::HL,
    );

    let hide = format!("Pool{name}Hide");
    let mut code = Block::new();
    code.label(&hide);
    validate(&mut code, count, occupancy);
    pointer(&mut code, base, hw::OAM_ENTRY_SIZE.value);
    code.xor(R8::A).ld(R8::AtHl, R8::A);
    let hide = finish(code, &hide, Regs::A, Regs::F, Regs::A | Regs::HL);
    vec![allocate, release, write, hide]
}
