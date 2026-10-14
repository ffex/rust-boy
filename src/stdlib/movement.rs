//! Fixed-point movement routines.

use crate::asm::{Block, R8, R16, R16Stack};
use crate::stdlib::routine::{Regs, Routine};

/// Advance a signed 8.8 position by its signed 8.8 velocity, then add signed 8.8 gravity.
///
/// The integer portion is the high byte and the fractional portion is the low byte. Inputs
/// are `hl` = position, `de` = velocity, and `bc` = gravity. Outputs are updated position
/// in `hl` and updated velocity in `de`; both wrap modulo 65536. The routine clobbers flags
/// and preserves `bc` and `a`.
pub fn step_8_8() -> Routine {
    let mut code = Block::new();
    code.label("StepFixed8_8")
        .add_hl(R16::DE)
        .push(R16Stack::HL)
        .ld(R8::H, R8::D)
        .ld(R8::L, R8::E)
        .add_hl(R16::BC)
        .ld(R8::D, R8::H)
        .ld(R8::E, R8::L)
        .pop(R16Stack::HL)
        .ret();
    Routine::new("StepFixed8_8", code)
        .with_reads(Regs::HL | Regs::DE | Regs::BC)
        .with_returns(Regs::HL | Regs::DE)
        .with_clobbers(Regs::F)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::test_cpu::TestCpu;

    fn read_hl(cpu: &TestCpu) -> u16 {
        u16::from_be_bytes([cpu.h, cpu.l])
    }

    fn write_pair(high: &mut u8, low: &mut u8, value: u16) {
        [*high, *low] = value.to_be_bytes();
    }

    #[test]
    fn fractional_velocity_and_gravity_accumulate() {
        let mut cpu = TestCpu::default();
        write_pair(&mut cpu.h, &mut cpu.l, 0x0100);
        write_pair(&mut cpu.d, &mut cpu.e, 0x0080);
        write_pair(&mut cpu.b, &mut cpu.c, 0x0040);
        let routine = step_8_8();

        cpu.run(routine.body());
        assert_eq!(read_hl(&cpu), 0x0180);
        assert_eq!(u16::from_be_bytes([cpu.d, cpu.e]), 0x00C0);
        cpu.run(routine.body());
        assert_eq!(read_hl(&cpu), 0x0240);
        assert_eq!(u16::from_be_bytes([cpu.d, cpu.e]), 0x0100);
        assert_eq!(u16::from_be_bytes([cpu.b, cpu.c]), 0x0040);
    }
    #[test]
    fn negative_velocity_and_gravity_and_position_wrap() {
        for (position, velocity, gravity, next_position, next_velocity) in [
            (0x0200, 0xFF80, 0xFFC0, 0x0180, 0xFF40),
            (0xFFC0, 0x0080, 0, 0x0040, 0x0080),
        ] {
            let mut cpu = TestCpu::default();
            cpu.a = 0x5A;
            write_pair(&mut cpu.h, &mut cpu.l, position);
            write_pair(&mut cpu.d, &mut cpu.e, velocity);
            write_pair(&mut cpu.b, &mut cpu.c, gravity);
            cpu.run(step_8_8().body());
            assert_eq!(read_hl(&cpu), next_position);
            assert_eq!(u16::from_be_bytes([cpu.d, cpu.e]), next_velocity);
            assert_eq!(u16::from_be_bytes([cpu.b, cpu.c]), gravity);
            assert_eq!(cpu.a, 0x5A);
        }
    }
}
