use rust_boy::gb_asm::{Asm, Chunk, Condition};

fn program() -> Asm {
    let mut asm = Asm::new();

    // Cartridge header at $100-$14F: rgbfix fills in the logo and checksums
    asm.include_hardware()
        .section("Header", "ROM0[$100]")
        .raw("nop")
        .raw("jp EntryPoint")
        .ds("$150 - @", "0");

    // Main code section
    asm.section("Main", "ROM0")
        .label("EntryPoint")
        .comment("Initialize display")
        .ld_a(0x91)
        .ldh_label("[$FF40]", "a");

    // Add a loop
    asm.label("MainLoop")
        .ld_bc(160)
        .call("WaitVBlank")
        .jp("MainLoop");

    // Functions chunk
    asm.chunk(Chunk::Functions)
        .label("WaitVBlank")
        .comment("Wait for vertical blank")
        .ld_a_label("[$FF44]")
        .cp_imm(144)
        .jr_cond(Condition::NZ, "WaitVBlank")
        .ret();

    // Data chunk
    asm.chunk(Chunk::Data)
        .label("TileData")
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
