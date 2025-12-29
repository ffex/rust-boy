use rust_boy::gb_asm::{Asm, Chunk, Condition};

fn main() {
    let mut asm = Asm::new();

    // Add some directives
    asm.include_hardware()
        .section("Header", "ROM0[$100]")
        .raw("nop")
        .raw("jp EntryPoint");

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

    // Generate and print the assembly
    println!("{}", asm.to_asm());
}
