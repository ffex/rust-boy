    INCLUDE "hardware.inc"
    SECTION "Header", ROM0[$100]
    jp EntryPoint
    ds $150 - @, 0

    EntryPoint:
    call WaitVBlank
    ld a, 0
    ld [rLCDC], a
    ld de, Coin
    ld hl, $8000
    ld bc, CoinEnd - Coin
    call Memcopy
    ld a, 0
    ld b, 160
    ld hl, _OAMRAM
    .clear_oam_2:
    ld [hli], a
    dec b
    jp nz, .clear_oam_2
    ld hl, _OAMRAM
    ld a, 88
    ld [hli], a
    ld a, 88
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, 228
    ld [rBGP], a
    ld [rOBP0], a
    ld [rOBP1], a
    ld a, 0
    ld [wCurKeys], a
    ld a, 0
    ld [wNewKeys], a
    ld a, 0
    ld [wFrameCounter], a
    ld a, 255
    ld [wAnim_Coin_Current], a
    ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | LCDCF_OBJ8
    ld [rLCDC], a

    Main:
    call WaitNotVBlank
    call WaitVBlank
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
    WaitNotVBlank:
    ld a, [rLY]
    cp a, 144
    jp nc, WaitNotVBlank
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
    xor a, b
    and a, b
    ld [wNewKeys], a
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
    Anim_Coin_CoinAnim:
    ld a, [_OAMRAM+2]
    cp a, 0
    jr c, .reset_CoinAnim
    cp a, 6
    jr c, .next_CoinAnim
    .reset_CoinAnim:
    ld a, 255
    .next_CoinAnim:
    inc a
    ld [_OAMRAM+2], a
    ret

    Coin:
    INCBIN "coin.2bpp"
    CoinEnd:

    SECTION "Variables", WRAM0[$C000]
    wCurKeys: db
    wNewKeys: db
    wFrameCounter: db
    wAnim_Coin_Current: db


