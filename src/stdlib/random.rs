//! Deterministic pseudo-random bytes for games.
//!
//! [`random_u8`] advances an 8-bit maximal-length LFSR and returns the new state in A.
//! It produces a repeatable sequence, not hardware entropy or cryptographic randomness.
//! Its state is `wRandomSeed`; programs may define that byte before registering the
//! routine to choose a nonzero starting state. A zero state is replaced with `$A7`.

use crate::asm::{Block, Condition, R8};
use crate::stdlib::routine::{Regs, Routine};

/// Return the next pseudo-random byte in A, updating `wRandomSeed`.
///
/// The 8-bit LFSR has a period of 255 for every nonzero seed. Its default seed is `$A7`
/// when the stored state is zero. A zero seed therefore never traps the generator in the
/// all-zero state. For a chosen sequence, define `wRandomSeed` with a nonzero initial value
/// before calling `gb.call_routine(&random_u8())`.
///
/// - Reads and returns: A (the new pseudo-random byte).
/// - Clobbers: the flags.
/// - Needs: `wRandomSeed` (created as a zero-initialized WRAM byte if not already defined).
pub fn random_u8() -> Routine {
    let mut asm = Block::new();
    asm.label("RandomU8");
    asm.ld_a_addr_def("wRandomSeed");
    asm.or(R8::A);
    asm.jr_cond(Condition::NZ, ".step");
    asm.ld_a(0xA7);
    asm.label(".step");
    asm.srl(R8::A);
    asm.jr_cond(Condition::NC, ".store");
    asm.xor(0xB8);
    asm.label(".store");
    asm.ld_addr_def_a("wRandomSeed");
    asm.ret();

    Routine::new("RandomU8", asm)
        .with_variable("wRandomSeed")
        .with_returns(Regs::A)
        .with_clobbers(Regs::F)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::program::Asm;

    #[test]
    fn random_routine_emits_the_lfsr_and_declares_its_state() {
        let routine = random_u8();
        assert_eq!(routine.name(), "RandomU8");
        assert_eq!(routine.variables(), &["wRandomSeed"]);
        assert_eq!(routine.returns(), Regs::A);
        assert_eq!(routine.clobbers(), Regs::F);

        let mut asm = Asm::new();
        asm.emit_all(routine.body().to_vec());
        let text = asm.to_asm();
        assert!(text.contains("RandomU8:"));
        assert!(text.contains("xor "));
        assert!(text.contains("ld [wRandomSeed], a"));
    }
}
