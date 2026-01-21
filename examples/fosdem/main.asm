    INCLUDE "hardware.inc"
    SECTION "Header", ROM0[$100]
    jp EntryPoint
    ds $150 - @, 0

    EntryPoint:
    call WaitVBlank
    ld a, 0
    ld [rLCDC], a
    ld de, player
    ld hl, $8000
    ld bc, playerEnd - player
    call Memcopy
    ld de, playerDx
    ld hl, $8400
    ld bc, playerDxEnd - playerDx
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
    ld a, 0
    ld [wNewKeys], a
    ld a, 1
    ld [wAnim_playerWalkDx_Active], a
    ld a, 0
    ld [wCurKeys], a
    ld a, 0
    ld [wFrameCounter], a
    ld a, 1
    ld [wAnim_playerWalk_Active], a
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
    ld a, [wAnim_playerWalk_Active]
    cp 0
    jr z, .skip_playerWalk
    call Anim_playerWalk
    .skip_playerWalk:
    ld a, [wAnim_playerWalkDx_Active]
    cp 0
    jr z, .skip_playerWalkDx
    call Anim_playerWalkDx
    .skip_playerWalkDx:
    AnimEnd:
    call UpdateKeys
    CheckA:
    ld a, [wCurKeys]
    and a, PADF_A
    jp z, CheckAEnd
    ld a, 1
    ld [wAnim_playerWalk_Active], a
    ld a, 1
    ld [wAnim_playerWalkDx_Active], a
    CheckAEnd:
    CheckB:
    ld a, [wCurKeys]
    and a, PADF_B
    jp z, CheckBEnd
    ld a, 0
    ld [wAnim_playerWalk_Active], a
    ld a, 0
    ld [wAnim_playerWalkDx_Active], a
    CheckBEnd:
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
    CheckDownEnd:
    jp Main

    WaitVBlank:
    ld a, [rLY]
    cp 144
    jp c, WaitVBlank
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
    WaitNotVBlank:
    ld a, [rLY]
    cp 144
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
    Anim_playerWalk:
    ld a, [_OAMRAM+2]
    inc a
    cp 5
    jr nz, updateSpriteIndex_playerWalk
    ld a, 0
    updateSpriteIndex_playerWalk:
    ld [_OAMRAM+2], a
    ret
    Anim_playerWalkDx:
    ld a, [_OAMRAM+6]
    inc a
    cp 69
    jr nz, updateSpriteIndex_playerWalkDx
    ld a, 64
    updateSpriteIndex_playerWalkDx:
    ld [_OAMRAM+6], a
    ret

    player:
    INCBIN "char.2bpp"
    playerEnd:
    playerDx:
    INCBIN "char-dx.2bpp"
    playerDxEnd:

    SECTION "Variables", WRAM0
    wCurKeys: db
    wNewKeys: db
    wFrameCounter: db
    wAnim_playerWalk_Active: db
    wAnim_playerWalkDx_Active: db


