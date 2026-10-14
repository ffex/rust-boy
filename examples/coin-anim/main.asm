    INCLUDE "hardware.inc"
    SECTION "VBlank interrupt", ROM0[$40]
    jp VBlankHandler
    SECTION "Header", ROM0[$100]
    jp EntryPoint
    ds $150 - @, 0

    EntryPoint:
    di
    ld sp, $FFFE
    call WaitVBlank
    ld a, 0
    ld [rLCDC], a
    ld a, 0
    ldh [rSCX], a
    ld a, 0
    ldh [rSCY], a
    ld de, Coin
    ld hl, $8000
    ld bc, CoinEnd - Coin
    call Memcopy
    call InitOamDma
    ld a, 0
    ld b, 160
    ld hl, wShadowOAM
    .clear_oam_2:
    ld [hli], a
    dec b
    jp nz, .clear_oam_2
    ld hl, wShadowOAM
    ld a, 88
    ld [hli], a
    ld a, 88
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, HIGH(wShadowOAM)
    call hOAMDMA
    ld a, 228
    ldh [rBGP], a
    ld a, 228
    ldh [rOBP0], a
    ld a, 228
    ldh [rOBP1], a
    ld a, 0
    ld [wCurKeys], a
    ld a, 0
    ld [wNewKeys], a
    ld a, 0
    ld [wReleasedKeys], a
    ld a, 0
    ld [wVBlankFlag], a
    ld a, 0
    ld [wFrameCounter], a
    ld a, 255
    ld [wAnim_Coin_Current], a
    ld a, IEF_VBLANK
    ldh [rIE], a
    ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | LCDCF_OBJ8
    ld [rLCDC], a
    xor a, a
    ldh [rIF], a
    ei

    Main:
    call WaitFrame
    ld a, [wFrameCounter]
    inc a
    ld [wFrameCounter], a
    cp a, 8
    jp c, .anim_end_3
    ld a, 0
    ld [wFrameCounter], a
    ld a, [wAnim_Coin_Current]
    cp a, 255
    jp z, .anim_Coin_end_4
    cp a, 0
    jr nz, .skip_Coin_CoinAnim_5
    call Anim_Coin_CoinAnim
    jp .anim_Coin_end_4
    .skip_Coin_CoinAnim_5:
    .anim_Coin_end_4:
    .anim_end_3:
    call UpdateKeys
    .check_a_0:
    ld a, [wCurKeys]
    and a, PADF_A
    jp z, .check_a_end_0
    ld a, 0
    ld [wAnim_Coin_Current], a
    .check_a_end_0:
    .check_b_1:
    ld a, [wCurKeys]
    and a, PADF_B
    jp z, .check_b_end_1
    ld a, 255
    ld [wAnim_Coin_Current], a
    .check_b_end_1:
    jp Main

    ; Copy bytes from one area to another
    ; @param de: source
    ; @param hl: destination
    ; @param bc: length (0 copies nothing)
    Memcopy:
    ld a, b
    or a, c
    ret z
    .copy:
    ld a, [de]
    ld [hli], a
    inc de
    dec bc
    ld a, b
    or a, c
    jp nz, .copy
    ret
    WaitVBlank:
    ld a, [rLY]
    cp a, 144
    jp c, WaitVBlank
    ret
    UpdateKeys:
    ld a, P1F_GET_BTN
    call .onenibble
    ld b, a
    ld a, P1F_GET_DPAD
    call .onenibble
    swap a
    xor a, b
    ld b, a
    ld a, P1F_GET_NONE
    ldh [rP1], a
    ld a, [wCurKeys]
    ld c, a
    xor a, b
    and a, b
    ld [wNewKeys], a
    ld a, b
    cpl
    and a, c
    ld [wReleasedKeys], a
    ld a, b
    ld [wCurKeys], a
    ret
    .onenibble:
    ldh [rP1], a
    call .knowret
    ldh a, [rP1]
    ldh a, [rP1]
    ldh a, [rP1]
    or a, 240
    .knowret:
    ret
    ; Wait for the next VBlank, asleep: the VBlank handler sets wVBlankFlag
    WaitFrame:
    ldh a, [rIE]
    or a, IEF_VBLANK
    ldh [rIE], a
    xor a, a
    ld [wVBlankFlag], a
    .wait:
    di
    ld a, [wVBlankFlag]
    and a, a
    jr nz, .done
    halt
    ei
    jr .wait
    .done:
    ei
    ret
    ; Copy the OAM DMA routine to HRAM: during a DMA the CPU reaches only HRAM
    InitOamDma:
    ld hl, OamDmaCode
    ld b, OamDmaCodeEnd - OamDmaCode
    ld c, LOW(hOAMDMA)
    .copy:
    ld a, [hli]
    ldh [c], a
    inc c
    dec b
    jr nz, .copy
    ret
    ; The OAM DMA routine (run from HRAM): a = the source page
    OamDmaCode:
    ldh [rDMA], a
    ld a, 40
    .wait:
    dec a
    jr nz, .wait
    ret
    OamDmaCodeEnd:
    Anim_Coin_CoinAnim:
    ld a, [wShadowOAM+2]
    cp a, 0
    jr c, .reset_CoinAnim
    cp a, 6
    jr c, .next_CoinAnim
    .reset_CoinAnim:
    ld a, 255
    .next_CoinAnim:
    inc a
    ld [wShadowOAM+2], a
    ret
    VBlankHandler:
    push af
    ld a, HIGH(wShadowOAM)
    call hOAMDMA
    ld a, 1
    ld [wVBlankFlag], a
    pop af
    reti

    Coin:
    dw $1818
    dw $7E66
    dw $467A
    dw $CFB1
    dw $DFA1
    dw $7E42
    dw $7E66
    dw $1818
    dw $1818
    dw $3C24
    dw $6E52
    dw $7E42
    dw $7E42
    dw $7E42
    dw $3C24
    dw $1818
    dw $1818
    dw $3C24
    dw $3C24
    dw $3C24
    dw $3C24
    dw $3C24
    dw $3C24
    dw $1818
    dw $1818
    dw $1818
    dw $1818
    dw $1818
    dw $1818
    dw $1818
    dw $1818
    dw $1818
    dw $1818
    dw $3C24
    dw $3C24
    dw $3C24
    dw $3C24
    dw $3C24
    dw $3C24
    dw $1818
    dw $1818
    dw $3C24
    dw $6E52
    dw $7E42
    dw $7E42
    dw $7E42
    dw $3C24
    dw $1818
    dw $1818
    dw $7E66
    dw $467A
    dw $CFB1
    dw $DFA1
    dw $7E42
    dw $7E66
    dw $1818
    CoinEnd:

    SECTION "Shadow OAM", WRAM0[$CF00]
    wShadowOAM:
    ds 160
    SECTION "OAM DMA", HRAM[$FFB8]
    hOAMDMA:
    ds 8
    SECTION "Variables", WRAM0[$C000]
    wCurKeys: db
    wNewKeys: db
    wReleasedKeys: db
    wVBlankFlag: db
    wFrameCounter: db
    wAnim_Coin_Current: db


