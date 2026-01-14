    INCLUDE "hardware.inc"
    DEF BRICK_LEFT EQU 5
    DEF BRICK_RIGHT EQU 6
    DEF BLANK_TILE EQU 8
    DEF DIGIT_OFFSET EQU 26
    DEF SCORE_TENS EQU 39024
    DEF SCORE_ONES EQU 39025
    SECTION "Header", ROM0[$100]
    jp EntryPoint
    ds $150 - @, 0
    EntryPoint:
    call WaitVBlank
    ld a, 0
    ld [rLCDC], a
    ld de, Tiles
    ld hl, $9000
    ld bc, TilesEnd - Tiles
    call Memcopy
    ld de, Ball
    ld hl, $8010
    ld bc, BallEnd - Ball
    call Memcopy
    ld de, Paddle
    ld hl, $8000
    ld bc, PaddleEnd - Paddle
    call Memcopy
    ld de, Tilemap
    ld hl, $9800
    ld bc, TilemapEnd - Tilemap
    call Memcopy
    ld a, 0
    ld b, 160
    ld hl, _OAMRAM
    ClearOam:
    ld [hli], a
    dec b
    jp nz, ClearOam
    ld a, 1
    ld [wBallMomentumX], a
    ld a, -1
    ld [wBallMomentumY], a
    ld hl, _OAMRAM
    ld a, 144
    ld [hli], a
    ld a, 24
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, 116
    ld [hli], a
    ld a, 40
    ld [hli], a
    ld a, 1
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON
    ld [rLCDC], a
    ld a, 228
    ld [rBGP], a
    ld a, 228
    ld [rOBP0], a
    ld a, 0
    ld [wFrameCounter], a
    ld [wNewKeys], a
    ld [wCurKeys], a
    ld [wScore], a
    Main:
    call WaitNotVBlank
    call WaitVBlank
    ld a, [wBallMomentumX]
    ld b, a
    ld a, [_OAMRAM+5]
    add a, b
    ld [_OAMRAM+5], a
    ld a, [wBallMomentumY]
    ld b, a
    ld a, [_OAMRAM+4]
    add a, b
    ld [_OAMRAM+4], a
    BounceOnTop:
    ld a, [_OAMRAM+4]
    sub a, 17
    ld c, a
    ld a, [_OAMRAM+5]
    sub a, 8
    ld b, a
    call GetTileByPixel
    ld a, [hl]
    call IsWallTile
    jp nz, BounceOnTopEnd
    ld a, 1
    ld [wBallMomentumY], a
    BounceOnTopEnd:
    BounceOnRight:
    ld a, [_OAMRAM+4]
    sub a, 16
    ld c, a
    ld a, [_OAMRAM+5]
    sub a, 7
    ld b, a
    call GetTileByPixel
    ld a, [hl]
    call IsWallTile
    jp nz, BounceOnRightEnd
    ld a, -1
    ld [wBallMomentumX], a
    BounceOnRightEnd:
    BounceOnLeft:
    ld a, [_OAMRAM+4]
    sub a, 16
    ld c, a
    ld a, [_OAMRAM+5]
    sub a, 9
    ld b, a
    call GetTileByPixel
    ld a, [hl]
    call IsWallTile
    jp nz, BounceOnLeftEnd
    ld a, 1
    ld [wBallMomentumX], a
    BounceOnLeftEnd:
    BounceOnBottom:
    ld a, [_OAMRAM+4]
    sub a, 15
    ld c, a
    ld a, [_OAMRAM+5]
    sub a, 8
    ld b, a
    call GetTileByPixel
    ld a, [hl]
    call IsWallTile
    jp nz, BounceOnBottomEnd
    ld a, -1
    ld [wBallMomentumY], a
    BounceOnBottomEnd:
    call UpdateKeys
    CheckLeft:
    ld a, [wCurKeys]
    and a, PADF_LEFT
    jp z, CheckLeftEnd
    LeftLimit:
    ld a, [_OAMRAM+1]
    sub a, 1
    cp 15
    jp z, LeftLimitEnd
    ld [_OAMRAM+1], a
    LeftLimitEnd:
    CheckLeftEnd:
    CheckRight:
    ld a, [wCurKeys]
    and a, PADF_RIGHT
    jp z, CheckRightEnd
    RightLimit:
    ld a, [_OAMRAM+1]
    add a, 1
    cp 105
    jp z, RightLimitEnd
    ld [_OAMRAM+1], a
    RightLimitEnd:
    CheckRightEnd:
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
    WaitVBlank:
    ld a, [rLY]
    cp 144
    jp c, WaitVBlank
    ret
    WaitNotVBlank:
    ld a, [rLY]
    cp 144
    jp nc, WaitNotVBlank
    ret
    ; Convert a pixel position to a tilemap address
    ; hl = $9800 + X + Y * 32
    ; @param b: X
    ; @param c: Y
    ; @return hl: tile address
    GetTileByPixel:
    ld a, c
    and a, 248
    ld l, a
    ld h, 0
    add hl, hl
    add hl, hl
    ld a, b
    srl a
    srl a
    srl a
    add a, l
    ld l, a
    adc a, h
    sub a, l
    ld h, a
    ld bc, $9800
    add hl, bc
    ret
    IsWallTile:
    cp $00
    ret z
    cp $01
    ret z
    cp $02
    ret z
    cp $04
    ret z
    cp $05
    ret z
    cp $06
    ret z
    cp $07
    ret

    Tiles:
    dw `33333333
    dw `33333333
    dw `33333333
    dw `33322222
    dw `33322222
    dw `33322222
    dw `33322211
    dw `33322211
    dw `33333333
    dw `33333333
    dw `33333333
    dw `22222222
    dw `22222222
    dw `22222222
    dw `11111111
    dw `11111111
    dw `33333333
    dw `33333333
    dw `33333333
    dw `22222333
    dw `22222333
    dw `22222333
    dw `11222333
    dw `11222333
    dw `33333333
    dw `33333333
    dw `33333333
    dw `33333333
    dw `33333333
    dw `33333333
    dw `33333333
    dw `33333333
    dw `33322211
    dw `33322211
    dw `33322211
    dw `33322211
    dw `33322211
    dw `33322211
    dw `33322211
    dw `33322211
    dw `22222222
    dw `20000000
    dw `20111111
    dw `20111111
    dw `20111111
    dw `20111111
    dw `22222222
    dw `33333333
    dw `22222223
    dw `00000023
    dw `11111123
    dw `11111123
    dw `11111123
    dw `11111123
    dw `22222223
    dw `33333333
    dw `11222333
    dw `11222333
    dw `11222333
    dw `11222333
    dw `11222333
    dw `11222333
    dw `11222333
    dw `11222333
    dw `00000000
    dw `00000000
    dw `00000000
    dw `00000000
    dw `00000000
    dw `00000000
    dw `00000000
    dw `00000000
    dw `11001100
    dw `11111111
    dw `11111111
    dw `21212121
    dw `22222222
    dw `22322232
    dw `23232323
    dw `33333333
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222211
    dw `22222211
    dw `22222211
    dw `22222222
    dw `22222222
    dw `22222222
    dw `11111111
    dw `11111111
    dw `11221111
    dw `11221111
    dw `11000011
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `11222222
    dw `11222222
    dw `11222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222211
    dw `22222200
    dw `22222200
    dw `22000000
    dw `22000000
    dw `22222222
    dw `22222222
    dw `22222222
    dw `11000011
    dw `11111111
    dw `11111111
    dw `11111111
    dw `11111111
    dw `11111111
    dw `11111111
    dw `11000022
    dw `11222222
    dw `11222222
    dw `11222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222200
    dw `22222200
    dw `22222211
    dw `22222211
    dw `22221111
    dw `22221111
    dw `22221111
    dw `11000022
    dw `00112222
    dw `00112222
    dw `11112200
    dw `11112200
    dw `11220000
    dw `11220000
    dw `11220000
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22000000
    dw `22000000
    dw `00000000
    dw `00000000
    dw `00000000
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `22222222
    dw `11110022
    dw `11110022
    dw `11110022
    dw `22221111
    dw `22221111
    dw `22221111
    dw `22221111
    dw `22221111
    dw `22222211
    dw `22222211
    dw `22222222
    dw `11220000
    dw `11110000
    dw `11110000
    dw `11111111
    dw `11111111
    dw `11111111
    dw `11111111
    dw `22222222
    dw `00000000
    dw `00111111
    dw `00111111
    dw `11111111
    dw `11111111
    dw `11111111
    dw `11111111
    dw `22222222
    dw `11110022
    dw `11000022
    dw `11000022
    dw `00002222
    dw `00002222
    dw `00222222
    dw `00222222
    dw `22222222
    dw `33333333
    dw `33000033
    dw `30033003
    dw `30033003
    dw `30033003
    dw `30033003
    dw `33000033
    dw `33333333
    dw `33333333
    dw `33300333
    dw `33000333
    dw `33300333
    dw `33300333
    dw `33300333
    dw `33000033
    dw `33333333
    dw `33333333
    dw `33000033
    dw `30330003
    dw `33330003
    dw `33000333
    dw `30003333
    dw `30000003
    dw `33333333
    dw `33333333
    dw `30000033
    dw `33330003
    dw `33000033
    dw `33330003
    dw `33330003
    dw `30000033
    dw `33333333
    dw `33333333
    dw `33000033
    dw `30030033
    dw `30330033
    dw `30330033
    dw `30000003
    dw `33330033
    dw `33333333
    dw `33333333
    dw `30000033
    dw `30033333
    dw `30000033
    dw `33330003
    dw `30330003
    dw `33000033
    dw `33333333
    dw `33333333
    dw `33000033
    dw `30033333
    dw `30000033
    dw `30033003
    dw `30033003
    dw `33000033
    dw `33333333
    dw `33333333
    dw `30000003
    dw `33333003
    dw `33330033
    dw `33300333
    dw `33000333
    dw `33000333
    dw `33333333
    dw `33333333
    dw `33000033
    dw `30333003
    dw `33000033
    dw `30333003
    dw `30333003
    dw `33000033
    dw `33333333
    dw `33333333
    dw `33000033
    dw `30330003
    dw `30330003
    dw `33000003
    dw `33330003
    dw `33000033
    dw `33333333
    TilesEnd:
    Ball:
    dw `00033000
    dw `00322300
    dw `03222230
    dw `03222230
    dw `00322300
    dw `00033000
    dw `00000000
    dw `00000000
    BallEnd:
    Paddle:
    dw `13333331
    dw `30000003
    dw `13333331
    dw `00000000
    dw `00000000
    dw `00000000
    dw `00000000
    dw `00000000
    PaddleEnd:

    Tilemap:
    db $00, $01, $01, $01, $01, $01, $01, $01, $01, $01, $01, $01, $01, $02, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $08, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $08, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $08, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $0A, $0B, $0C, $0D, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $0E, $0F, $10, $11, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $12, $13, $14, $15, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $16, $17, $18, $19, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $04, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $07, $03, $03, $03, $03, $03, $03, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    TilemapEnd:

    SECTION "Counter", WRAM0
    wFrameCounter: db
    SECTION "Input Variables", WRAM0
    wCurKeys: db
    wNewKeys: db
    SECTION "Ball Data", WRAM0
    wBallMomentumX: db
    wBallMomentumY: db
    SECTION "Score", WRAM0
    wScore: db


