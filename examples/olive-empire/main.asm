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
    ld de, BgMap
    ld hl, $9800
    ld bc, BgMapEnd - BgMap
    call Memcopy
    call InitOamDma
    ld a, 0
    ld b, 160
    ld hl, wShadowOAM
    .clear_oam_12:
    ld [hli], a
    dec b
    jp nz, .clear_oam_12
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
    ld a, IEF_VBLANK
    ldh [rIE], a
    ld a, LCDCF_ON | LCDCF_BGON | LCDCF_OBJON | LCDCF_OBJ16
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
    jp c, .anim_end_13
    ld a, 0
    ld [wFrameCounter], a
    ld a, [wAnim_player_left_Current]
    cp a, 255
    jp z, .anim_player_left_end_14
    cp a, 0
    jr nz, .skip_player_left_playerWalkFront_15
    call Anim_player_left_playerWalkFront
    jp .anim_player_left_end_14
    .skip_player_left_playerWalkFront_15:
    cp a, 1
    jr nz, .skip_player_left_playerWalkBack_16
    call Anim_player_left_playerWalkBack
    jp .anim_player_left_end_14
    .skip_player_left_playerWalkBack_16:
    cp a, 2
    jr nz, .skip_player_left_playerWalkLeft_17
    call Anim_player_left_playerWalkLeft
    jp .anim_player_left_end_14
    .skip_player_left_playerWalkLeft_17:
    cp a, 3
    jr nz, .skip_player_left_playerWalkRight_18
    call Anim_player_left_playerWalkRight
    jp .anim_player_left_end_14
    .skip_player_left_playerWalkRight_18:
    .anim_player_left_end_14:
    ld a, [wAnim_player_right_Current]
    cp a, 255
    jp z, .anim_player_right_end_19
    cp a, 0
    jr nz, .skip_player_right_playerWalkFront_20
    call Anim_player_right_playerWalkFront
    jp .anim_player_right_end_19
    .skip_player_right_playerWalkFront_20:
    cp a, 1
    jr nz, .skip_player_right_playerWalkBack_21
    call Anim_player_right_playerWalkBack
    jp .anim_player_right_end_19
    .skip_player_right_playerWalkBack_21:
    cp a, 2
    jr nz, .skip_player_right_playerWalkLeft_22
    call Anim_player_right_playerWalkLeft
    jp .anim_player_right_end_19
    .skip_player_right_playerWalkLeft_22:
    cp a, 3
    jr nz, .skip_player_right_playerWalkRight_23
    call Anim_player_right_playerWalkRight
    jp .anim_player_right_end_19
    .skip_player_right_playerWalkRight_23:
    .anim_player_right_end_19:
    .anim_end_13:
    call UpdateKeys
    .check_left_4:
    ld a, [wCurKeys]
    and a, PADF_LEFT
    jp z, .check_left_end_4
    ld a, [wShadowOAM+1]
    sub a, 1
    jp c, .sprite0_left_limit_end_0
    sub a, 1
    jp nc, .sprite0_left_limit_store_0
    ld a, 0
    .sprite0_left_limit_store_0:
    add a, 1
    ld [wShadowOAM+1], a
    add a, 8
    ld [wShadowOAM+5], a
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
    ld a, [wShadowOAM+5]
    sub a, 149
    jp nc, .sprite1_right_limit_end_1
    add a, 1
    jp nc, .sprite1_right_limit_store_1
    ld a, 0
    .sprite1_right_limit_store_1:
    add a, 149
    ld [wShadowOAM+5], a
    sub a, 8
    ld [wShadowOAM+1], a
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
    ld a, [wShadowOAM+0]
    sub a, 1
    jp c, .sprite0_up_limit_end_2
    sub a, 1
    jp nc, .sprite0_up_limit_store_2
    ld a, 0
    .sprite0_up_limit_store_2:
    add a, 1
    ld [wShadowOAM+0], a
    ld [wShadowOAM+4], a
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
    ld a, [wShadowOAM+0]
    sub a, 149
    jp nc, .sprite0_down_limit_end_3
    add a, 1
    jp nc, .sprite0_down_limit_store_3
    ld a, 0
    .sprite0_down_limit_store_3:
    add a, 149
    ld [wShadowOAM+0], a
    ld [wShadowOAM+4], a
    .sprite0_down_limit_end_3:
    ld a, 0
    ld [wAnim_player_left_Current], a
    ld a, 0
    ld [wAnim_player_right_Current], a
    .check_down_end_7:
    .check_left_8:
    ld a, [wReleasedKeys]
    and a, PADF_LEFT
    jp z, .check_left_end_8
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
    .check_left_end_8:
    .check_right_9:
    ld a, [wReleasedKeys]
    and a, PADF_RIGHT
    jp z, .check_right_end_9
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
    .check_right_end_9:
    .check_up_10:
    ld a, [wReleasedKeys]
    and a, PADF_UP
    jp z, .check_up_end_10
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
    .check_up_end_10:
    .check_down_11:
    ld a, [wReleasedKeys]
    and a, PADF_DOWN
    jp z, .check_down_end_11
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
    .check_down_end_11:
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
    ld a, HIGH(wShadowOAM)
    call hOAMDMA
    ld a, 1
    ld [wVBlankFlag], a
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

    BgMap:
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    db $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00
    BgMapEnd:

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
    wAnim_player_left_Current: db
    wAnim_player_right_Current: db


