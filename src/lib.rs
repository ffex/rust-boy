#![doc = include_str!("../README.md")]

pub mod gb_asm;
pub mod gb_std;
pub mod hw;
pub mod rust_boy;

#[cfg(test)]
mod tests {
    use super::gb_asm::{Asm, Condition, Section};

    #[test]
    fn test_basic_assembly_generation() {
        let mut asm = Asm::new();

        asm.include_hardware()
            .section(Section::rom0("Main"))
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
    fn test_code_is_printed_in_order() {
        let mut asm = Asm::new();

        asm.label("Main").call("Function").blank_line();

        asm.label("Function").ret();

        let output = asm.to_asm();

        assert_eq!(
            output,
            "    Main:\n    call Function\n\n    Function:\n    ret\n\n"
        );
    }
}
