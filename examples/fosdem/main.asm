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
    ld de, player_left
    ld hl, $8000
    ld bc, player_leftEnd - player_left
    call Memcopy
    ld de, player_right
    ld hl, $8400
    ld bc, player_rightEnd - player_right
    call Memcopy
    call InitOamDma
    ld a, 0
    ld b, 160
    ld hl, wShadowOAM
    .clear_oam_8:
    ld [hli], a
    dec b
    jp nz, .clear_oam_8
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
    ld [rBGP], a
    ld [rOBP0], a
    ld [rOBP1], a
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
    dw $744F
    dw $7C4F
    dw $3B3F
    dw $090F
    dw $1917
    dw $3F21
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
    dw $332F
    dw $3C2F
    dw $1B1F
    dw $090F
    dw $1D13
    dw $3F21
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
    dw $744F
    dw $7C4F
    dw $3B3F
    dw $090F
    dw $0E09
    dw $1F10
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1E17
    dw $0A0F
    dw $1A1F
    dw $3C27
    dw $3F27
    dw $1C1F
    dw $090F
    dw $1F10
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
    dw $302F
    dw $382F
    dw $1B1F
    dw $090F
    dw $1D13
    dw $3F21
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
    dw $744F
    dw $7C4F
    dw $3B3F
    dw $090F
    dw $1D13
    dw $3F21
    dw $0000
    dw $0707
    dw $0F08
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F10
    dw $1F18
    dw $3F27
    dw $3827
    dw $1C1F
    dw $090F
    dw $1F10
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
    dw $744F
    dw $7C4F
    dw $3B3F
    dw $090F
    dw $0E09
    dw $1F10
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
    dw $1F17
    dw $1C17
    dw $1C1F
    dw $3927
    dw $3F21
    dw $1E12
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
    dw $0203
    dw $0302
    dw $0704
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
    dw $3F27
    dw $3827
    dw $1C1F
    dw $3F27
    dw $3D23
    dw $1E12
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
    dw $0203
    dw $0302
    dw $0704
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
    dw $C4BF
    dw $EC9F
    dw $FE9F
    dw $737F
    dw $7C44
    dw $3E22
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
    dw $1E13
    dw $0E0B
    dw $0F0E
    dw $0706
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
    dw $E0BF
    dw $E8BF
    dw $447F
    dw $637F
    dw $7C44
    dw $3E22
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
    dw $1E13
    dw $0E0B
    dw $0F0E
    dw $0706
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
    dw $744F
    dw $7C4F
    dw $3B3F
    dw $090F
    dw $1D13
    dw $3F21
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1B17
    dw $0A0F
    dw $0A0F
    dw $080F
    dw $1F17
    dw $1C17
    dw $0C0F
    dw $0303
    dw $0203
    dw $0302
    dw $0704
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
    dw $1E13
    dw $0E0B
    dw $0F0E
    dw $0706
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
    dw $302F
    dw $382F
    dw $1B1F
    dw $090F
    dw $0E09
    dw $1F10
    dw $0707
    dw $0F08
    dw $1F10
    dw $3F20
    dw $3F20
    dw $1817
    dw $7E7F
    dw $EABF
    dw $F49F
    dw $7B7F
    dw $111F
    dw $1C1F
    dw $0B0F
    dw $080F
    dw $0605
    dw $0F08
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
    dw $3739
    dw $1F19
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
    dw $E89F
    dw $F89F
    dw $777F
    dw $1D13
    dw $3F21
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
    dw $3C27
    dw $3C27
    dw $1C1F
    dw $0B0F
    dw $090F
    dw $0E09
    dw $1F10
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
    dw $161F
    dw $233F
    dw $7E46
    dw $3E22
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
    dw $213F
    dw $7B47
    dw $3E22
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
    dw $0F0B
    dw $0E0B
    dw $1C1F
    dw $3927
    dw $3F21
    dw $1E12
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
    dw $3827
    dw $3827
    dw $1F1F
    dw $080F
    dw $0B0C
    dw $1F11
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
    dw $744F
    dw $7C4F
    dw $3B3F
    dw $090F
    dw $1D13
    dw $3F21
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
    dw $7F53
    dw $3F31
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
    dw $744F
    dw $7C4F
    dw $3B3F
    dw $1D13
    dw $3F21
    dw $0303
    dw $0704
    dw $0F08
    dw $1F10
    dw $1F11
    dw $6F6B
    dw $FD9B
    dw $E59F
    dw $627F
    dw $313F
    dw $7C4F
    dw $7A77
    dw $5877
    dw $5F77
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
    dw $2EF2
    dw $3EF2
    dw $DCFC
    dw $90F0
    dw $30D0
    dw $F808
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $F8E8
    dw $50F0
    dw $58F8
    dw $3CE4
    dw $FCE4
    dw $38F8
    dw $90F0
    dw $F808
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
    dw $2EF2
    dw $3EF2
    dw $DCFC
    dw $90F0
    dw $B8C8
    dw $FC84
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
    dw $CCF4
    dw $3CF4
    dw $D8F8
    dw $90F0
    dw $B8C8
    dw $FC84
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F818
    dw $FCE4
    dw $1CE4
    dw $38F8
    dw $90F0
    dw $F808
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
    dw $2EF2
    dw $3EF2
    dw $DCFC
    dw $90F0
    dw $7090
    dw $F808
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
    dw $1CE4
    dw $1CF4
    dw $D8F8
    dw $90F0
    dw $B8C8
    dw $FC84
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
    dw $2EF2
    dw $3EF2
    dw $DCFC
    dw $90F0
    dw $B8C8
    dw $FC84
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
    dw $23FD
    dw $37F9
    dw $7FF9
    dw $CEFE
    dw $3E22
    dw $7C44
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
    dw $78C8
    dw $70D0
    dw $F070
    dw $E060
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
    dw $07FD
    dw $17FD
    dw $22FE
    dw $C6FE
    dw $3E22
    dw $7C44
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
    dw $78C8
    dw $70D0
    dw $F070
    dw $E060
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
    dw $F8E8
    dw $38E8
    dw $38F8
    dw $9CE4
    dw $FC84
    dw $7848
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
    dw $40C0
    dw $C040
    dw $E020
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
    dw $FCE4
    dw $1CE4
    dw $38F8
    dw $FCE4
    dw $BCC4
    dw $7848
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
    dw $40C0
    dw $C040
    dw $E020
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $7888
    dw $F8E8
    dw $50F0
    dw $58F8
    dw $3CE4
    dw $FCE4
    dw $38F8
    dw $30F0
    dw $D0F0
    dw $90F0
    dw $7090
    dw $F808
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
    dw $78C8
    dw $70D0
    dw $F070
    dw $E060
    dw $E0E0
    dw $F010
    dw $F808
    dw $FC04
    dw $FC04
    dw $D8E8
    dw $50F0
    dw $50F0
    dw $10F0
    dw $F8E8
    dw $38E8
    dw $30F0
    dw $C0C0
    dw $40C0
    dw $C040
    dw $E020
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F818
    dw $FCE4
    dw $1CE4
    dw $38F8
    dw $D0F0
    dw $90F0
    dw $B8C8
    dw $FC84
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
    dw $97F9
    dw $3FF9
    dw $DEFE
    dw $F0B0
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
    dw $EEF2
    dw $9EF2
    dw $FCFC
    dw $10F0
    dw $60A0
    dw $F010
    dw $0000
    dw $E0E0
    dw $F010
    dw $F808
    dw $F808
    dw $F808
    dw $F808
    dw $F818
    dw $FC24
    dw $FCE4
    dw $18F8
    dw $60E0
    dw $A0E0
    dw $20E0
    dw $7090
    dw $F808
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
    dw $B0D0
    dw $F090
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
    dw $F0D0
    dw $70D0
    dw $38F8
    dw $9CE4
    dw $FC84
    dw $7848
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
    dw $1CE4
    dw $1CE4
    dw $F8F8
    dw $10F0
    dw $D030
    dw $F888
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
    dw $68F8
    dw $C4FC
    dw $7E62
    dw $7C44
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
    dw $84FC
    dw $DEE2
    dw $7C44
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
    dw $2EF2
    dw $3EF2
    dw $DCFC
    dw $90F0
    dw $7090
    dw $F808
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
    dw $FE92
    dw $FC1C
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
    dw $2EF2
    dw $3EF2
    dw $DCFC
    dw $7090
    dw $F808
    dw $F0F0
    dw $F808
    dw $FC04
    dw $FC04
    dw $FC04
    dw $7EE6
    dw $5FE9
    dw $4FF9
    dw $92FE
    dw $E4FC
    dw $18F8
    dw $3EF2
    dw $1EEE
    dw $DAEE
    dw $7A6E
    dw $3C3C
    player_rightEnd:

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


