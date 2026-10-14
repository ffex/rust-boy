; Original two-note fixture for rust-boy, MIT licensed.
INCLUDE "hUGE.inc"
SECTION "Song Data", ROM0
DemoSong::
    db 2
    dw order_count
    dw order1, order2, order3, order4
    dw duty_instruments, wave_instruments, noise_instruments
    dw routines, waves
order_count: db 2
order1: dw pulse_pattern
order2: dw pulse_pattern
order3: dw wave_pattern
order4: dw noise_pattern
pulse_pattern:
    dn C_5, 1, 0
    dn E_5, 1, 0
    REPT 62
        dn ___, 0, 0
    ENDR
wave_pattern:
    dn C_4, 1, 0
    dn E_4, 1, 0
    REPT 62
        dn ___, 0, 0
    ENDR
noise_pattern:
    dn C_3, 1, 0
    dn D_3, 1, 0
    REPT 62
        dn ___, 0, 0
    ENDR
duty_instruments: db 0, $80, $a2
    dw 0
    db $80
wave_instruments: db 0, $20, 0
    dw 0
    db $80
noise_instruments: db $a2
    dw 0
    db 0, 0, 0
routines:
    REPT 16
        dw noop
    ENDR
noop: ret
waves: db $01, $23, $45, $67, $89, $ab, $cd, $ef, $fe, $dc, $ba, $98, $76, $54, $32, $10
