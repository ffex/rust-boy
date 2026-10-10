    INCLUDE "hardware.inc"
    SECTION "Header", ROM0[$100]
    jp EntryPoint
    ds $150 - @, 0

    EntryPoint:
    call WaitVBlank
    ld a, 0
    ld [rLCDC], a
    ld de, player_left
    ld hl, $8000
    ld bc, player_leftEnd - player_left
    call Memcopy
    ld de, player_right
    ld hl, $8400
    ld bc, player_rightEnd - player_right
    call Memcopy
    ld a, 0
    ld b, 160
    ld hl, _OAMRAM
    .clear_oam_8:
    ld [hli], a
    dec b
    jp nz, .clear_oam_8
    ld hl, _OAMRAM
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
    ld [wAnim_player_left_Current], a
    ld a, 255
    ld [wAnim_player_right_Current], a
    ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | LCDCF_OBJ16
    ld [rLCDC], a

    Main:
    call WaitNotVBlank
    call WaitVBlank
    ld a, [wFrameCounter]
    inc a
    ld [wFrameCounter], a
    cp a, 8
    jp c, .anim_end_9
    ld a, 0
    ld [wFrameCounter], a
    ld a, [wAnim_player_left_Current]
    cp a, 255
    jp z, .anim_player_left_end_10
    cp a, 0
    jr nz, .skip_player_left_playerWalkFront_11
    call Anim_player_left_playerWalkFront
    jp .anim_player_left_end_10
    .skip_player_left_playerWalkFront_11:
    cp a, 1
    jr nz, .skip_player_left_playerWalkBack_12
    call Anim_player_left_playerWalkBack
    jp .anim_player_left_end_10
    .skip_player_left_playerWalkBack_12:
    cp a, 2
    jr nz, .skip_player_left_playerWalkLeft_13
    call Anim_player_left_playerWalkLeft
    jp .anim_player_left_end_10
    .skip_player_left_playerWalkLeft_13:
    cp a, 3
    jr nz, .skip_player_left_playerWalkRight_14
    call Anim_player_left_playerWalkRight
    jp .anim_player_left_end_10
    .skip_player_left_playerWalkRight_14:
    .anim_player_left_end_10:
    ld a, [wAnim_player_right_Current]
    cp a, 255
    jp z, .anim_player_right_end_15
    cp a, 0
    jr nz, .skip_player_right_playerWalkFront_16
    call Anim_player_right_playerWalkFront
    jp .anim_player_right_end_15
    .skip_player_right_playerWalkFront_16:
    cp a, 1
    jr nz, .skip_player_right_playerWalkBack_17
    call Anim_player_right_playerWalkBack
    jp .anim_player_right_end_15
    .skip_player_right_playerWalkBack_17:
    cp a, 2
    jr nz, .skip_player_right_playerWalkLeft_18
    call Anim_player_right_playerWalkLeft
    jp .anim_player_right_end_15
    .skip_player_right_playerWalkLeft_18:
    cp a, 3
    jr nz, .skip_player_right_playerWalkRight_19
    call Anim_player_right_playerWalkRight
    jp .anim_player_right_end_15
    .skip_player_right_playerWalkRight_19:
    .anim_player_right_end_15:
    .anim_end_9:
    call UpdateKeys
    .check_left_4:
    ld a, [wCurKeys]
    and a, PADF_LEFT
    jp z, .check_left_end_4
    ld a, [_OAMRAM+1]
    sub a, 1
    jp c, .sprite0_left_limit_end_0
    sub a, 1
    jp nc, .sprite0_left_limit_store_0
    ld a, 0
    .sprite0_left_limit_store_0:
    add a, 1
    ld [_OAMRAM+1], a
    add a, 8
    ld [_OAMRAM+5], a
    .sprite0_left_limit_end_0:
    ld a, 2
    ld [wAnim_player_left_Current], a
    ld a, 2
    ld [wAnim_player_right_Current], a
    .check_left_end_4:
    .check_right_5:
    ld a, [wCurKeys]
    and a, PADF_RIGHT
    jp z, .check_right_end_5
    ld a, [_OAMRAM+5]
    sub a, 149
    jp nc, .sprite1_right_limit_end_1
    add a, 1
    jp nc, .sprite1_right_limit_store_1
    ld a, 0
    .sprite1_right_limit_store_1:
    add a, 149
    ld [_OAMRAM+5], a
    sub a, 8
    ld [_OAMRAM+1], a
    .sprite1_right_limit_end_1:
    ld a, 3
    ld [wAnim_player_left_Current], a
    ld a, 3
    ld [wAnim_player_right_Current], a
    .check_right_end_5:
    .check_up_6:
    ld a, [wCurKeys]
    and a, PADF_UP
    jp z, .check_up_end_6
    ld a, [_OAMRAM+0]
    sub a, 1
    jp c, .sprite0_up_limit_end_2
    sub a, 1
    jp nc, .sprite0_up_limit_store_2
    ld a, 0
    .sprite0_up_limit_store_2:
    add a, 1
    ld [_OAMRAM+0], a
    ld [_OAMRAM+4], a
    .sprite0_up_limit_end_2:
    ld a, 1
    ld [wAnim_player_left_Current], a
    ld a, 1
    ld [wAnim_player_right_Current], a
    .check_up_end_6:
    .check_down_7:
    ld a, [wCurKeys]
    and a, PADF_DOWN
    jp z, .check_down_end_7
    ld a, [_OAMRAM+0]
    sub a, 149
    jp nc, .sprite0_down_limit_end_3
    add a, 1
    jp nc, .sprite0_down_limit_store_3
    ld a, 0
    .sprite0_down_limit_store_3:
    add a, 149
    ld [_OAMRAM+0], a
    ld [_OAMRAM+4], a
    .sprite0_down_limit_end_3:
    ld a, 0
    ld [wAnim_player_left_Current], a
    ld a, 0
    ld [wAnim_player_right_Current], a
    .check_down_end_7:
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
    Anim_player_left_playerWalkFront:
    ld a, [_OAMRAM+2]
    cp a, 0
    jr c, .reset_playerWalkFront
    cp a, 6
    jr c, .next_playerWalkFront
    .reset_playerWalkFront:
    ld a, 254
    .next_playerWalkFront:
    add a, 2
    ld [_OAMRAM+2], a
    ret
    Anim_player_left_playerWalkBack:
    ld a, [_OAMRAM+2]
    cp a, 8
    jr c, .reset_playerWalkBack
    cp a, 14
    jr c, .next_playerWalkBack
    .reset_playerWalkBack:
    ld a, 6
    .next_playerWalkBack:
    add a, 2
    ld [_OAMRAM+2], a
    ret
    Anim_player_left_playerWalkLeft:
    ld a, [_OAMRAM+2]
    cp a, 16
    jr c, .reset_playerWalkLeft
    cp a, 22
    jr c, .next_playerWalkLeft
    .reset_playerWalkLeft:
    ld a, 14
    .next_playerWalkLeft:
    add a, 2
    ld [_OAMRAM+2], a
    ret
    Anim_player_left_playerWalkRight:
    ld a, [_OAMRAM+2]
    cp a, 24
    jr c, .reset_playerWalkRight
    cp a, 30
    jr c, .next_playerWalkRight
    .reset_playerWalkRight:
    ld a, 22
    .next_playerWalkRight:
    add a, 2
    ld [_OAMRAM+2], a
    ret
    Anim_player_right_playerWalkFront:
    ld a, [_OAMRAM+6]
    cp a, 64
    jr c, .reset_playerWalkFront
    cp a, 70
    jr c, .next_playerWalkFront
    .reset_playerWalkFront:
    ld a, 62
    .next_playerWalkFront:
    add a, 2
    ld [_OAMRAM+6], a
    ret
    Anim_player_right_playerWalkBack:
    ld a, [_OAMRAM+6]
    cp a, 72
    jr c, .reset_playerWalkBack
    cp a, 78
    jr c, .next_playerWalkBack
    .reset_playerWalkBack:
    ld a, 70
    .next_playerWalkBack:
    add a, 2
    ld [_OAMRAM+6], a
    ret
    Anim_player_right_playerWalkLeft:
    ld a, [_OAMRAM+6]
    cp a, 80
    jr c, .reset_playerWalkLeft
    cp a, 86
    jr c, .next_playerWalkLeft
    .reset_playerWalkLeft:
    ld a, 78
    .next_playerWalkLeft:
    add a, 2
    ld [_OAMRAM+6], a
    ret
    Anim_player_right_playerWalkRight:
    ld a, [_OAMRAM+6]
    cp a, 88
    jr c, .reset_playerWalkRight
    cp a, 94
    jr c, .next_playerWalkRight
    .reset_playerWalkRight:
    ld a, 86
    .next_playerWalkRight:
    add a, 2
    ld [_OAMRAM+6], a
    ret

    player_left:
    INCBIN "char.2bpp"
    player_leftEnd:
    player_right:
    INCBIN "char-dx.2bpp"
    player_rightEnd:

    SECTION "Variables", WRAM0[$C000]
    wCurKeys: db
    wNewKeys: db
    wFrameCounter: db
    wAnim_player_left_Current: db
    wAnim_player_right_Current: db


