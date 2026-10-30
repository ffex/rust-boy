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
    ld de, player_left
    ld hl, $8000
    ld bc, player_leftEnd - player_left
    call Memcopy
    ld de, player_right
    ld hl, $8400
    ld bc, player_rightEnd - player_right
    call Memcopy
    ld de, BgTile
    ld hl, $9000
    ld bc, BgTileEnd - BgTile
    call Memcopy
    ld de, OliveTree
    ld hl, $9010
    ld bc, OliveTreeEnd - OliveTree
    call Memcopy
    ld de, OliveTreeFull
    ld hl, $9050
    ld bc, OliveTreeFullEnd - OliveTreeFull
    call Memcopy
    ld de, BgMap
    ld hl, $9800
    ld bc, BgMapEnd - BgMap
    call Memcopy
    ld de, HudBlank
    ld hl, $9090
    ld bc, HudBlankEnd - HudBlank
    call Memcopy
    ld de, HudOlive
    ld hl, $90A0
    ld bc, HudOliveEnd - HudOlive
    call Memcopy
    ld de, Digits
    ld hl, $90E0
    ld bc, DigitsEnd - Digits
    call Memcopy
    ld de, Hud
    ld hl, $9C00
    ld bc, HudEnd - Hud
    call Memcopy
    call InitOamDma
    ld a, 0
    ld b, 160
    ld hl, wShadowOAM
    .clear_oam_23:
    ld [hli], a
    dec b
    jp nz, .clear_oam_23
    ld hl, wShadowOAM
    ld a, 88
    ld [hli], a
    ld a, 88
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, 88
    ld [hli], a
    ld a, 96
    ld [hli], a
    ld a, 64
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
    ld hl, wTextOlivesGlyphs
    ld bc, 5
    .array_init_35:
    ld a, 14
    ld [hli], a
    dec bc
    ld a, b
    or a, c
    jr nz, .array_init_35
    ld a, 0
    ld [wTextOlivesCursor], a
    ld a, 0
    ld [wTextOlivesCursor+1], a
    ld a, 0
    ld [wOlivesHarvested], a
    ld a, 0
    ld [wOlivesHarvested+1], a
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
    ld [wAnim_player_left_Current], a
    ld a, 255
    ld [wAnim_player_right_Current], a
    xor a, a
    ld [wVramQueueRead], a
    ld [wVramQueueWrite], a
    ld a, IEF_VBLANK
    ldh [rIE], a
    ld a, 7
    ldh [rWX], a
    ld a, 128
    ldh [rWY], a
    ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | LCDCF_OBJ16 | LCDCF_WINON | LCDCF_WIN9C00
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
    jp c, .anim_end_24
    ld a, 0
    ld [wFrameCounter], a
    ld a, [wAnim_player_left_Current]
    cp a, 255
    jp z, .anim_player_left_end_25
    cp a, 0
    jr nz, .skip_player_left_playerWalkFront_26
    call Anim_player_left_playerWalkFront
    jp .anim_player_left_end_25
    .skip_player_left_playerWalkFront_26:
    cp a, 1
    jr nz, .skip_player_left_playerWalkBack_27
    call Anim_player_left_playerWalkBack
    jp .anim_player_left_end_25
    .skip_player_left_playerWalkBack_27:
    cp a, 2
    jr nz, .skip_player_left_playerWalkLeft_28
    call Anim_player_left_playerWalkLeft
    jp .anim_player_left_end_25
    .skip_player_left_playerWalkLeft_28:
    cp a, 3
    jr nz, .skip_player_left_playerWalkRight_29
    call Anim_player_left_playerWalkRight
    jp .anim_player_left_end_25
    .skip_player_left_playerWalkRight_29:
    .anim_player_left_end_25:
    ld a, [wAnim_player_right_Current]
    cp a, 255
    jp z, .anim_player_right_end_30
    cp a, 0
    jr nz, .skip_player_right_playerWalkFront_31
    call Anim_player_right_playerWalkFront
    jp .anim_player_right_end_30
    .skip_player_right_playerWalkFront_31:
    cp a, 1
    jr nz, .skip_player_right_playerWalkBack_32
    call Anim_player_right_playerWalkBack
    jp .anim_player_right_end_30
    .skip_player_right_playerWalkBack_32:
    cp a, 2
    jr nz, .skip_player_right_playerWalkLeft_33
    call Anim_player_right_playerWalkLeft
    jp .anim_player_right_end_30
    .skip_player_right_playerWalkLeft_33:
    cp a, 3
    jr nz, .skip_player_right_playerWalkRight_34
    call Anim_player_right_playerWalkRight
    jp .anim_player_right_end_30
    .skip_player_right_playerWalkRight_34:
    .anim_player_right_end_30:
    .anim_end_24:
    call PrintTextOlives
    call UpdateKeys
    .check_left_14:
    ld a, [wCurKeys]
    and a, PADF_LEFT
    jp z, .check_left_end_14
    ld a, [wShadowOAM+1]
    add a, 249
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 248
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_blocked_1
    cp a, 8
    jp z, .solid_tile_blocked_1
    cp a, 3
    jp z, .solid_tile_blocked_1
    cp a, 4
    jp z, .solid_tile_blocked_1
    ld a, [wShadowOAM+1]
    add a, 249
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 255
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_blocked_1
    cp a, 8
    jp z, .solid_tile_blocked_1
    cp a, 3
    jp z, .solid_tile_blocked_1
    cp a, 4
    jp z, .solid_tile_blocked_1
    ld a, [wShadowOAM+1]
    cp a, 40
    jp nz, .else_2
    ldh a, [rSCX]
    add a, 255
    ldh [rSCX], a
    jp .end_if_2
    .else_2:
    ld a, [wShadowOAM+1]
    sub a, 40
    jp c, .sprite0_left_limit_end_0
    sub a, 1
    jp nc, .sprite0_left_limit_store_0
    ld a, 0
    .sprite0_left_limit_store_0:
    add a, 40
    ld [wShadowOAM+1], a
    add a, 8
    ld [wShadowOAM+5], a
    .sprite0_left_limit_end_0:
    .end_if_2:
    .solid_tile_blocked_1:
    ld a, 2
    ld [wAnim_player_left_Current], a
    ld a, 2
    ld [wAnim_player_right_Current], a
    .check_left_end_14:
    .check_right_15:
    ld a, [wCurKeys]
    and a, PADF_RIGHT
    jp z, .check_right_end_15
    ld a, [wShadowOAM+1]
    add a, 6
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 248
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_blocked_4
    cp a, 8
    jp z, .solid_tile_blocked_4
    cp a, 3
    jp z, .solid_tile_blocked_4
    cp a, 4
    jp z, .solid_tile_blocked_4
    ld a, [wShadowOAM+1]
    add a, 6
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 255
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_blocked_4
    cp a, 8
    jp z, .solid_tile_blocked_4
    cp a, 3
    jp z, .solid_tile_blocked_4
    cp a, 4
    jp z, .solid_tile_blocked_4
    ld a, [wShadowOAM+5]
    cp a, 128
    jp nz, .else_5
    ldh a, [rSCX]
    add a, 1
    ldh [rSCX], a
    jp .end_if_5
    .else_5:
    ld a, [wShadowOAM+5]
    sub a, 128
    jp nc, .sprite1_right_limit_end_3
    add a, 1
    jp nc, .sprite1_right_limit_store_3
    ld a, 0
    .sprite1_right_limit_store_3:
    add a, 128
    ld [wShadowOAM+5], a
    sub a, 8
    ld [wShadowOAM+1], a
    .sprite1_right_limit_end_3:
    .end_if_5:
    .solid_tile_blocked_4:
    ld a, 3
    ld [wAnim_player_left_Current], a
    ld a, 3
    ld [wAnim_player_right_Current], a
    .check_right_end_15:
    .check_up_16:
    ld a, [wCurKeys]
    and a, PADF_UP
    jp z, .check_up_end_16
    ld a, [wShadowOAM+1]
    add a, 250
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 247
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_blocked_7
    cp a, 8
    jp z, .solid_tile_blocked_7
    cp a, 3
    jp z, .solid_tile_blocked_7
    cp a, 4
    jp z, .solid_tile_blocked_7
    ld a, [wShadowOAM+1]
    add a, 2
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 247
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_blocked_7
    cp a, 8
    jp z, .solid_tile_blocked_7
    cp a, 3
    jp z, .solid_tile_blocked_7
    cp a, 4
    jp z, .solid_tile_blocked_7
    ld a, [wShadowOAM+1]
    add a, 5
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 247
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_blocked_7
    cp a, 8
    jp z, .solid_tile_blocked_7
    cp a, 3
    jp z, .solid_tile_blocked_7
    cp a, 4
    jp z, .solid_tile_blocked_7
    ld a, [wShadowOAM+0]
    cp a, 48
    jp nz, .else_8
    ldh a, [rSCY]
    add a, 255
    ldh [rSCY], a
    jp .end_if_8
    .else_8:
    ld a, [wShadowOAM+0]
    sub a, 48
    jp c, .sprite0_up_limit_end_6
    sub a, 1
    jp nc, .sprite0_up_limit_store_6
    ld a, 0
    .sprite0_up_limit_store_6:
    add a, 48
    ld [wShadowOAM+0], a
    ld [wShadowOAM+4], a
    .sprite0_up_limit_end_6:
    .end_if_8:
    .solid_tile_blocked_7:
    ld a, 1
    ld [wAnim_player_left_Current], a
    ld a, 1
    ld [wAnim_player_right_Current], a
    .check_up_end_16:
    .check_down_17:
    ld a, [wCurKeys]
    and a, PADF_DOWN
    jp z, .check_down_end_17
    ld a, [wShadowOAM+1]
    add a, 250
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_blocked_10
    cp a, 8
    jp z, .solid_tile_blocked_10
    cp a, 3
    jp z, .solid_tile_blocked_10
    cp a, 4
    jp z, .solid_tile_blocked_10
    ld a, [wShadowOAM+1]
    add a, 2
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_blocked_10
    cp a, 8
    jp z, .solid_tile_blocked_10
    cp a, 3
    jp z, .solid_tile_blocked_10
    cp a, 4
    jp z, .solid_tile_blocked_10
    ld a, [wShadowOAM+1]
    add a, 5
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_blocked_10
    cp a, 8
    jp z, .solid_tile_blocked_10
    cp a, 3
    jp z, .solid_tile_blocked_10
    cp a, 4
    jp z, .solid_tile_blocked_10
    ld a, [wShadowOAM+0]
    cp a, 96
    jp nz, .else_11
    ldh a, [rSCY]
    add a, 1
    ldh [rSCY], a
    jp .end_if_11
    .else_11:
    ld a, [wShadowOAM+0]
    sub a, 96
    jp nc, .sprite0_down_limit_end_9
    add a, 1
    jp nc, .sprite0_down_limit_store_9
    ld a, 0
    .sprite0_down_limit_store_9:
    add a, 96
    ld [wShadowOAM+0], a
    ld [wShadowOAM+4], a
    .sprite0_down_limit_end_9:
    .end_if_11:
    .solid_tile_blocked_10:
    ld a, 0
    ld [wAnim_player_left_Current], a
    ld a, 0
    ld [wAnim_player_right_Current], a
    .check_down_end_17:
    .check_left_18:
    ld a, [wReleasedKeys]
    and a, PADF_LEFT
    jp z, .check_left_end_18
    ld a, 2
    ld [wAnim_player_left_Current], a
    xor a, a
    ld a, 16
    ld [wShadowOAM+2], a
    ld a, 2
    ld [wAnim_player_right_Current], a
    xor a, a
    ld a, 80
    ld [wShadowOAM+6], a
    ld a, 255
    ld [wAnim_player_left_Current], a
    ld a, 255
    ld [wAnim_player_right_Current], a
    .check_left_end_18:
    .check_right_19:
    ld a, [wReleasedKeys]
    and a, PADF_RIGHT
    jp z, .check_right_end_19
    ld a, 3
    ld [wAnim_player_left_Current], a
    xor a, a
    ld a, 24
    ld [wShadowOAM+2], a
    ld a, 3
    ld [wAnim_player_right_Current], a
    xor a, a
    ld a, 88
    ld [wShadowOAM+6], a
    ld a, 255
    ld [wAnim_player_left_Current], a
    ld a, 255
    ld [wAnim_player_right_Current], a
    .check_right_end_19:
    .check_up_20:
    ld a, [wReleasedKeys]
    and a, PADF_UP
    jp z, .check_up_end_20
    ld a, 1
    ld [wAnim_player_left_Current], a
    xor a, a
    ld a, 8
    ld [wShadowOAM+2], a
    ld a, 1
    ld [wAnim_player_right_Current], a
    xor a, a
    ld a, 72
    ld [wShadowOAM+6], a
    ld a, 255
    ld [wAnim_player_left_Current], a
    ld a, 255
    ld [wAnim_player_right_Current], a
    .check_up_end_20:
    .check_down_21:
    ld a, [wReleasedKeys]
    and a, PADF_DOWN
    jp z, .check_down_end_21
    ld a, 0
    ld [wAnim_player_left_Current], a
    xor a, a
    ld a, 0
    ld [wShadowOAM+2], a
    ld a, 0
    ld [wAnim_player_right_Current], a
    xor a, a
    ld a, 64
    ld [wShadowOAM+6], a
    ld a, 255
    ld [wAnim_player_left_Current], a
    ld a, 255
    ld [wAnim_player_right_Current], a
    .check_down_end_21:
    .check_a_22:
    ld a, [wNewKeys]
    and a, PADF_A
    jp z, .check_a_end_22
    ld a, [wShadowOAM+1]
    add a, 249
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 248
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_hit_13
    cp a, 8
    jp z, .solid_tile_hit_13
    ld a, [wShadowOAM+1]
    add a, 249
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 255
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_hit_13
    cp a, 8
    jp z, .solid_tile_hit_13
    ld a, [wShadowOAM+1]
    add a, 6
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 248
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_hit_13
    cp a, 8
    jp z, .solid_tile_hit_13
    ld a, [wShadowOAM+1]
    add a, 6
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 255
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_hit_13
    cp a, 8
    jp z, .solid_tile_hit_13
    ld a, [wShadowOAM+1]
    add a, 250
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 247
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_hit_13
    cp a, 8
    jp z, .solid_tile_hit_13
    ld a, [wShadowOAM+1]
    add a, 2
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 247
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_hit_13
    cp a, 8
    jp z, .solid_tile_hit_13
    ld a, [wShadowOAM+1]
    add a, 5
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    add a, 247
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_hit_13
    cp a, 8
    jp z, .solid_tile_hit_13
    ld a, [wShadowOAM+1]
    add a, 250
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_hit_13
    cp a, 8
    jp z, .solid_tile_hit_13
    ld a, [wShadowOAM+1]
    add a, 2
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_hit_13
    cp a, 8
    jp z, .solid_tile_hit_13
    ld a, [wShadowOAM+1]
    add a, 5
    ld b, a
    push bc
    ld a, [wShadowOAM+0]
    ld d, a
    pop bc
    ld c, d
    call GetTileByScreenPixel
    cp a, 7
    jp z, .solid_tile_hit_13
    cp a, 8
    jp z, .solid_tile_hit_13
    jp .solid_tile_end_13
    .solid_tile_hit_13:
    res 0, l
    ld bc, -32
    add hl, bc
    push hl
    ld a, 1
    call QueueVramWrite
    pop hl
    inc hl
    push hl
    ld a, 2
    call QueueVramWrite
    pop hl
    ld bc, 31
    add hl, bc
    push hl
    ld a, 3
    call QueueVramWrite
    pop hl
    inc hl
    push hl
    ld a, 4
    call QueueVramWrite
    pop hl
    ld a, [wOlivesHarvested]
    ld l, a
    ld a, [wOlivesHarvested+1]
    ld h, a
    inc hl
    ld a, l
    ld [wOlivesHarvested], a
    ld a, h
    ld [wOlivesHarvested+1], a
    ld d, h
    ld e, l
    ld hl, wTextOlivesGlyphs
    ld c, 14
    call EncodeDecimalU16
    jr c, .number_12
    ld a, 0
    ld [wTextOlivesCursor], a
    ld a, 0
    ld [wTextOlivesCursor+1], a
    and a, a
    .number_12:
    .solid_tile_end_13:
    .check_a_end_22:
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
    ; Convert a pixel position to a tilemap address and read the tile there
    ; hl = active background map + X / 8 + (Y / 8) * 32
    ; @param b: X
    ; @param c: Y
    ; @return hl: tile address
    ; @return a: tile index at that address
    ; changes bc
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
    ld a, [rLCDC]
    and a, LCDCF_BG9C00
    jr z, .map_selected
    ld bc, $9C00
    .map_selected:
    add hl, bc
    ld a, [hl]
    ret
    ; Add the camera scroll to screen coordinates, then look up that map pixel
    GetTileByScreenPixel:
    ldh a, [rSCX]
    add a, b
    ld b, a
    ldh a, [rSCY]
    add a, c
    ld c, a
    call GetTileByPixel
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
    QueueVramWrite:
    ld d, a
    ld a, h
    cp a, 128
    ret c
    cp a, 160
    jr nc, .reject
    ld b, h
    ld c, l
    ld a, [wVramQueueWrite]
    ld e, a
    add a, 3
    cp a, 39
    jr c, .next
    xor a, a
    .next:
    ld h, a
    ld a, [wVramQueueRead]
    cp a, h
    jr z, .reject
    ld l, e
    ld e, h
    ld h, 0
    push bc
    ld bc, wVramQueue
    add hl, bc
    pop bc
    ld a, c
    ld [hli], a
    ld a, b
    ld [hli], a
    ld [hl], d
    .publish:
    ld a, e
    ld [wVramQueueWrite], a
    and a, a
    ret
    .reject:
    scf
    ret
    FlushVramQueue:
    .next:
    ldh a, [rLY]
    cp a, 144
    ret c
    cp a, 153
    ret nc
    ld a, [wVramQueueRead]
    ld e, a
    ld a, [wVramQueueWrite]
    cp a, e
    ret z
    ld l, e
    ld h, 0
    ld bc, wVramQueue
    add hl, bc
    ld a, [hli]
    ld e, a
    ld a, [hli]
    ld d, a
    ld a, [hl]
    .write:
    ld [de], a
    ld a, [wVramQueueRead]
    add a, 3
    cp a, 39
    jr c, .advance
    xor a, a
    .advance:
    ld [wVramQueueRead], a
    jr .next
    PrintTextOlives:
    .next:
    ld a, [wTextOlivesCursor]
    ld e, a
    ld a, [wTextOlivesCursor+1]
    ld d, a
    cp a, 0
    jr c, .write
    jr nz, .done
    ld a, e
    cp a, 5
    jr nc, .done
    .write:
    push de
    ld hl, TextOlivesTargets
    add hl, de
    add hl, de
    ld a, [hli]
    ld c, a
    ld b, [hl]
    push bc
    ld hl, wTextOlivesGlyphs
    add hl, de
    ld a, [hl]
    pop hl
    call QueueVramWrite
    pop de
    ret c
    inc de
    ld a, e
    ld [wTextOlivesCursor], a
    ld a, d
    ld [wTextOlivesCursor+1], a
    jr .next
    .done:
    xor a, a
    ret
    EncodeDecimalU16:
    ld b, 0
    .digit10000:
    ld a, e
    sub a, 16
    ld e, a
    ld a, d
    sbc a, 39
    jr c, .end10000
    ld d, a
    inc b
    jr .digit10000
    .end10000:
    ld a, e
    add a, 16
    ld e, a
    ld a, b
    add a, c
    ld [hli], a
    ld b, 0
    .digit1000:
    ld a, e
    sub a, 232
    ld e, a
    ld a, d
    sbc a, 3
    jr c, .end1000
    ld d, a
    inc b
    jr .digit1000
    .end1000:
    ld a, e
    add a, 232
    ld e, a
    ld a, b
    add a, c
    ld [hli], a
    ld b, 0
    .digit100:
    ld a, e
    sub a, 100
    ld e, a
    ld a, d
    sbc a, 0
    jr c, .end100
    ld d, a
    inc b
    jr .digit100
    .end100:
    ld a, e
    add a, 100
    ld e, a
    ld a, b
    add a, c
    ld [hli], a
    ld b, 0
    .digit10:
    ld a, e
    sub a, 10
    ld e, a
    ld a, d
    sbc a, 0
    jr c, .end10
    ld d, a
    inc b
    jr .digit10
    .end10:
    ld a, e
    add a, 10
    ld e, a
    ld a, b
    add a, c
    ld [hli], a
    ld b, 0
    .digit1:
    ld a, e
    sub a, 1
    ld e, a
    ld a, d
    sbc a, 0
    jr c, .end1
    ld d, a
    inc b
    jr .digit1
    .end1:
    ld a, e
    add a, 1
    ld e, a
    ld a, b
    add a, c
    ld [hli], a
    and a, a
    ret
    Anim_player_left_playerWalkFront:
    ld a, [wShadowOAM+2]
    cp a, 0
    jr c, .reset_playerWalkFront
    cp a, 6
    jr c, .next_playerWalkFront
    .reset_playerWalkFront:
    ld a, 254
    .next_playerWalkFront:
    add a, 2
    ld [wShadowOAM+2], a
    ret
    Anim_player_left_playerWalkBack:
    ld a, [wShadowOAM+2]
    cp a, 8
    jr c, .reset_playerWalkBack
    cp a, 14
    jr c, .next_playerWalkBack
    .reset_playerWalkBack:
    ld a, 6
    .next_playerWalkBack:
    add a, 2
    ld [wShadowOAM+2], a
    ret
    Anim_player_left_playerWalkLeft:
    ld a, [wShadowOAM+2]
    cp a, 16
    jr c, .reset_playerWalkLeft
    cp a, 22
    jr c, .next_playerWalkLeft
    .reset_playerWalkLeft:
    ld a, 14
    .next_playerWalkLeft:
    add a, 2
    ld [wShadowOAM+2], a
    ret
    Anim_player_left_playerWalkRight:
    ld a, [wShadowOAM+2]
    cp a, 24
    jr c, .reset_playerWalkRight
    cp a, 30
    jr c, .next_playerWalkRight
    .reset_playerWalkRight:
    ld a, 22
    .next_playerWalkRight:
    add a, 2
    ld [wShadowOAM+2], a
    ret
    Anim_player_right_playerWalkFront:
    ld a, [wShadowOAM+6]
    cp a, 64
    jr c, .reset_playerWalkFront
    cp a, 70
    jr c, .next_playerWalkFront
    .reset_playerWalkFront:
    ld a, 62
    .next_playerWalkFront:
    add a, 2
    ld [wShadowOAM+6], a
    ret
    Anim_player_right_playerWalkBack:
    ld a, [wShadowOAM+6]
    cp a, 72
    jr c, .reset_playerWalkBack
    cp a, 78
    jr c, .next_playerWalkBack
    .reset_playerWalkBack:
    ld a, 70
    .next_playerWalkBack:
    add a, 2
    ld [wShadowOAM+6], a
    ret
    Anim_player_right_playerWalkLeft:
    ld a, [wShadowOAM+6]
    cp a, 80
    jr c, .reset_playerWalkLeft
    cp a, 86
    jr c, .next_playerWalkLeft
    .reset_playerWalkLeft:
    ld a, 78
    .next_playerWalkLeft:
    add a, 2
    ld [wShadowOAM+6], a
    ret
    Anim_player_right_playerWalkRight:
    ld a, [wShadowOAM+6]
    cp a, 88
    jr c, .reset_playerWalkRight
    cp a, 94
    jr c, .next_playerWalkRight
    .reset_playerWalkRight:
    ld a, 86
    .next_playerWalkRight:
    add a, 2
    ld [wShadowOAM+6], a
    ret
    VBlankHandler:
    push af
    push bc
    push de
    push hl
    ld a, HIGH(wShadowOAM)
    call hOAMDMA
    call FlushVramQueue
    ld a, 1
    ld [wVBlankFlag], a
    pop hl
    pop de
    pop bc
    pop af
    reti

    player_left:
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1E17
    dw $0A0F
    dw $0A0F
    dw $141F
    dw $233F
    dw $547F
    dw $4C7F
    dw $3F3B
    dw $0F09
    dw $1F19
    dw $3F3F
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1E17
    dw $0A0F
    dw $0A0F
    dw $141F
    dw $233F
    dw $2C3F
    dw $1F1B
    dw $0F09
    dw $1F1D
    dw $3F3F
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1E17
    dw $0A0F
    dw $0A0F
    dw $141F
    dw $233F
    dw $547F
    dw $4C7F
    dw $3F3B
    dw $0F09
    dw $0F0E
    dw $1F1F
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1E17
    dw $0A0F
    dw $1A1F
    dw $243F
    dw $273F
    dw $1C1F
    dw $0F09
    dw $1F1F
    dw $0F0F
    dw $0000
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $0F08
    dw $171F
    dw $203F
    dw $283F
    dw $1F1B
    dw $0F09
    dw $1F1D
    dw $3F3F
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $0F08
    dw $171F
    dw $203F
    dw $547F
    dw $4C7F
    dw $3F3B
    dw $0F09
    dw $1F1D
    dw $3F3F
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F18
    dw $273F
    dw $203F
    dw $1C1F
    dw $0F09
    dw $1F1F
    dw $0F0F
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $0F08
    dw $171F
    dw $203F
    dw $547F
    dw $4C7F
    dw $3F3B
    dw $0F09
    dw $0F0E
    dw $1F1F
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1B17
    dw $0A0F
    dw $0A0F
    dw $080F
    dw $171F
    dw $141F
    dw $1F1C
    dw $3F39
    dw $3F3F
    dw $1E1E
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1B17
    dw $0A0F
    dw $0A0F
    dw $080F
    dw $0707
    dw $0407
    dw $0407
    dw $0303
    dw $0302
    dw $0302
    dw $0707
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1B17
    dw $0A0F
    dw $0A0F
    dw $181F
    dw $273F
    dw $203F
    dw $1F1C
    dw $3F3F
    dw $3F3D
    dw $1E1E
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1B17
    dw $0A0F
    dw $0A0F
    dw $080F
    dw $0707
    dw $0407
    dw $0407
    dw $0303
    dw $0302
    dw $0302
    dw $0707
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1E11
    dw $1E11
    dw $3C3B
    dw $477F
    dw $84FF
    dw $8CFF
    dw $9FFE
    dw $7F73
    dw $7C7C
    dw $3E3E
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1E11
    dw $1E11
    dw $0C0B
    dw $0707
    dw $090F
    dw $121F
    dw $121F
    dw $131E
    dw $0B0E
    dw $0F0E
    dw $0707
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1E11
    dw $1E11
    dw $1C1B
    dw $677F
    dw $A0FF
    dw $A8FF
    dw $7F44
    dw $7F63
    dw $7C7C
    dw $3E3E
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1E11
    dw $1E11
    dw $0C0B
    dw $0707
    dw $090F
    dw $121F
    dw $121F
    dw $131E
    dw $0B0E
    dw $0F0E
    dw $0707
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1E17
    dw $0A0F
    dw $0A0F
    dw $141F
    dw $233F
    dw $547F
    dw $4C7F
    dw $3F3B
    dw $0F09
    dw $1F1D
    dw $3F3F
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1B17
    dw $0A0F
    dw $0A0F
    dw $080F
    dw $171F
    dw $141F
    dw $0C0F
    dw $0303
    dw $0302
    dw $0302
    dw $0707
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1E11
    dw $1E11
    dw $0C0B
    dw $0707
    dw $090F
    dw $121F
    dw $121F
    dw $131E
    dw $0B0E
    dw $0F0E
    dw $0707
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $0F08
    dw $171F
    dw $203F
    dw $283F
    dw $1F1B
    dw $0F09
    dw $0F0E
    dw $1F1F
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1817
    dw $7E7F
    dw $AAFF
    dw $94FF
    dw $7B7F
    dw $111F
    dw $1C1F
    dw $0F0B
    dw $0F08
    dw $0706
    dw $0F0F
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1817
    dw $0E0F
    dw $1A1F
    dw $243F
    dw $2F3F
    dw $233F
    dw $313F
    dw $191F
    dw $0707
    dw $0707
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $3F38
    dw $477F
    dw $88FF
    dw $98FF
    dw $7F77
    dw $1F1D
    dw $3F3F
    dw $1E1E
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $0F0C
    dw $131F
    dw $243F
    dw $243F
    dw $1C1F
    dw $0F0B
    dw $0F09
    dw $0F0E
    dw $1F1F
    dw $0000
    dw $0F0F
    dw $1F10
    dw $3F20
    dw $3F38
    dw $3F24
    dw $3B27
    dw $121F
    dw $223F
    dw $223F
    dw $1D1F
    dw $080F
    dw $1E17
    dw $3F23
    dw $7E7E
    dw $3E3E
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1E11
    dw $1E11
    dw $0F0B
    dw $0C0F
    dw $141F
    dw $131F
    dw $181F
    dw $3F21
    dw $7F7B
    dw $3E3E
    dw $0000
    dw $0303
    dw $0704
    dw $0F08
    dw $1F10
    dw $1F10
    dw $0D0B
    dw $0507
    dw $0507
    dw $0407
    dw $0B0F
    dw $0A0F
    dw $1C1F
    dw $3F39
    dw $3F3F
    dw $1E1E
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1B17
    dw $0A0F
    dw $0A0F
    dw $1F1F
    dw $203F
    dw $203F
    dw $1F1F
    dw $0F08
    dw $0F0B
    dw $1F1F
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1817
    dw $0E0F
    dw $080F
    dw $141F
    dw $233F
    dw $547F
    dw $4C7F
    dw $3F3B
    dw $0F09
    dw $1F1D
    dw $3F3F
    dw $0000
    dw $0000
    dw $0000
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1817
    dw $080F
    dw $0E0F
    dw $141F
    dw $2F3F
    dw $5F73
    dw $3F3F
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1E17
    dw $0A0F
    dw $0A0F
    dw $141F
    dw $233F
    dw $547F
    dw $4C7F
    dw $3F3B
    dw $1F1D
    dw $3F3F
    dw $0303
    dw $0704
    dw $0F08
    dw $1F10
    dw $1F11
    dw $6F6B
    dw $9DFB
    dw $85FF
    dw $627F
    dw $313F
    dw $7C4F
    dw $7A77
    dw $7857
    dw $7F57
    dw $3838
    dw $0000
    player_leftEnd:
    player_right:
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $F8E8
    dw $50F0
    dw $50F0
    dw $28F8
    dw $C4FC
    dw $2AFE
    dw $32FE
    dw $FCDC
    dw $F090
    dw $F030
    dw $F8F8
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $F8E8
    dw $50F0
    dw $58F8
    dw $24FC
    dw $E4FC
    dw $38F8
    dw $F090
    dw $F8F8
    dw $F0F0
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $F8E8
    dw $50F0
    dw $50F0
    dw $28F8
    dw $C4FC
    dw $2AFE
    dw $32FE
    dw $FCDC
    dw $F090
    dw $F8B8
    dw $FCFC
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $F8E8
    dw $50F0
    dw $50F0
    dw $28F8
    dw $C4FC
    dw $34FC
    dw $F8D8
    dw $F090
    dw $F8B8
    dw $FCFC
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F818
    dw $E4FC
    dw $04FC
    dw $38F8
    dw $F090
    dw $F8F8
    dw $F0F0
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F010
    dw $E8F8
    dw $04FC
    dw $2AFE
    dw $32FE
    dw $FCDC
    dw $F090
    dw $F070
    dw $F8F8
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F010
    dw $E8F8
    dw $04FC
    dw $14FC
    dw $F8D8
    dw $F090
    dw $F8B8
    dw $FCFC
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F010
    dw $E8F8
    dw $04FC
    dw $2AFE
    dw $32FE
    dw $FCDC
    dw $F090
    dw $F8B8
    dw $FCFC
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $7888
    dw $38C8
    dw $3CDC
    dw $E2FE
    dw $21FF
    dw $31FF
    dw $F97F
    dw $FECE
    dw $3E3E
    dw $7C7C
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $7888
    dw $7888
    dw $30D0
    dw $E0E0
    dw $90F0
    dw $48F8
    dw $48F8
    dw $C878
    dw $D070
    dw $F070
    dw $E0E0
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $7888
    dw $7888
    dw $38D8
    dw $E6FE
    dw $05FF
    dw $15FF
    dw $FE22
    dw $FEC6
    dw $3E3E
    dw $7C7C
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $7888
    dw $7888
    dw $30D0
    dw $E0E0
    dw $90F0
    dw $48F8
    dw $48F8
    dw $C878
    dw $D070
    dw $F070
    dw $E0E0
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $FC04
    dw $FC04
    dw $D8E8
    dw $50F0
    dw $50F0
    dw $10F0
    dw $E8F8
    dw $28F8
    dw $F838
    dw $FC9C
    dw $FCFC
    dw $7878
    dw $E0E0
    dw $F010
    dw $F808
    dw $FC04
    dw $FC04
    dw $D8E8
    dw $50F0
    dw $50F0
    dw $10F0
    dw $E0E0
    dw $20E0
    dw $20E0
    dw $C0C0
    dw $C040
    dw $C040
    dw $E0E0
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $FC04
    dw $FC04
    dw $D8E8
    dw $50F0
    dw $50F0
    dw $18F8
    dw $E4FC
    dw $04FC
    dw $F838
    dw $FCFC
    dw $FCBC
    dw $7878
    dw $E0E0
    dw $F010
    dw $F808
    dw $FC04
    dw $FC04
    dw $D8E8
    dw $50F0
    dw $50F0
    dw $10F0
    dw $E0E0
    dw $20E0
    dw $20E0
    dw $C0C0
    dw $C040
    dw $C040
    dw $E0E0
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $F8E8
    dw $50F0
    dw $58F8
    dw $24FC
    dw $E4FC
    dw $38F8
    dw $30F0
    dw $F0D0
    dw $F090
    dw $F070
    dw $F8F8
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $7888
    dw $7888
    dw $30D0
    dw $E0E0
    dw $90F0
    dw $48F8
    dw $48F8
    dw $C878
    dw $D070
    dw $F070
    dw $E0E0
    dw $E0E0
    dw $F010
    dw $F808
    dw $FC04
    dw $FC04
    dw $D8E8
    dw $50F0
    dw $50F0
    dw $10F0
    dw $E8F8
    dw $28F8
    dw $30F0
    dw $C0C0
    dw $C040
    dw $C040
    dw $E0E0
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F818
    dw $E4FC
    dw $04FC
    dw $38F8
    dw $F0D0
    dw $F090
    dw $F8B8
    dw $FCFC
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $18E8
    dw $F0F0
    dw $58F8
    dw $24FC
    dw $C2FE
    dw $91FF
    dw $39FF
    dw $FEDE
    dw $F0F0
    dw $E0E0
    dw $8080
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $18E8
    dw $F0F0
    dw $58F8
    dw $24FC
    dw $E2FE
    dw $92FE
    dw $FCFC
    dw $F010
    dw $E060
    dw $F0F0
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F818
    dw $FC24
    dw $E4FC
    dw $18F8
    dw $60E0
    dw $E0A0
    dw $E020
    dw $F070
    dw $F8F8
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F010
    dw $E8F8
    dw $18F8
    dw $70F0
    dw $90F0
    dw $F0B0
    dw $F0F0
    dw $E0E0
    dw $8080
    dw $0000
    dw $C0C0
    dw $E020
    dw $F010
    dw $F808
    dw $F808
    dw $B0D0
    dw $A0E0
    dw $A0E0
    dw $20E0
    dw $D0F0
    dw $50F0
    dw $38F8
    dw $FC9C
    dw $FCFC
    dw $7878
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $FC04
    dw $FC04
    dw $D8E8
    dw $50F0
    dw $50F0
    dw $F8F8
    dw $04FC
    dw $04FC
    dw $F8F8
    dw $F010
    dw $F0D0
    dw $F8F8
    dw $0000
    dw $F0F0
    dw $F808
    dw $FC04
    dw $FC1C
    dw $FC24
    dw $DCE4
    dw $48F8
    dw $44FC
    dw $44FC
    dw $B8F8
    dw $10F0
    dw $78E8
    dw $FCC4
    dw $7E7E
    dw $7C7C
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $7888
    dw $7888
    dw $F0D0
    dw $30F0
    dw $28F8
    dw $C8F8
    dw $18F8
    dw $FC84
    dw $FEDE
    dw $7C7C
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $18E8
    dw $F0F0
    dw $10F0
    dw $28F8
    dw $C4FC
    dw $2AFE
    dw $32FE
    dw $FCDC
    dw $F090
    dw $F070
    dw $F8F8
    dw $0000
    dw $0000
    dw $0000
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $18E8
    dw $10F0
    dw $D0F0
    dw $28F8
    dw $E4FC
    dw $F29E
    dw $FCFC
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $F8E8
    dw $50F0
    dw $50F0
    dw $28F8
    dw $C4FC
    dw $2AFE
    dw $32FE
    dw $FCDC
    dw $F070
    dw $F8F8
    dw $F0F0
    dw $F808
    dw $FC04
    dw $FC04
    dw $FC04
    dw $7EE6
    dw $59EF
    dw $49FF
    dw $92FE
    dw $E4FC
    dw $18F8
    dw $3EF2
    dw $1EEE
    dw $DEEA
    dw $7E6A
    dw $3C3C
    player_rightEnd:
    BgTile:
    dw $00FF
    dw $42BD
    dw $02FD
    dw $10EF
    dw $10EF
    dw $40BF
    dw $44BB
    dw $00FF
    BgTileEnd:
    OliveTree:
    dw $00FF
    dw $42BD
    dw $02FD
    dw $17EF
    dw $18FF
    dw $72AD
    dw $65DA
    dw $6BD4
    dw $00FF
    dw $42BD
    dw $02FD
    dw $F0FF
    dw $B84F
    dw $5EA3
    dw $AF51
    dw $EF11
    dw $77C8
    dw $7FC1
    dw $FFE1
    dw $7FFD
    dw $39EF
    dw $43BF
    dw $4FBC
    dw $07F9
    dw $EF11
    dw $FF03
    dw $FFE7
    dw $7CBF
    dw $708F
    dw $58AF
    dw $BE47
    dw $F8E7
    OliveTreeEnd:
    OliveTreeFull:
    dw $00FF
    dw $42BD
    dw $02FD
    dw $17EF
    dw $18FF
    dw $72AF
    dw $65DE
    dw $7BD4
    dw $00FF
    dw $42BD
    dw $02FD
    dw $F0FF
    dw $B84F
    dw $DEB3
    dw $AF71
    dw $FF95
    dw $77CA
    dw $7FC9
    dw $FFE1
    dw $7FFD
    dw $39EF
    dw $43BF
    dw $4FBC
    dw $07F9
    dw $EF31
    dw $FF0B
    dw $FFE7
    dw $7CBF
    dw $708F
    dw $58AF
    dw $BE47
    dw $F8E7
    OliveTreeFullEnd:
    HudBlank:
    dw $0000
    dw $0000
    dw $0000
    dw $0000
    dw $0000
    dw $0000
    dw $0000
    dw $0000
    HudBlankEnd:
    HudOlive:
    dw $0000
    dw $0303
    dw $0704
    dw $0E09
    dw $1D12
    dw $3B25
    dw $372A
    dw $6F5C
    dw $0000
    dw $F0F0
    dw $B0F0
    dw $D858
    dw $F8F8
    dw $F818
    dw $B848
    dw $9868
    dw $5774
    dw $6F68
    dw $0F08
    dw $0F08
    dw $0F08
    dw $0706
    dw $0101
    dw $0000
    dw $CC34
    dw $CC34
    dw $9868
    dw $F808
    dw $F010
    dw $E060
    dw $8080
    dw $0000
    HudOliveEnd:
    Digits:
    dw $3838
    dw $4444
    dw $4C4C
    dw $5454
    dw $6464
    dw $4444
    dw $3838
    dw $0000
    dw $1010
    dw $3030
    dw $1010
    dw $1010
    dw $1010
    dw $1010
    dw $3838
    dw $0000
    dw $3838
    dw $4444
    dw $0404
    dw $0808
    dw $1010
    dw $2020
    dw $7C7C
    dw $0000
    dw $7C7C
    dw $0808
    dw $1010
    dw $0808
    dw $0404
    dw $4444
    dw $3838
    dw $0000
    dw $0808
    dw $1818
    dw $2828
    dw $4848
    dw $7C7C
    dw $0808
    dw $0808
    dw $0000
    dw $7C7C
    dw $4040
    dw $7878
    dw $0404
    dw $0404
    dw $4444
    dw $3838
    dw $0000
    dw $1818
    dw $2020
    dw $4040
    dw $7878
    dw $4444
    dw $4444
    dw $3838
    dw $0000
    dw $7C7C
    dw $0404
    dw $0808
    dw $1010
    dw $2020
    dw $2020
    dw $2020
    dw $0000
    dw $3838
    dw $4444
    dw $4444
    dw $3838
    dw $4444
    dw $4444
    dw $3838
    dw $0000
    dw $3838
    dw $4444
    dw $4444
    dw $3C3C
    dw $0404
    dw $0808
    dw $3030
    dw $0000
    DigitsEnd:
    TextOlivesTargets:
    dw $9C24, $9C25, $9C26, $9C27, $9C28

    BgMap:
    db $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00
    db $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00
    db $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00
    db $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00
    db $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00
    db $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00
    db $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00
    db $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00, $05, $06, $00, $00
    db $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00, $07, $08, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    BgMapEnd:
    Hud:
    db $09, $0A, $0B, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09
    db $09, $0C, $0D, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09
    HudEnd:

    SECTION "Shadow OAM", WRAM0[$CF00]
    wShadowOAM:
    ds 160
    SECTION "OAM DMA", HRAM[$FFB8]
    hOAMDMA:
    ds 8
    SECTION "VRAM queue", WRAM0[$CED9]
    wVramQueue:
    ds 39
    SECTION "VRAM queue read cursor", WRAM0[$CED8]
    wVramQueueRead:
    ds 1
    SECTION "VRAM queue write cursor", WRAM0[$CED7]
    wVramQueueWrite:
    ds 1
    SECTION "Variables", WRAM0[$C000]
    wTextOlivesGlyphs:
    ds 5
    wTextOlivesCursor: dw
    wOlivesHarvested: dw
    wCurKeys: db
    wNewKeys: db
    wReleasedKeys: db
    wVBlankFlag: db
    wFrameCounter: db
    wAnim_player_left_Current: db
    wAnim_player_right_Current: db


