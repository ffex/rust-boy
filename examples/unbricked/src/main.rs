use retroshield_z80_workbench::prelude::*;

fn main() {
    let mut rom = CodeGen::new();
    /* TODO missed code:
     *     INCLUDE "hardware.inc"

    DEF BRICK_LEFT EQU $05
    DEF BRICK_RIGHT EQU $06
    DEF BLANK_TILE EQU $08
    DEF DIGIT_OFFSET EQU $1A
    DEF SCORE_TENS EQU $9870
    DEF SCORE_ONES EQU $9871

    SECTION "Header", ROM0[$100]
    */
    rom.jp("EntryPoint");
    //TODO ds $150 - @, 0 ; room for header
    rom.label("EntryPoint");
    rom.label("WaitVBlank");

    // ld a, [rLY]
    rom.ld_a_addr(0xFF44); // TODO (understand how to use constant in the libn) rLY = $FF44 in Game Boy hardware

    // cp 144
    rom.cp(144);

    // jp c, WaitVBlank
    rom.jp_c("WaitVBlank");

    // Turn off LCD
    // ld a, 0
    rom.ld_a(0);

    // ld [rLCDC], a
    rom.ld_addr_a(0xFF40); // TODO rLCDC = $FF40 in Game Boy hardware

    // Copy tiles data
    // ld de, Tiles
    rom.ld_de_label("Tiles");

    // ld hl, $9000
    rom.ld_hl(0x9000);

    // TODO ld bc, TilesEnd - Tiles
    // Note: This requires calculating the size at runtime or defining it as a constant
    //
    //TODO we have to deep in the concept. the library traslate in binary. we are not ready to this. better a middle step to generate the .asm
}
