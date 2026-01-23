    INCLUDE "hardware.inc"
    SECTION "Header", ROM0[$100]
    jp EntryPoint
    ds $150 - @, 0

    EntryPoint:
    call WaitVBlank
    ld a, 0
    ld [rLCDC], a
    ld de, player_right
    ld hl, $8400
    ld bc, player_rightEnd - player_right
    call Memcopy
    ld de, player_left
    ld hl, $8000
    ld bc, player_leftEnd - player_left
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
    ld a, 88
    ld [hli], a
    ld a, 96
    ld [hli], a
    ld a, 64
    ld [hli], a
    ld a, 0
    ld [hli], a
    ld a, 255
    ld [wAnim_player_left_Current], a
    ld a, 255
    ld [wAnim_player_right_Current], a
    ld a, 0
    ld [wCurKeys], a
    ld a, 0
    ld [wFrameCounter], a
    ld a, 0
    ld [wNewKeys], a
    ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | LCDCF_OBJ16
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
    ld a, [wAnim_player_left_Current]
    cp 255
    jr z, .animEnd_player_left
    cp 0
    jr nz, .skip_playerWalkFront_0
    call Anim_playerWalkFront_0
    jr .animEnd_player_left
    .skip_playerWalkFront_0:
    cp 1
    jr nz, .skip_playerWalkBack_0
    call Anim_playerWalkBack_0
    jr .animEnd_player_left
    .skip_playerWalkBack_0:
    cp 2
    jr nz, .skip_playerWalkLeft_0
    call Anim_playerWalkLeft_0
    jr .animEnd_player_left
    .skip_playerWalkLeft_0:
    cp 3
    jr nz, .skip_playerWalkRight_0
    call Anim_playerWalkRight_0
    jr .animEnd_player_left
    .skip_playerWalkRight_0:
    .animEnd_player_left:
    ld a, [wAnim_player_right_Current]
    cp 255
    jr z, .animEnd_player_right
    cp 0
    jr nz, .skip_playerWalkFront_1
    call Anim_playerWalkFront_1
    jr .animEnd_player_right
    .skip_playerWalkFront_1:
    cp 1
    jr nz, .skip_playerWalkBack_1
    call Anim_playerWalkBack_1
    jr .animEnd_player_right
    .skip_playerWalkBack_1:
    cp 2
    jr nz, .skip_playerWalkLeft_1
    call Anim_playerWalkLeft_1
    jr .animEnd_player_right
    .skip_playerWalkLeft_1:
    cp 3
    jr nz, .skip_playerWalkRight_1
    call Anim_playerWalkRight_1
    jr .animEnd_player_right
    .skip_playerWalkRight_1:
    .animEnd_player_right:
    AnimEnd:
    call UpdateKeys
    CheckLeft:
    ld a, [wCurKeys]
    and a, PADF_LEFT
    jp z, CheckLeftEnd
    ld a, [_OAMRAM+1]
    sub a, 1
    cp 0
    jp z, Sprite0LeftLimitEnd
    ld [_OAMRAM+1], a
    Sprite0LeftLimitEnd:
    ld a, [_OAMRAM+5]
    sub a, 1
    cp 0
    jp z, Sprite1LeftLimitEnd
    ld [_OAMRAM+5], a
    Sprite1LeftLimitEnd:
    ld a, 2
    ld [wAnim_player_left_Current], a
    ld a, 2
    ld [wAnim_player_right_Current], a
    CheckLeftEnd:
    CheckRight:
    ld a, [wCurKeys]
    and a, PADF_RIGHT
    jp z, CheckRightEnd
    ld a, [_OAMRAM+1]
    add a, 1
    cp 150
    jp z, Sprite0RightLimitEnd
    ld [_OAMRAM+1], a
    Sprite0RightLimitEnd:
    ld a, [_OAMRAM+5]
    add a, 1
    cp 150
    jp z, Sprite1RightLimitEnd
    ld [_OAMRAM+5], a
    Sprite1RightLimitEnd:
    ld a, 3
    ld [wAnim_player_left_Current], a
    ld a, 3
    ld [wAnim_player_right_Current], a
    CheckRightEnd:
    CheckUp:
    ld a, [wCurKeys]
    and a, PADF_UP
    jp z, CheckUpEnd
    ld a, [_OAMRAM+0]
    sub a, 1
    cp 0
    jp z, Sprite0UpLimitEnd
    ld [_OAMRAM+0], a
    Sprite0UpLimitEnd:
    ld a, [_OAMRAM+4]
    sub a, 1
    cp 0
    jp z, Sprite1UpLimitEnd
    ld [_OAMRAM+4], a
    Sprite1UpLimitEnd:
    ld a, 1
    ld [wAnim_player_left_Current], a
    ld a, 1
    ld [wAnim_player_right_Current], a
    CheckUpEnd:
    CheckDown:
    ld a, [wCurKeys]
    and a, PADF_DOWN
    jp z, CheckDownEnd
    ld a, [_OAMRAM+0]
    add a, 1
    cp 150
    jp z, Sprite0DownLimitEnd
    ld [_OAMRAM+0], a
    Sprite0DownLimitEnd:
    ld a, [_OAMRAM+4]
    add a, 1
    cp 150
    jp z, Sprite1DownLimitEnd
    ld [_OAMRAM+4], a
    Sprite1DownLimitEnd:
    ld a, 0
    ld [wAnim_player_left_Current], a
    ld a, 0
    ld [wAnim_player_right_Current], a
    CheckDownEnd:
    jp Main

    WaitNotVBlank:
    ld a, [rLY]
    cp 144
    jp nc, WaitNotVBlank
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
    Anim_playerWalkFront_0:
    ld a, [_OAMRAM+2]
    add a, 2
    cp 0
    jr c, .reset_playerWalkFront_0
    cp 8
    jr c, .store_playerWalkFront_0
    .reset_playerWalkFront_0:
    ld a, 0
    .store_playerWalkFront_0:
    ld [_OAMRAM+2], a
    ret
    Anim_playerWalkBack_0:
    ld a, [_OAMRAM+2]
    add a, 2
    cp 8
    jr c, .reset_playerWalkBack_0
    cp 16
    jr c, .store_playerWalkBack_0
    .reset_playerWalkBack_0:
    ld a, 8
    .store_playerWalkBack_0:
    ld [_OAMRAM+2], a
    ret
    Anim_playerWalkLeft_0:
    ld a, [_OAMRAM+2]
    add a, 2
    cp 16
    jr c, .reset_playerWalkLeft_0
    cp 24
    jr c, .store_playerWalkLeft_0
    .reset_playerWalkLeft_0:
    ld a, 16
    .store_playerWalkLeft_0:
    ld [_OAMRAM+2], a
    ret
    Anim_playerWalkRight_0:
    ld a, [_OAMRAM+2]
    add a, 2
    cp 24
    jr c, .reset_playerWalkRight_0
    cp 32
    jr c, .store_playerWalkRight_0
    .reset_playerWalkRight_0:
    ld a, 24
    .store_playerWalkRight_0:
    ld [_OAMRAM+2], a
    ret
    Anim_playerWalkFront_1:
    ld a, [_OAMRAM+6]
    add a, 2
    cp 64
    jr c, .reset_playerWalkFront_1
    cp 72
    jr c, .store_playerWalkFront_1
    .reset_playerWalkFront_1:
    ld a, 64
    .store_playerWalkFront_1:
    ld [_OAMRAM+6], a
    ret
    Anim_playerWalkBack_1:
    ld a, [_OAMRAM+6]
    add a, 2
    cp 72
    jr c, .reset_playerWalkBack_1
    cp 80
    jr c, .store_playerWalkBack_1
    .reset_playerWalkBack_1:
    ld a, 72
    .store_playerWalkBack_1:
    ld [_OAMRAM+6], a
    ret
    Anim_playerWalkLeft_1:
    ld a, [_OAMRAM+6]
    add a, 2
    cp 80
    jr c, .reset_playerWalkLeft_1
    cp 88
    jr c, .store_playerWalkLeft_1
    .reset_playerWalkLeft_1:
    ld a, 80
    .store_playerWalkLeft_1:
    ld [_OAMRAM+6], a
    ret
    Anim_playerWalkRight_1:
    ld a, [_OAMRAM+6]
    add a, 2
    cp 88
    jr c, .reset_playerWalkRight_1
    cp 96
    jr c, .store_playerWalkRight_1
    .reset_playerWalkRight_1:
    ld a, 88
    .store_playerWalkRight_1:
    ld [_OAMRAM+6], a
    ret

    player_right:
    INCBIN "char-dx.2bpp"
    player_rightEnd:
    player_left:
    INCBIN "char.2bpp"
    player_leftEnd:

    SECTION "Variables", WRAM0
    wCurKeys: db
    wNewKeys: db
    wFrameCounter: db
    wAnim_player_left_Current: db
    wAnim_player_right_Current: db


