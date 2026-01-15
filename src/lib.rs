// Game Boy Assembly Generator Library
pub mod gb_asm;
pub mod gb_std;
pub mod rust_boy;

#[cfg(test)]
mod tests {
    use super::gb_asm::{Asm, Chunk, Condition};

    #[test]
    fn test_basic_assembly_generation() {
        let mut asm = Asm::new();

        asm.include_hardware()
            .section("Main", "ROM0")
            .label("Start")
            .ld_a(0x42)
            .ret();

        let output = asm.to_asm();

        assert!(output.contains("INCLUDE \"hardware.inc\""));
        assert!(output.contains("SECTION \"Main\", ROM0"));
        assert!(output.contains("Start:"));
        assert!(output.contains("ld a, 66"));
        assert!(output.contains("ret"));
    }

    #[test]
    fn test_conditional_jumps() {
        let mut asm = Asm::new();

        asm.label("Loop")
            .cp_imm(0)
            .jr_cond(Condition::Z, "End")
            .jp("Loop")
            .label("End");

        let output = asm.to_asm();

        assert!(output.contains("jr z, End"));
        assert!(output.contains("jp Loop"));
    }

    #[test]
    fn test_chunks() {
        let mut asm = Asm::new();

        asm.chunk(Chunk::Main).label("Main").call("Function");

        asm.chunk(Chunk::Functions).label("Function").ret();

        let output = asm.to_asm();

        assert!(output.contains("Main:"));
        assert!(output.contains("Function:"));
        assert!(output.contains("call Function"));
    }
}
