    INCLUDE "hardware.inc"
    SECTION "Header", ROM0[$100]
    nop
    jp EntryPoint
    ds $150 - @, 0
    SECTION "Main", ROM0
    EntryPoint:
    ; Initialize display
    ld a, 145
    ldh [$FF40], a
    MainLoop:
    ld bc, 160
    call WaitVBlank
    jp MainLoop

    WaitVBlank:
    ; Wait for vertical blank
    ld a, [$FF44]
    cp a, 144
    jr nz, WaitVBlank
    ret

    TileData:
    db $FF, $00, $7E, $FF, $85, $81, $89, $83


