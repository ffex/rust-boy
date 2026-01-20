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
    ClearOam:
    ld [hli], a
    dec b
    jp nz, ClearOam
    ld hl, _OAMRAM
    ld a, 88
    ld [hli], a
    ld a, 88
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, 0
    ld [wFrameCounter], a
    ld a, 0
    ld [wCurKeys], a
    ld a, 0
    ld [wNewKeys], a
    ld a, 1
    ld [wAnim_CoinAnim_Active], a
    ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON
    ld [rLCDC], a
    ld a, 228
    ld [rBGP], a
    ld a, 228
    ld [rOBP0], a

    Main:
    call WaitNotVBlank
    call WaitVBlank
    ld a, [wFrameCounter]
    inc a
    ld [wFrameCounter], a
    cp 8
    jr c, AnimEnd
    ld a, 0
    ld [wFrameCounter], a
    ld a, [wAnim_CoinAnim_Active]
    cp 0
    jr z, .skip_CoinAnim
    call Anim_CoinAnim
    .skip_CoinAnim:
    AnimEnd:
    call UpdateKeys
    CheckA:
    ld a, [wCurKeys]
    and a, PADF_A
    jp z, CheckAEnd
    ld a, 1
    ld [wAnim_CoinAnim_Active], a
    CheckAEnd:
    CheckB:
    ld a, [wCurKeys]
    and a, PADF_B
    jp z, CheckBEnd
    ld a, 0
    ld [wAnim_CoinAnim_Active], a
    CheckBEnd:
    jp Main

    ; Copy bytes from one area to another
    ; @param de: source
    ; @param hl: destination
    ; @param bc: length
    Memcopy:
    ld a, [de]
    ld [hli], a
    inc de
    dec bc
    ld a, b
    or a, c
    jp nz, Memcopy
    ret
    WaitVBlank:
    ld a, [rLY]
    cp 144
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
    WaitNotVBlank:
    ld a, [rLY]
    cp 144
    jp nc, WaitNotVBlank
    ret
    Anim_CoinAnim:
    ld a, [_OAMRAM+2]
    inc a
    cp 7
    jr nz, updateSpriteIndex_CoinAnim
    ld a, 0
    updateSpriteIndex_CoinAnim:
    ld [_OAMRAM+2], a
    ret

    Coin:
    INCBIN "coin.2bpp"
    CoinEnd:

    SECTION "Variables", WRAM0
    wCurKeys: db
    wNewKeys: db
    wFrameCounter: db
    wAnim_CoinAnim_Active: db


