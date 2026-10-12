//! Unsigned arithmetic routines for the SM83.
//!
//! The 16-bit helpers operate on `hl` and `de`. The byte multiply accepts `a` and `b`;
//! division accepts a 16-bit `hl` dividend and an 8-bit divisor in `c`.

use crate::asm::{Block, Condition, R8, R16};
use crate::stdlib::routine::{Regs, Routine};

/// Add `de` to `hl`; the 16-bit result is in `hl` and carry reports overflow.
///
/// Reads: `hl`, `de`. Returns: `hl`. Clobbers: flags (except the hardware-preserved Z flag).
pub fn add_u16() -> Routine {
    let mut code = Block::new();
    code.label("AddU16").add_hl(R16::DE).ret();
    Routine::new("AddU16", code)
        .with_reads(Regs::HL | Regs::DE)
        .with_returns(Regs::HL)
        .with_clobbers(Regs::F)
}

/// Subtract `de` from `hl`; the 16-bit result is in `hl` and carry reports a borrow.
///
/// Reads: `hl`, `de`. Returns: `hl`. Clobbers: `a`, flags.
pub fn sub_u16() -> Routine {
    let mut code = Block::new();
    code.label("SubU16")
        .ld(R8::A, R8::L)
        .sub(R8::E)
        .ld(R8::L, R8::A)
        .ld(R8::A, R8::H)
        .sbc(R8::D)
        .ld(R8::H, R8::A)
        .ret();
    Routine::new("SubU16", code)
        .with_reads(Regs::HL | Regs::DE)
        .with_returns(Regs::HL)
        .with_clobbers(Regs::A | Regs::F)
}

/// Compare unsigned `hl` with `de` without changing either pair.
///
/// On return Z is set when equal and carry is set when `hl < de`. Reads: `hl`, `de`.
/// Returns: flags. Clobbers: `a`.
pub fn compare_u16() -> Routine {
    let mut code = Block::new();
    code.label("CompareU16")
        .ld(R8::A, R8::H)
        .cp(R8::D)
        .jr_cond(Condition::NZ, ".done")
        .ld(R8::A, R8::L)
        .cp(R8::E)
        .label(".done")
        .ret();
    Routine::new("CompareU16", code)
        .with_reads(Regs::HL | Regs::DE)
        .with_returns(Regs::F)
        .with_clobbers(Regs::A)
}

/// Multiply unsigned bytes `a` and `b`; return the exact 16-bit product in `hl`.
///
/// Uses shift-and-add in at most eight iterations. Reads: `a`, `b`. Returns: `hl`.
/// Clobbers: `a`, `c`, `de`, flags; preserves `b`.
pub fn multiply_u8() -> Routine {
    let mut code = Block::new();
    code.label("MultiplyU8")
        .ld(R8::C, R8::B)
        .ld(R8::E, R8::A)
        .ld_d(0)
        .ld_hl(0)
        .label(".loop")
        .ld(R8::A, R8::C)
        .and(1)
        .jr_cond(Condition::Z, ".skip_add")
        .add_hl(R16::DE)
        .label(".skip_add")
        .srl(R8::C)
        .sla(R8::E)
        .rl(R8::D)
        .ld(R8::A, R8::C)
        .or(R8::A)
        .jr_cond(Condition::NZ, ".loop")
        .ret();
    Routine::new("MultiplyU8", code)
        .with_reads(Regs::A | Regs::B)
        .with_returns(Regs::HL)
        .with_clobbers(Regs::A | Regs::C | Regs::DE | Regs::F)
}

/// Divide unsigned `hl` by the byte in `c`.
///
/// On success, return the quotient in `hl`, remainder in `a`, and clear carry. Division
/// by zero returns `$FFFF` in `hl`, zero in `a`, and sets carry. Reads: `hl`, `c`.
/// Returns: `hl`, `a`, flags. Clobbers: `b`, `de`.
pub fn divide_u16_by_u8() -> Routine {
    let mut code = Block::new();
    code.label("DivideU16ByU8")
        .ld(R8::A, R8::C)
        .or(R8::A)
        .jr_cond(Condition::NZ, ".divide")
        .ld_hl(u16::MAX)
        .ld_a(0)
        .scf()
        .ret()
        .label(".divide")
        .ld(R16::DE, 0)
        .ld_b(16)
        .label(".step")
        .add_hl(R16::HL)
        .rl(R8::E)
        .rl(R8::D)
        .ld(R8::A, R8::D)
        .or(R8::A)
        .jr_cond(Condition::NZ, ".subtract")
        .ld(R8::A, R8::E)
        .cp(R8::C)
        .jr_cond(Condition::C, ".next")
        .label(".subtract")
        .ld(R8::A, R8::E)
        .sub(R8::C)
        .ld(R8::E, R8::A)
        .ld(R8::A, R8::D)
        .sbc(0)
        .ld(R8::D, R8::A)
        .set(0, R8::L)
        .label(".next")
        .dec(R8::B)
        .jr_cond(Condition::NZ, ".step")
        .ld(R8::A, R8::E)
        .and(R8::A)
        .ret();
    Routine::new("DivideU16ByU8", code)
        .with_reads(Regs::HL | Regs::C)
        .with_returns(Regs::HL | Regs::AF)
        .with_clobbers(Regs::B | Regs::DE)
}

/// All arithmetic routines, in stable order.
pub fn routines() -> [Routine; 5] {
    [
        add_u16(),
        sub_u16(),
        compare_u16(),
        multiply_u8(),
        divide_u16_by_u8(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::test_cpu::TestCpu;

    fn run(routine: &Routine, cpu: &mut TestCpu) {
        cpu.run(routine.body());
    }

    fn hl(cpu: &TestCpu) -> u16 {
        u16::from_be_bytes([cpu.h, cpu.l])
    }

    fn set_hl(cpu: &mut TestCpu, value: u16) {
        [cpu.h, cpu.l] = value.to_be_bytes();
    }

    fn set_de(cpu: &mut TestCpu, value: u16) {
        [cpu.d, cpu.e] = value.to_be_bytes();
    }

    #[test]
    fn add_subtract_and_compare_cover_carry_and_borrow() {
        let mut cpu = TestCpu::default();
        set_hl(&mut cpu, u16::MAX);
        set_de(&mut cpu, 1);
        run(&add_u16(), &mut cpu);
        assert_eq!(hl(&cpu), 0);
        assert!(cpu.carry);

        set_hl(&mut cpu, 0);
        set_de(&mut cpu, 1);
        run(&sub_u16(), &mut cpu);
        assert_eq!(hl(&cpu), u16::MAX);
        assert!(cpu.carry);

        set_hl(&mut cpu, 0x1200);
        set_de(&mut cpu, 0x12FF);
        run(&compare_u16(), &mut cpu);
        assert!(cpu.carry);
        assert!(!cpu.zero);
        set_hl(&mut cpu, 0xABCD);
        set_de(&mut cpu, 0xABCD);
        run(&compare_u16(), &mut cpu);
        assert!(cpu.zero);
        assert!(!cpu.carry);
        assert_eq!(hl(&cpu), 0xABCD);
    }

    #[test]
    fn multiply_handles_zero_and_the_largest_product() {
        let mut cpu = TestCpu::default();
        cpu.a = 0xFF;
        cpu.b = 0xFF;
        run(&multiply_u8(), &mut cpu);
        assert_eq!(hl(&cpu), 0xFE01);
        assert_eq!(cpu.b, 0xFF);

        cpu.a = 0;
        cpu.b = 0x91;
        run(&multiply_u8(), &mut cpu);
        assert_eq!(hl(&cpu), 0);
    }

    #[test]
    fn divide_handles_remainders_large_values_and_zero_divisors() {
        let mut cpu = TestCpu::default();
        set_hl(&mut cpu, u16::MAX);
        cpu.c = 255;
        run(&divide_u16_by_u8(), &mut cpu);
        assert_eq!(hl(&cpu), 257);
        assert_eq!(cpu.a, 0);
        assert!(!cpu.carry);

        set_hl(&mut cpu, 1000);
        cpu.c = 7;
        run(&divide_u16_by_u8(), &mut cpu);
        assert_eq!(hl(&cpu), 142);
        assert_eq!(cpu.a, 6);
        assert!(!cpu.carry);

        set_hl(&mut cpu, 1234);
        cpu.c = 0;
        run(&divide_u16_by_u8(), &mut cpu);
        assert_eq!(hl(&cpu), u16::MAX);
        assert_eq!(cpu.a, 0);
        assert!(cpu.carry);
    }
}
