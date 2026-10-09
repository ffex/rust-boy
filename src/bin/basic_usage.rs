use rust_boy::gb_asm::{Asm, Condition, Expr, Mem, R8, Section};

fn program() -> Asm {
    let mut asm = Asm::new();

    // Cartridge header at $100-$14F: rgbfix fills in the logo and checksums
    asm.include_hardware()
        .section(Section::rom0("Header").at(0x0100))
        .raw("nop")
        .raw("jp EntryPoint")
        .ds_fill("$150 - @", "0");

    // Main code section
    asm.section(Section::rom0("Main"))
        .label("EntryPoint")
        .comment("Initialize display")
        .ld_a(0x91)
        .ldh(Mem::addr(Expr::hex(0xFF40)), R8::A);

    // Add a loop
    asm.label("MainLoop")
        .ld_bc(160)
        .call("WaitVBlank")
        .jp("MainLoop")
        .blank_line();

    // Functions, still in the Main section
    asm.label("WaitVBlank")
        .comment("Wait for vertical blank")
        .ld(R8::A, Mem::addr(Expr::hex(0xFF44)))
        .cp_imm(144)
        .jr_cond(Condition::NZ, "WaitVBlank")
        .ret()
        .blank_line();

    // Data, in ROM too
    asm.label("TileData")
        .db("$FF, $00, $7E, $FF, $85, $81, $89, $83");

    asm
}

fn main() {
    // Generate and print the assembly
    println!("{}", program().to_asm());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_reserves_the_cartridge_header() {
        let out = program().to_asm();
        let lines: Vec<&str> = out.lines().map(str::trim).collect();
        let jp = lines.iter().position(|l| *l == "jp EntryPoint").unwrap();
        // The header must be padded to $150 before any other section starts (B21)
        assert_eq!(lines[jp + 1], "ds $150 - @, 0");
        assert!(lines[jp + 2].starts_with("SECTION \"Main\""));
    }
}
