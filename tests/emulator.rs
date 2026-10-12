//! Headless-emulator tests: build a ROM with RGBDS, run it on `support::gameboy` with
//! scripted input, and check its memory and registers.
//!
//! They need RGBDS: they run when `RGBDS_LINK_CHECK` is set (CI sets it) and return early
//! otherwise. The emulator's own checks against Blargg's test ROMs run when
//! `GB_TEST_ROMS` names a directory that holds them (`scripts/fetch-test-roms.sh`; CI
//! sets it too).

mod support;

use std::collections::BTreeSet;

use rust_boy::asm::R16Stack;
use rust_boy::prelude::*;
use support::gameboy::{Button, GameBoy, Mode};
use support::rom::{Rom, rgbds_enabled};

// ----- The emulator itself -----

/// Run Blargg test ROM `path` (under `GB_TEST_ROMS`) until it prints its verdict on the
/// serial port, at most `max_frames` frames
fn blargg(path: &str, max_frames: u64) {
    let Some(dir) = std::env::var_os("GB_TEST_ROMS") else {
        eprintln!("skipped: set GB_TEST_ROMS to the directory of scripts/fetch-test-roms.sh");
        return;
    };
    let file = std::path::Path::new(&dir).join(path);
    let rom = std::fs::read(&file).unwrap_or_else(|error| panic!("{}: {}", file.display(), error));
    let mut gb = GameBoy::new(rom);
    for _ in 0..max_frames {
        gb.run_frames(1);
        let output = String::from_utf8_lossy(gb.serial_output());
        if output.contains("Passed") {
            return;
        }
        if output.contains("Failed") {
            break;
        }
    }
    panic!(
        "{}: {}",
        path,
        String::from_utf8_lossy(gb.serial_output()).trim()
    );
}

macro_rules! blargg_tests {
    ($($name:ident: $path:expr, $frames:expr;)*) => {
        $(
            #[test]
            fn $name() {
                blargg($path, $frames);
            }
        )*
    };
}

blargg_tests! {
    blargg_cpu_instrs_01_special: "cpu_instrs/individual/01-special.gb", 600;
    blargg_cpu_instrs_02_interrupts: "cpu_instrs/individual/02-interrupts.gb", 600;
    blargg_cpu_instrs_03_op_sp_hl: "cpu_instrs/individual/03-op sp,hl.gb", 600;
    blargg_cpu_instrs_04_op_r_imm: "cpu_instrs/individual/04-op r,imm.gb", 600;
    blargg_cpu_instrs_05_op_rp: "cpu_instrs/individual/05-op rp.gb", 600;
    blargg_cpu_instrs_06_ld_r_r: "cpu_instrs/individual/06-ld r,r.gb", 600;
    blargg_cpu_instrs_07_jr_jp_call_ret_rst: "cpu_instrs/individual/07-jr,jp,call,ret,rst.gb", 600;
    blargg_cpu_instrs_08_misc_instrs: "cpu_instrs/individual/08-misc instrs.gb", 600;
    blargg_cpu_instrs_09_op_r_r: "cpu_instrs/individual/09-op r,r.gb", 1200;
    blargg_cpu_instrs_10_bit_ops: "cpu_instrs/individual/10-bit ops.gb", 1200;
    blargg_cpu_instrs_11_op_a_hl: "cpu_instrs/individual/11-op a,(hl).gb", 1800;
    blargg_instr_timing: "instr_timing/instr_timing.gb", 600;
    blargg_mem_timing_01_read: "mem_timing/individual/01-read_timing.gb", 600;
    blargg_mem_timing_02_write: "mem_timing/individual/02-write_timing.gb", 600;
    blargg_mem_timing_03_modify: "mem_timing/individual/03-modify_timing.gb", 600;
}

/// A program that counts the frames (VBlanks) in WRAM, keeps the buttons it reads in
/// HRAM and leaves `$42` in `b`; or, `idle`, the same program stuck in a loop before it
/// does any of that
fn frame_counter_rom(idle: bool) -> Rom {
    let asm = FRAME_COUNTER_ASM.replace(
        "EntryPoint:\n",
        if idle {
            "EntryPoint:\n.idle:\n    jr .idle\n"
        } else {
            "EntryPoint:\n"
        },
    );
    Rom::build(&asm, &[])
}

const FRAME_COUNTER_ASM: &str = r#"INCLUDE "hardware.inc"

SECTION "Header", ROM0[$100]
    jp EntryPoint
    ds $150 - @, 0

SECTION "Main", ROM0
EntryPoint:
    xor a, a
    ld [wFrames], a
    ld b, $42
.loop:
.wait_not_vblank:
    ldh a, [rLY]
    cp a, 144
    jr z, .wait_not_vblank
.wait_vblank:
    ldh a, [rLY]
    cp a, 144
    jr nz, .wait_vblank
    ld a, [wFrames]
    inc a
    ld [wFrames], a
    ld a, P1F_GET_BTN
    ldh [rP1], a
    ldh a, [rP1]
    ldh [hButtons], a
    jr .loop

SECTION "Frames", WRAM0
wFrames: ds 1

SECTION "Buttons", HRAM
hButtons: ds 1
"#;

/// The helper builds the frame counter with RGBDS, runs it with scripted input and reads
/// the WRAM counter, the HRAM buttons and the registers
#[test]
fn emulator_runs_a_rom_frame_by_frame_with_input() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = frame_counter_rom(false).boot();
    gb.run_frames(10);
    assert_eq!(gb.read_symbol("wFrames"), 10, "one VBlank per frame");
    assert_eq!(gb.registers().b, 0x42);
    assert_eq!(gb.read_symbol("hButtons"), 0xDF, "no button pressed");

    gb.run_script(&[(2, &[Button::A]), (1, &[])]);
    assert_eq!(gb.read_symbol("wFrames"), 13);
    assert_eq!(gb.read_symbol("hButtons"), 0xDF, "A released again");
    gb.press(Button::A);
    gb.run_frames(1);
    assert_eq!(
        gb.read_symbol("hButtons"),
        0xDE,
        "A pressed: its bit reads 0"
    );
    let pc = gb.registers().pc;
    assert!(
        (gb.symbol("EntryPoint")..gb.symbol("EntryPoint") + 0x30).contains(&pc),
        "the CPU stays in the loop: pc = ${:04X}",
        pc
    );
}

/// The same checks fail on a ROM that does nothing: the counter, the buttons and `b` keep
/// the values of power-on
#[test]
fn emulator_sees_a_rom_that_does_nothing() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = frame_counter_rom(true).boot();
    gb.run_script(&[(10, &[Button::A])]);
    assert_eq!(gb.read_symbol("wFrames"), 0);
    assert_eq!(gb.read_symbol("hButtons"), 0);
    assert_eq!(gb.registers().b, 0x00, "what the boot ROM leaves");
    assert_eq!(gb.registers().pc, gb.symbol("EntryPoint.idle"));
}

// ----- Interrupts, halt, ei / di -----

/// A program with counting handlers for the VBlank, STAT and timer interrupts, whose
/// entry point runs `main`, then idles. Each handler adds 1 to its counter, `wVBlanks`,
/// `wStats` or `wTimers`; the VBlank one also keeps `a` as it found it in `wSeenA`, and
/// the STAT one `LY` in `wStatLY`. `wMark` and `wAfterHalt` are for `main`.
fn interrupt_rom(main: &str) -> Rom {
    let asm = INTERRUPT_ASM.replace("    ; main\n", main);
    Rom::build(&asm, &[])
}

const INTERRUPT_ASM: &str = r#"INCLUDE "hardware.inc"

SECTION "VBlank interrupt", ROM0[$40]
    ld [wSeenA], a
    push af
    jp VBlankHandler

SECTION "STAT interrupt", ROM0[$48]
    push af
    jp StatHandler

SECTION "Timer interrupt", ROM0[$50]
    push af
    jp TimerHandler

SECTION "Header", ROM0[$100]
    jp EntryPoint
    ds $150 - @, 0

SECTION "Main", ROM0
VBlankHandler:
    ld a, [wVBlanks]
    inc a
    ld [wVBlanks], a
    pop af
    reti

StatHandler:
    ldh a, [rLY]
    ld [wStatLY], a
    ld a, [wStats]
    inc a
    ld [wStats], a
    pop af
    reti

TimerHandler:
    ld a, [wTimers]
    inc a
    ld [wTimers], a
    pop af
    reti

EntryPoint:
    ; main
.idle:
    jr .idle

SECTION "Counters", WRAM0
wVBlanks: ds 1
wStats: ds 1
wTimers: ds 1
wSeenA: ds 1
wStatLY: ds 1
wMark: ds 1
wAfterHalt: ds 1
"#;

/// A `halt` main loop woken by the VBlank interrupt: one interrupt a frame, and the code
/// after the `halt` runs once each
#[test]
fn interrupts_vblank_wakes_a_halt_loop() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = interrupt_rom(
        "    ld a, IEF_VBLANK
    ldh [rIE], a
    xor a, a
    ldh [rIF], a
    ei
.loop:
    halt
    ld a, [wAfterHalt]
    inc a
    ld [wAfterHalt], a
    jr .loop
",
    )
    .boot();
    gb.run_frames(10);
    assert_eq!(
        gb.read_symbol("wVBlanks"),
        10,
        "one VBlank interrupt a frame"
    );
    assert_eq!(
        gb.read_symbol("wAfterHalt"),
        10,
        "the code after halt ran each time"
    );
    assert!(gb.registers().halted, "waiting in the halt");
}

/// `ei` then `halt` with an interrupt already pending (IF = VBlank, as at power-on): the
/// interrupt is taken at once and returns to the `halt`, which then waits for the next
/// one (Pan Docs, "halt bug")
#[test]
fn interrupts_ei_then_halt_with_one_pending_returns_to_the_halt() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = interrupt_rom(
        "    ld a, IEF_VBLANK
    ldh [rIE], a
    ldh [rIF], a
    ei
    halt
    ld a, [wVBlanks]
    ld [wAfterHalt], a
    ld a, 1
    ld [wMark], a
",
    )
    .boot();
    gb.run_frames(3);
    assert_eq!(gb.read_symbol("wMark"), 1, "the code after the halt ran");
    assert_eq!(
        gb.read_symbol("wAfterHalt"),
        2,
        "the pending interrupt, then the halt again until the frame's VBlank"
    );
    assert_eq!(
        gb.read_symbol("wVBlanks"),
        4,
        "then one a frame, in the idle loop"
    );
    assert_eq!(gb.registers().pc, gb.symbol("EntryPoint.idle"));
}

/// `halt` with IME clear and an interrupt pending: the halt bug, the byte after the
/// `halt` is read twice (`inc b` runs twice), and no interrupt is taken
#[test]
fn interrupts_halt_with_ime_clear_and_one_pending_is_the_halt_bug() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = interrupt_rom(
        "    di
    ld a, IEF_VBLANK
    ldh [rIE], a
    ldh [rIF], a
    ld b, 0
    halt
    inc b
    ld a, b
    ld [wMark], a
",
    )
    .boot();
    gb.run_frames(2);
    assert_eq!(gb.read_symbol("wMark"), 2, "inc b ran twice");
    assert_eq!(gb.read_symbol("wVBlanks"), 0);
}

/// `ei` right before `di`: IME is never set, so the pending interrupt is never taken
#[test]
fn interrupts_ei_then_di_takes_none() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = interrupt_rom(
        "    ld a, IEF_VBLANK
    ldh [rIE], a
    ldh [rIF], a
    ei
    di
",
    )
    .boot();
    gb.run_frames(5);
    assert_eq!(gb.read_symbol("wVBlanks"), 0);
    assert!(!gb.registers().ime);
}

/// `ei` takes effect after the next instruction: with an interrupt pending, the
/// instruction after `ei` runs before the handler (which sees its `a`)
#[test]
fn interrupts_ei_takes_effect_after_the_next_instruction() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = interrupt_rom(
        "    ld a, IEF_VBLANK
    ldh [rIE], a
    ldh [rIF], a
    ei
    ld a, $42
    di
",
    )
    .boot();
    gb.run_frames(2);
    assert_eq!(gb.read_symbol("wVBlanks"), 1, "taken once, then di");
    assert_eq!(gb.read_symbol("wSeenA"), 0x42, "after ld a, $42 ran");
}

/// The STAT interrupt on `LY` = `LYC`: once a frame, on that line
#[test]
fn interrupts_stat_on_lyc() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = interrupt_rom(
        "    ld a, 100
    ldh [rLYC], a
    ld a, STATF_LYC
    ldh [rSTAT], a
    ld a, IEF_STAT
    ldh [rIE], a
    xor a, a
    ldh [rIF], a
    ei
",
    )
    .boot();
    gb.run_frames(5);
    assert_eq!(gb.read_symbol("wStats"), 5);
    assert_eq!(gb.read_symbol("wStatLY"), 100);
}

/// The timer interrupt: at 4096 Hz (one `TIMA` step every 256 M-cycles) from `TMA` = $C0,
/// an overflow every 64 steps, 16384 M-cycles: 10 in 10 frames (175560 M-cycles)
#[test]
fn interrupts_timer_overflow() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = interrupt_rom(
        "    ld a, $C0
    ldh [rTMA], a
    ldh [rTIMA], a
    ld a, TACF_START | TACF_4KHZ
    ldh [rTAC], a
    ld a, IEF_TIMER
    ldh [rIE], a
    xor a, a
    ldh [rIF], a
    ei
",
    )
    .boot();
    gb.run_frames(10);
    assert_eq!(gb.read_symbol("wTimers"), 10);
}

/// The joypad interrupt comes when a line of `P1` goes from high to low: a press in a
/// selected group, or a group selected while one of its buttons is held; never with no
/// group selected. The program counts the interrupts in `wJoypads`, with `P1` as `wMark`
/// says: the main loop writes `[wMark]` to `P1` each frame (`P1F_GET_NONE`, then the
/// test changes it)
#[test]
fn interrupts_joypad_only_on_selected_lines() {
    if !rgbds_enabled() {
        return;
    }
    let asm = r#"INCLUDE "hardware.inc"

SECTION "Joypad interrupt", ROM0[$60]
    push af
    ld a, [wJoypads]
    inc a
    ld [wJoypads], a
    pop af
    reti

SECTION "Header", ROM0[$100]
    jp EntryPoint
    ds $150 - @, 0

SECTION "Main", ROM0
EntryPoint:
    xor a, a
    ld [wJoypads], a
    ld a, P1F_GET_NONE
    ld [wMark], a
    ld a, IEF_HILO
    ldh [rIE], a
    xor a, a
    ldh [rIF], a
    ei
.loop:
    ld a, [wMark]
    ldh [rP1], a
    jr .loop

SECTION "Counters", WRAM0
wJoypads: ds 1
wMark: ds 1
"#;
    let rom = Rom::build(asm, &[]);
    let mut gb = rom.boot();
    let mark = gb.symbol("wMark");
    gb.run_frames(1);
    // No group selected: a press pulls no line low
    gb.run_script(&[(2, &[Button::A, Button::Left])]);
    assert_eq!(gb.read_symbol("wJoypads"), 0, "no group selected");
    // The buttons selected: A is one press; Left, a direction, none
    gb.write_ram(mark, 0x10); // P1F_GET_BTN
    gb.run_frames(1);
    gb.run_script(&[(2, &[Button::A]), (2, &[Button::Left])]);
    assert_eq!(gb.read_symbol("wJoypads"), 1, "A, in the selected group");
    // Selecting the directions while Left is held pulls its line low
    gb.press(Button::Left);
    gb.run_frames(1);
    let before = gb.read_symbol("wJoypads");
    gb.write_ram(mark, 0x20); // P1F_GET_DPAD
    gb.run_frames(1);
    assert_eq!(
        gb.read_symbol("wJoypads"),
        before + 1,
        "selected while held"
    );
}

/// The first line after the LCD is turned on has no OAM scan: OAM can be written at once
#[test]
fn lcd_on_first_line_has_no_oam_scan() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = interrupt_rom(
        ".wait_vblank:
    ldh a, [rLY]
    cp a, 144
    jr nz, .wait_vblank
    xor a, a
    ldh [rLCDC], a
    ld a, LCDCF_ON | LCDCF_OBJON
    ldh [rLCDC], a
    ld a, $42
    ld [_OAMRAM], a
    ldh a, [rSTAT]
    ld [wMark], a
",
    )
    .boot();
    gb.run_frames(2);
    assert_eq!(gb.oam_entry(0)[0], 0x42, "written in line 0");
    assert_eq!(gb.read_symbol("wMark") & 0x03, 0, "STAT reads mode 0");
    assert!(gb.dropped_oam_writes().is_empty());
}

/// The usual OAM DMA routine, run from HRAM, copies a shadow OAM in WRAM. A DMA written in
/// M-cycle `w` keeps the CPU off WRAM until `w + 161`: a read of WRAM in `w + 161` gives
/// $FF, one in `w + 162` the byte (each routine below counts its cycles from the `ldh`)
#[test]
fn oam_dma_from_hram() {
    if !rgbds_enabled() {
        return;
    }
    let asm = r#"INCLUDE "hardware.inc"

SECTION "Header", ROM0[$100]
    jp EntryPoint
    ds $150 - @, 0

SECTION "Main", ROM0
EntryPoint:
    ; the shadow OAM: byte i = i
    ld hl, wShadowOam
    xor a, a
.fill:
    ld [hli], a
    inc a
    cp a, OAM_COUNT * sizeof_OAM_ATTRS
    jr nz, .fill
    ; the routines to HRAM
    ld hl, Routines
    ld de, hLastBusy
    ld b, RoutinesEnd - Routines
.copy:
    ld a, [hli]
    ld [de], a
    inc de
    dec b
    jr nz, .copy
    ld hl, wShadowOam + 5
    ld a, HIGH(wShadowOam)
    call hLastBusy
    ldh [hReadLastBusy], a
    ld a, HIGH(wShadowOam)
    call hFirstFree
    ldh [hReadFirstFree], a
.idle:
    jr .idle

; Each: the DMA, then a read of [hl] in M-cycle w + 161 (w + 162 for the second); it
; returns what it read in a
Routines:
LastBusy:
    ldh [rDMA], a       ; written in w
    ld a, 39            ; w + 1, w + 2
.wait:
    dec a               ; 39 times dec + jr: w + 3 to w + 157
    jr nz, .wait
    nop                 ; w + 158
    nop                 ; w + 159
    ld a, [hl]          ; read in w + 161
    ret
FirstFree:
    ldh [rDMA], a
    ld a, 39
.wait:
    dec a
    jr nz, .wait
    nop
    nop
    nop
    ld a, [hl]          ; read in w + 162
    ret
RoutinesEnd:

SECTION "Shadow OAM", WRAM0, ALIGN[8]
wShadowOam: ds 160

SECTION "DMA routines", HRAM
hLastBusy: ds FirstFree - LastBusy
hFirstFree: ds RoutinesEnd - FirstFree
hReadLastBusy: ds 1
hReadFirstFree: ds 1
"#;
    let mut gb = Rom::build(asm, &[]).boot();
    gb.run_frames(2);
    let expected: Vec<u8> = (0..160).collect();
    assert_eq!(gb.oam().to_vec(), expected, "OAM is the shadow OAM");
    assert_eq!(
        gb.read_symbol("hReadLastBusy"),
        0xFF,
        "w + 161: still the DMA's"
    );
    assert_eq!(gb.read_symbol("hReadFirstFree"), 5, "w + 162: free again");
    assert_eq!(
        gb.dma_conflicts(),
        1,
        "that one read; the rest ran from HRAM"
    );
    assert_eq!(gb.registers().pc, gb.symbol("EntryPoint.idle"));
}

// ----- The examples -----

/// Every CPU write to OAM with the LCD on happened in VBlank and none was dropped: the
/// hand-written examples (`unbricked`, `basic_usage`) write OAM from a main loop that fits
/// in VBlank, so they do not run into [B12]
///
/// [B12]: ../CONTEXT.md#b12
fn assert_oam_written_in_vblank(gb: &GameBoy) {
    let outside: Vec<_> = gb
        .oam_writes()
        .iter()
        .filter(|write| write.lcd_on && (write.mode != Mode::VBlank || write.dropped))
        .collect();
    assert!(
        outside.is_empty(),
        "{} OAM writes outside VBlank, the first: {:?}",
        outside.len(),
        outside.first()
    );
    assert_eq!(
        gb.blocked_accesses(),
        (0, 0, 0),
        "VRAM / OAM accesses the PPU blocked"
    );
}

/// The sprites reach OAM only through OAM DMAs of the shadow OAM, in VBlank ([B12]): the
/// CPU never writes OAM with the LCD on (the start-up clears the shadow OAM, not OAM),
/// every OAM DMA with the LCD on copies `wShadowOAM` and starts and ends in VBlank, one a
/// frame, and no access was blocked (no DMA conflict either: the DMA routine runs from HRAM)
///
/// [B12]: ../CONTEXT.md#b12
fn assert_sprites_shown_by_dma_in_vblank(gb: &GameBoy) {
    let written: Vec<_> = gb.oam_writes().iter().filter(|w| w.lcd_on).collect();
    assert!(
        written.is_empty(),
        "{} CPU writes to OAM with the LCD on, the first: {:?}",
        written.len(),
        written.first()
    );
    let shadow = gb.symbol("wShadowOAM");
    let dmas: Vec<_> = gb.oam_dmas().iter().filter(|dma| dma.lcd_on).collect();
    let frames = gb.frame();
    assert!(
        dmas.len() as u64 + 3 >= frames,
        "{} OAM DMAs with the LCD on in {} frames",
        dmas.len(),
        frames
    );
    for dma in &dmas {
        assert_eq!(dma.source, shadow, "a DMA of the shadow OAM: {:?}", dma);
        let in_vblank = |(ly, mode): (u8, Mode)| (144..=153).contains(&ly) && mode == Mode::VBlank;
        assert!(
            in_vblank(dma.first) && dma.last.is_some_and(in_vblank),
            "a DMA outside VBlank: {:?}",
            dma
        );
    }
    assert_eq!(gb.dma_conflicts(), 0, "the CPU left HRAM during a DMA");
    assert_eq!(
        gb.blocked_accesses(),
        (0, 0, 0),
        "VRAM / OAM accesses the PPU blocked"
    );
}

/// The ball (OAM entry 1) of an Unbricked example moves one pixel along each axis every
/// frame and bounces off the walls; the paddle (OAM entry 0) follows the joypad and stops
/// at its left limit, OAM X 16
fn check_unbricked(bin: &str) -> GameBoy {
    let rom = Rom::example(bin);
    let mut gb = rom.boot();
    gb.run_frames(10);
    let paddle = gb.oam_entry(0);
    assert_eq!((paddle[0], paddle[1]), (144, 24), "the paddle at its start");

    let mut ball = gb.oam_entry(1);
    let mut x_directions = BTreeSet::new();
    let mut y_directions = BTreeSet::new();
    for _ in 0..300 {
        gb.run_frames(1);
        let next = gb.oam_entry(1);
        let dy = i16::from(next[0]) - i16::from(ball[0]);
        let dx = i16::from(next[1]) - i16::from(ball[1]);
        assert!(
            dx.abs() == 1 && dy.abs() == 1,
            "frame {}: the ball moved from {:?} to {:?}",
            gb.frame(),
            ball,
            next
        );
        assert!((8..=168).contains(&next[1]) && (16..=160).contains(&next[0]));
        x_directions.insert(dx);
        y_directions.insert(dy);
        ball = next;
    }
    assert_eq!(x_directions.len(), 2, "the ball bounced left and right");
    assert_eq!(y_directions.len(), 2, "the ball bounced up and down");

    gb.run_script(&[(30, &[Button::Left])]);
    assert_eq!(gb.oam_entry(0)[1], 16, "the paddle stops at its left limit");
    gb.run_script(&[(20, &[Button::Right])]);
    let x = gb.oam_entry(0)[1];
    assert!((35..=36).contains(&x), "the paddle moved right: X {}", x);
    assert_eq!(gb.oam_entry(0)[0], 144, "the paddle stays on its row");
    gb
}

/// The bricks left on the map: tiles `BRICK_LEFT` ($05) in the `$9800` tilemap
fn bricks(gb: &GameBoy) -> usize {
    gb.vram()[0x1800..0x1C00]
        .iter()
        .filter(|&&tile| tile == 0x05)
        .count()
}

#[test]
fn basic_usage_turns_the_lcd_on_and_loops_on_vblank() {
    if !rgbds_enabled() {
        return;
    }
    let rom = Rom::example("basic_usage");
    let mut gb = rom.boot();
    gb.run_frames(30);
    let registers = gb.registers();
    assert_eq!(
        registers.bc(),
        160,
        "ld bc, 160 ran (the boot ROM leaves $0013)"
    );
    assert!(
        (gb.symbol("MainLoop")..gb.symbol("TileData")).contains(&registers.pc),
        "the CPU loops in MainLoop / WaitVBlank: pc = ${:04X}",
        registers.pc
    );
    assert_eq!(
        gb.read(0xFF40),
        0x91,
        "LCDC: LCD on, tiles at $8000, background on"
    );
    assert_oam_written_in_vblank(&gb);
}

#[test]
fn unbricked_ball_bounces_paddle_moves_bricks_break() {
    if !rgbds_enabled() {
        return;
    }
    let gb = check_unbricked("unbricked");
    assert_oam_written_in_vblank(&gb);
    assert!(bricks(&gb) < 32, "bricks broke: {} left", bricks(&gb));
    assert!(gb.read_symbol("wScore") > 0, "the score went up");
}

#[test]
fn unbricked_std_ball_bounces_paddle_moves() {
    if !rgbds_enabled() {
        return;
    }
    let gb = check_unbricked("unbricked_std");
    assert_sprites_shown_by_dma_in_vblank(&gb);
}

#[test]
fn unbricked_rustboy_ball_bounces_paddle_moves_bricks_break() {
    if !rgbds_enabled() {
        return;
    }
    let gb = check_unbricked("unbricked_rustboy");
    assert_sprites_shown_by_dma_in_vblank(&gb);
    assert!(bricks(&gb) < 32, "bricks broke: {} left", bricks(&gb));
    // The momentum variables are what the bounces change
    let momentum_x = gb.read_symbol("wBallMomentumX");
    let momentum_y = gb.read_symbol("wBallMomentumY");
    assert!(matches!(momentum_x, 0x01 | 0xFF) && matches!(momentum_y, 0x01 | 0xFF));
}

#[test]
fn fosdem_player_walks_as_one_block_with_its_animation() {
    if !rgbds_enabled() {
        return;
    }
    let rom = Rom::example("fosdem");
    let mut gb = rom.boot();
    gb.run_frames(10);
    let start = (gb.oam_entry(0), gb.oam_entry(1));
    assert_eq!(
        (start.0[0], start.0[1]),
        (88, 88),
        "the left half at (80, 72) + (8, 16)"
    );
    assert_eq!(
        (start.1[0], start.1[1]),
        (88, 96),
        "the right half 8 pixels further right"
    );
    gb.run_frames(30);
    assert_eq!(
        (gb.oam_entry(0), gb.oam_entry(1)),
        start,
        "no input, no animation"
    );

    let mut tiles = BTreeSet::new();
    for _ in 0..30 {
        gb.run_script(&[(1, &[Button::Right])]);
        tiles.insert(gb.oam_entry(0)[2]);
        let (left, right) = (gb.oam_entry(0), gb.oam_entry(1));
        assert_eq!(right[1], left[1] + 8, "the halves move together");
        assert_eq!(right[2], left[2] + 64, "the right half shows its own tiles");
    }
    let x = gb.oam_entry(0)[1];
    assert!((88 + 29..=88 + 30).contains(&x), "walked right: X {}", x);
    assert!(
        tiles.len() >= 3,
        "the walk-right animation played: tiles {:?}",
        tiles
    );

    gb.run_script(&[(150, &[Button::Left])]);
    assert_eq!(
        gb.oam_entry(0)[1],
        1,
        "the leading half stops on the left limit, 1"
    );
    assert_eq!(gb.oam_entry(1)[1], 9);
    gb.run_script(&[(100, &[Button::Up])]);
    assert_eq!(gb.oam_entry(0)[0], 1, "the top limit, 1");
    assert_sprites_shown_by_dma_in_vblank(&gb);
}

#[test]
fn coin_anim_a_starts_the_animation_b_stops_it() {
    if !rgbds_enabled() {
        return;
    }
    let rom = Rom::example("coin-anim");
    let mut gb = rom.boot();
    gb.run_frames(60);
    assert_eq!(
        gb.oam_entry(0),
        [88, 88, 0, 0],
        "the coin at (80, 72), frame 0"
    );
    assert_eq!(
        gb.read_symbol("wAnim_Coin_Current"),
        0xFF,
        "animation disabled"
    );

    gb.run_script(&[(2, &[Button::A])]);
    let mut tiles = Vec::new();
    for _ in 0..8 * 8 {
        gb.run_frames(1);
        tiles.push(gb.oam_entry(0)[2]);
    }
    let seen: BTreeSet<u8> = tiles.iter().copied().collect();
    assert_eq!(
        seen,
        (0..=6).collect(),
        "the 7 frames of the coin, then again"
    );
    // One frame every 8 (the animation delay), 0 to 6, then back to 0
    for pair in tiles.windows(2) {
        assert!(
            pair[1] == pair[0] || pair[1] == (pair[0] + 1) % 7,
            "{:?}",
            tiles
        );
    }

    gb.run_script(&[(2, &[Button::B])]);
    let stopped = gb.oam_entry(0)[2];
    gb.run_frames(40);
    assert_eq!(gb.oam_entry(0)[2], stopped, "B stopped the animation");
    assert_sprites_shown_by_dma_in_vblank(&gb);
}

// ----- The engine's frame model and interrupts -----

/// Code that adds 1 to the `u8` variable `var`
fn count(var: &str) -> Block {
    let mut code = Block::new();
    code.ld_a_addr_def(var).inc(R8::A).ld_addr_def_a(var);
    code
}

/// Code that stores `LY` in the variable `var`
fn store_ly(var: &str) -> Block {
    let mut code = Block::new();
    code.ldh(R8::A, Mem::addr(hw::LY)).ld_addr_def_a(var);
    code
}

/// Build `gb` into a ROM
fn engine_rom(gb: &RustBoy) -> Rom {
    Rom::build(&gb.build().expect("the program builds"), &[])
}

/// The share of `samples` moments, spread over `frames` frames, at which the CPU is in a
/// `halt`
fn halted_share(gb: &mut GameBoy, frames: u64, samples: u64) -> f64 {
    let step = frames * support::gameboy::FRAME_MCYCLES / samples;
    let mut halted = 0;
    for i in 0..samples {
        // An odd offset, so the samples do not fall on one point of the frame
        gb.run_until_cycle(gb.cycles() + step + i % 7);
        if gb.registers().halted {
            halted += 1;
        }
    }
    halted as f64 / samples as f64
}

/// The main loop waits for each frame asleep, woken by the VBlank interrupt: the VBlank
/// handler runs once a frame, then the main loop once, at the start of VBlank (line 144),
/// and the CPU is in `halt` most of the time. No handler runs before the first VBlank after
/// the start-up (the requests made during the start-up are dropped before `ei`).
#[test]
fn engine_vblank_handler_and_main_loop_run_once_a_frame() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = RustBoy::new();
    gb.vars.create_u8("wVBlanks", 0);
    gb.vars.create_u8("wLoops", 0);
    gb.vars.create_u8("wLoopLY", 0);
    gb.on_vblank(count("wVBlanks"));
    gb.add_to_main_loop(count("wLoops"));
    gb.add_to_main_loop(store_ly("wLoopLY"));
    let rom = engine_rom(&gb);
    let mut gb = rom.boot();

    assert!(gb.run_until_pc(gb.symbol("Main"), 10), "the start-up ends");
    assert_eq!(
        gb.read_symbol("wVBlanks"),
        0,
        "a VBlank requested during the start-up was not dropped before ei"
    );
    gb.run_frames(1);
    let (vblanks, loops) = (gb.read_symbol("wVBlanks"), gb.read_symbol("wLoops"));
    assert_eq!((vblanks, loops), (1, 1), "the first frame");

    gb.run_frames(100);
    assert_eq!(
        gb.read_symbol("wVBlanks"),
        vblanks + 100,
        "one VBlank handler a frame"
    );
    assert_eq!(
        gb.read_symbol("wLoops"),
        loops + 100,
        "one main loop a frame"
    );
    assert_eq!(
        gb.read_symbol("wLoopLY"),
        144,
        "the main loop runs at the start of VBlank"
    );

    let share = halted_share(&mut gb, 30, 600);
    assert!(
        share > 0.95,
        "the CPU sleeps in halt: {:.0}% of the time",
        share * 100.0
    );
    let pc = gb.registers().pc;
    gb.run_frames(1);
    assert!(gb.registers().halted);
    assert!(
        (gb.symbol("WaitFrame")..gb.symbol("WaitFrame.done")).contains(&gb.registers().pc),
        "asleep in WaitFrame: pc = ${:04X} (${:04X} before)",
        gb.registers().pc,
        pc
    );
}

/// WaitFrame checks its flag and sleeps with interrupts off, so it never sleeps through a
/// VBlank that comes right after another interrupt: a STAT interrupt on line 143 whose
/// handler takes a few more M-cycles each frame (4 per step of its loop) ends at every
/// moment before, and after, the start of VBlank; the main loop still runs once a frame
#[test]
fn engine_wait_frame_never_misses_a_vblank() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = RustBoy::new();
    gb.vars.create_u8("wVBlanks", 0);
    gb.vars.create_u8("wLoops", 0);
    gb.vars.create_u8("wSteps", 0);
    gb.on_vblank(count("wVBlanks"));
    let mut busy = Block::new();
    let delay = gb.labels().local("delay");
    busy.ld_a_addr_def("wSteps")
        .inc(R8::A)
        .label(&delay)
        .dec(R8::A)
        .jr_cond(Condition::NZ, &delay);
    gb.on_stat(Stat::new().line(143), busy);
    gb.add_to_main_loop(count("wLoops"));
    // One more step each frame: 1 to 64
    let mut next = Block::new();
    next.ld_a_addr_def("wSteps")
        .inc(R8::A)
        .and(63)
        .ld_addr_def_a("wSteps");
    gb.add_to_main_loop(next);
    let mut gb = engine_rom(&gb).boot();
    gb.run_frames(5);
    let (vblanks, loops) = (gb.read_symbol("wVBlanks"), gb.read_symbol("wLoops"));
    gb.run_frames(128);
    assert_eq!(gb.read_symbol("wVBlanks"), vblanks.wrapping_add(128));
    assert_eq!(
        gb.read_symbol("wLoops"),
        loops.wrapping_add(128),
        "the main loop missed a frame"
    );
}

/// The STAT handler runs on the conditions `on_stat` selects: on the LYC line once a frame,
/// and in HBlank on each of the 144 visible lines
#[test]
fn engine_stat_handler_runs_on_its_conditions() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = RustBoy::new();
    gb.vars.create_u8("wStats", 0);
    gb.vars.create_u8("wStatLY", 0);
    let mut handler = count("wStats");
    handler.emit_all(store_ly("wStatLY").into_instrs());
    gb.on_stat(Stat::new().line(100), handler);
    let mut gb = engine_rom(&gb).boot();
    gb.run_frames(5);
    let stats = gb.read_symbol("wStats");
    gb.run_frames(50);
    assert_eq!(gb.read_symbol("wStats"), stats + 50, "once a frame");
    assert_eq!(gb.read_symbol("wStatLY"), 100, "on line 100");

    let mut gb = RustBoy::new();
    gb.vars.create_u8("wStats", 0);
    gb.vars.create_u8("wStatMode", 0);
    let mut handler = count("wStats");
    handler
        .ldh(R8::A, Mem::addr(hw::STAT))
        .and(0b11)
        .ld_addr_def_a("wStatMode");
    gb.on_stat(Stat::new().hblank(), handler);
    let mut gb = engine_rom(&gb).boot();
    gb.run_frames(5);
    let stats = gb.read_symbol("wStats");
    gb.run_frames(1);
    assert_eq!(
        gb.read_symbol("wStats"),
        stats.wrapping_add(144),
        "once on each visible line"
    );
    assert_eq!(gb.read_symbol("wStatMode"), 0, "in mode 0");
}

/// The timer handler runs once a period of the timer `on_timer` starts: at 65536 Hz (16
/// M-cycles a step) from 128, every 2048 M-cycles; and `rTIMA` starts again from the modulo
#[test]
fn engine_timer_handler_runs_once_a_period() {
    if !rgbds_enabled() {
        return;
    }
    let timer = Timer::new(TimerClock::Hz65536, 0x80);
    let period = u64::from(timer.period_mcycles());
    assert_eq!(period, 2048);
    let mut gb = RustBoy::new();
    gb.vars.create_u8("wTimers", 0);
    gb.vars.create_u8("wTima", 0);
    let mut handler = count("wTimers");
    handler
        .ldh(R8::A, Mem::addr(hw::TIMA))
        .ld_addr_def_a("wTima");
    gb.on_timer(timer, handler);
    let mut gb = engine_rom(&gb).boot();
    gb.run_frames(5);
    for _ in 0..3 {
        let (cycle, timers) = (gb.cycles(), gb.read_symbol("wTimers"));
        gb.run_until_cycle(cycle + 100 * period);
        let ran = gb.read_symbol("wTimers").wrapping_sub(timers);
        assert!(
            (99..=101).contains(&ran),
            "{} timer interrupts in 100 periods",
            ran
        );
    }
    // Read a step after the reload at most (the handler starts ~20 M-cycles after it)
    let tima = gb.read_symbol("wTima");
    assert!(
        (0x80..=0x81).contains(&tima),
        "rTIMA reloaded from rTMA: {:#04X}",
        tima
    );
}

/// Every handler keeps the registers of the code it interrupts: the main loop loads every
/// register, runs 2000 `nop`s while the timer (every 256 M-cycles), STAT (HBlank) and VBlank
/// handlers change every register, then stores them. The STAT handler calls a routine whose
/// convention says it changes `de` only: the handler saves `de` for it.
#[test]
fn engine_handlers_preserve_the_registers() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = RustBoy::new();
    for var in [
        "wA",
        "wB",
        "wC",
        "wD",
        "wE",
        "wH",
        "wL",
        "wF",
        "wTimers",
        "wScramble",
    ] {
        gb.vars.create_u8(var, 0);
    }
    // Code that changes every register and the flags
    let clobber = || {
        let mut code = Block::new();
        code.ld_bc(0x0F0F)
            .ld_de(0x0F0F)
            .ld_hl(0x0F0F)
            .xor(R8::A)
            .ld_addr_def_a("wScramble");
        code
    };
    let mut timer = count("wTimers");
    timer.emit_all(clobber().into_instrs());
    gb.on_timer(Timer::new(TimerClock::Hz65536, 0xF0), timer);
    gb.on_vblank(clobber());
    let mut body = Block::new();
    body.label("ScrambleDe").ld_de(0).ret();
    let scramble = Routine::new("ScrambleDe", body).with_clobbers(Regs::DE);
    gb.on_interrupt_routine(Interrupt::Stat, &scramble);
    gb.on_stat(Stat::new().hblank(), Vec::<Instr>::new());

    let mut code = Block::new();
    code.ld_bc(0x1234)
        .ld_de(0x5678)
        .ld_hl(0x9ABC)
        .ld_a(0x5A)
        .or(R8::A)
        .scf(); // f: carry only
    for _ in 0..2000 {
        code.nop();
    }
    code.ld_addr_def_a("wA");
    for (reg, var) in [
        (R8::B, "wB"),
        (R8::C, "wC"),
        (R8::D, "wD"),
        (R8::E, "wE"),
        (R8::H, "wH"),
        (R8::L, "wL"),
    ] {
        code.ld(R8::A, reg).ld_addr_def_a(var);
    }
    code.push(R16Stack::AF)
        .pop(R16Stack::BC)
        .ld(R8::A, R8::C)
        .ld_addr_def_a("wF");
    gb.add_to_main_loop(code);
    let out = gb.build().expect("the program builds");
    let mut gb = Rom::build(&out, &[]).boot();
    gb.run_frames(10);
    // Every 256 M-cycles
    let timers = gb.read_symbol("wTimers");
    gb.run_until_cycle(gb.cycles() + 256 * 200);
    let ran = gb.read_symbol("wTimers").wrapping_sub(timers);
    assert!(ran >= 190, "the timer handler ran {} times", ran);
    let regs: Vec<u8> = ["wA", "wB", "wC", "wD", "wE", "wH", "wL", "wF"]
        .iter()
        .map(|var| gb.read_symbol(var))
        .collect();
    assert_eq!(regs, [0x5A, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0x10]);
    // For the routine, its convention: de
    assert!(
        out.contains("StatHandler:\n    push de\n    call ScrambleDe"),
        "{}",
        out
    );
}

/// An interrupt set up by hand (its own vector in `raw()` code, the timer started and its
/// bit enabled in `init()` code): with its bit ORed into `rIE`, as documented, the timer
/// and the main loop both run; writing `rIE` (`IEF_TIMER` only, VBlank left out) still
/// leaves the main loop running, because `WaitFrame` enables VBlank again
#[test]
fn engine_interrupt_set_up_by_hand_in_init() {
    if !rgbds_enabled() {
        return;
    }
    let program = |or_in: bool| {
        let mut gb = RustBoy::new();
        gb.vars.create_u8("wLoops", 0);
        gb.vars.create_u8("wTicks", 0);
        gb.add_to_main_loop(count("wLoops"));
        gb.raw(|asm| {
            asm.chunk(Chunk::Header)
                .section(Section::rom0("My timer").at(0x50))
                .push(R16Stack::AF)
                .ld_a_addr_def("wTicks")
                .inc(R8::A)
                .ld_addr_def_a("wTicks")
                .pop(R16Stack::AF)
                .reti();
        });
        let mut init = Block::new();
        init.emit_all(rust_boy::stdlib::interrupts::start_timer(Timer::new(
            TimerClock::Hz4096,
            0xC0,
        )));
        if or_in {
            init.ldh(R8::A, Mem::addr(hw::IE))
                .or(hw::IEF_TIMER)
                .ldh(Mem::addr(hw::IE), R8::A);
        } else {
            init.ld(R8::A, hw::IEF_TIMER).ldh(Mem::addr(hw::IE), R8::A);
        }
        gb.init(init);
        engine_rom(&gb).boot()
    };
    for or_in in [true, false] {
        let mut gb = program(or_in);
        gb.run_frames(5);
        let (loops, ticks) = (gb.read_symbol("wLoops"), gb.read_symbol("wTicks"));
        gb.run_frames(30);
        assert_eq!(
            gb.read_symbol("wLoops"),
            loops + 30,
            "the main loop, rIE {}",
            if or_in { "ORed" } else { "written" }
        );
        // Every 16384 M-cycles: about 1 a frame
        let ran = gb.read_symbol("wTicks") - ticks;
        assert!((31..=33).contains(&ran), "{} timer interrupts", ran);
        assert_eq!(gb.read(0xFFFF) & 0x05, 0x05, "VBlank and timer enabled");
    }
}

/// The VBlank handler keeps the registers of the code it interrupts, its own `a` included:
/// the main loop loads `a` and the flags, sleeps in a `halt` of its own until the VBlank
/// interrupt (whose handler writes `a` for its flag), then stores them
#[test]
fn engine_vblank_handler_preserves_a_and_the_flags() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = RustBoy::new();
    gb.vars.create_u8("wA", 0);
    gb.vars.create_u8("wF", 0);
    gb.vars.create_u8("wLoops", 0);
    gb.add_to_main_loop(count("wLoops"));
    let mut code = Block::new();
    code.ld_a(0x5A)
        .or(R8::A)
        .scf() // f: carry only
        .halt() // the next VBlank's handler runs here
        .nop()
        .ld_addr_def_a("wA")
        .push(R16Stack::AF)
        .pop(R16Stack::BC)
        .ld(R8::A, R8::C)
        .ld_addr_def_a("wF");
    gb.add_to_main_loop(code);
    let mut gb = engine_rom(&gb).boot();
    gb.run_frames(10);
    assert!(gb.read_symbol("wLoops") >= 4, "the main loop ran");
    assert_eq!(
        (gb.read_symbol("wA"), gb.read_symbol("wF")),
        (0x5A, 0x10),
        "a and the flags after the VBlank handler"
    );
}

/// `disable_interrupt` stops a handler while the program runs, `enable_interrupt` starts it
/// again: the main loop disables the timer on its 10th frame and enables it on its 20th
#[test]
fn engine_interrupts_enabled_and_disabled_at_run_time() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = RustBoy::new();
    let loops = gb.vars.create_u8("wLoops", 0);
    gb.vars.create_u8("wTimers", 0);
    gb.on_timer(Timer::new(TimerClock::Hz4096, 0xC0), count("wTimers"));
    gb.add_to_main_loop(count("wLoops"));
    gb.add_to_main_loop(loops.get());
    let disable = gb.disable_interrupt(Interrupt::Timer);
    gb.add_to_main_loop(IfA::eq(10, disable));
    gb.add_to_main_loop(loops.get());
    let enable = gb.enable_interrupt(Interrupt::Timer);
    gb.add_to_main_loop(IfA::eq(20, enable));
    let mut gb = engine_rom(&gb).boot();
    gb.run_frames(12);
    let stopped = gb.read_symbol("wTimers");
    assert!(stopped >= 5, "the timer ran: {}", stopped);
    gb.run_frames(6);
    assert_eq!(gb.read_symbol("wTimers"), stopped, "disabled");
    gb.run_frames(10);
    assert!(gb.read_symbol("wTimers") > stopped, "enabled again");
}

/// The joypad handler runs when a button is pressed: the start-up selects both groups of
/// keys, so a press pulls a line low; holding it does not run it again
#[test]
fn engine_joypad_handler_runs_on_a_press() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = RustBoy::new();
    gb.vars.create_u8("wPresses", 0);
    gb.on_joypad(count("wPresses"));
    let mut gb = engine_rom(&gb).boot();
    gb.run_frames(5);
    assert_eq!(gb.read_symbol("wPresses"), 0);
    gb.run_script(&[(5, &[Button::A]), (2, &[]), (3, &[Button::Left])]);
    assert_eq!(gb.read_symbol("wPresses"), 2, "A, then Left");

    // With UpdateKeys, which leaves no group selected: the main loop selects them again,
    // so a press in the middle of the frame runs the handler at once, not at the next
    // UpdateKeys (in VBlank)
    let mut gb = RustBoy::new();
    gb.vars.create_u8("wPresses", 0);
    gb.vars.create_u8("wPressLY", 0);
    let mut handler = count("wPresses");
    handler.emit_all(store_ly("wPressLY").into_instrs());
    gb.on_joypad(handler);
    let mut inputs = InputManager::new();
    inputs.on_press(PadButton::A, Vec::new());
    gb.add_inputs(inputs);
    let mut gb = engine_rom(&gb).boot();
    gb.run_frames(5);
    while gb.ly_mode().0 != 50 {
        gb.run_until_cycle(gb.cycles() + 1);
    }
    gb.press(Button::A);
    gb.run_until_cycle(gb.cycles() + 100);
    assert_eq!(gb.read_symbol("wPresses"), 1, "at the press");
    assert_eq!(gb.read_symbol("wPressLY"), 50);
}

/// The soft reset: A + B + Select + Start held together run the start-up code again, once
/// per press. `wBoots`, a byte the start-up does not set, counts the start-ups (the `init`
/// code adds 1); `wFrames`, a variable, counts the frames since the last one.
#[test]
fn engine_soft_reset() {
    if !rgbds_enabled() {
        return;
    }
    let reset = [Button::A, Button::B, Button::Select, Button::Start];
    let mut gb = RustBoy::new();
    gb.set_soft_reset(true);
    let frames = gb.vars.create_u8("wFrames", 0);
    gb.add_to_main_loop(count(frames.name()));
    gb.init(count("wBoots"));
    gb.raw(|asm| {
        asm.chunk(Chunk::Data).label("wBoots").ds("1");
    });
    let rom = engine_rom(&gb);
    let mut gb = rom.boot();
    gb.run_frames(30);
    assert_eq!(gb.read_symbol("wBoots"), 1);
    assert!(gb.read_symbol("wFrames") >= 28);

    // Three of the four: nothing
    gb.run_script(&[(5, &[Button::A, Button::B, Button::Start])]);
    assert_eq!(gb.read_symbol("wBoots"), 1);

    // The four, held for 20 frames: one reset, and the game runs again
    gb.run_script(&[(20, &reset)]);
    assert_eq!(
        gb.read_symbol("wBoots"),
        2,
        "one reset while the four are held"
    );
    let since = gb.read_symbol("wFrames");
    assert!(
        (10..20).contains(&since),
        "the variables started again: {} frames",
        since
    );
    assert!(gb.run_until_pc(gb.symbol("Main"), 2));
    assert_eq!(gb.registers().sp, hw::STACK_TOP, "the stack starts again");

    // Released, then the four again: a second reset
    gb.run_script(&[(5, &[]), (3, &reset), (5, &[])]);
    assert_eq!(gb.read_symbol("wBoots"), 3);
    assert!(gb.read_symbol("wFrames") < 10);
}

// ----- B12: the shadow OAM and the OAM DMA -----

/// A program whose main loop moves a sprite one pixel right every frame, after
/// `delay` iterations of the `Delay` routine (about 7 M-cycles each)
fn moving_sprite_rom(delay: u16) -> Rom {
    let mut gb = RustBoy::new();
    let tile = ["`33333333"; 8];
    let dot = gb.add_sprite("Dot", TileSource::from_raw(&[tile]), 8, 8, 0);
    gb.vars.create_i8("wStep", 1);
    if delay > 0 {
        let mut busy = Block::new();
        busy.ld_bc(delay);
        gb.add_to_main_loop(busy);
        let call = gb.call("Delay");
        gb.add_to_main_loop(call);
    }
    gb.add_to_main_loop(gb.sprites.move_x_var(dot, "wStep"));
    Rom::build(&gb.build().expect("the program builds"), &[])
}

/// [B12], fixed: the engine moves a sprite in the shadow OAM, in WRAM, and the VBlank
/// handler copies it to OAM with an OAM DMA, so the sprite moves one pixel a frame whatever
/// the main loop does before the move. Before the fix the move wrote OAM itself: with 1004
/// iterations of `Delay` (1002 to 1007 gave the same) it landed in mode 3 of line 52 on
/// every frame, where the read gave `$FF` and the write was dropped, so the sprite never
/// moved. Here the delays put the move in VBlank (0), in mode 3 of line 52 (1004), and at
/// many other points of the frame, up to the end of its visible lines.
///
/// [B12]: ../CONTEXT.md#b12
#[test]
fn b12_sprite_moves_whatever_the_main_loop_delay() {
    if !rgbds_enabled() {
        return;
    }
    let delays = [0, 1, 300, 1004, 1005, 1500, 2000, 2300];
    let runs: Vec<(u16, GameBoy, u8, usize)> = delays
        .into_iter()
        .map(|delay| {
            let mut gb = moving_sprite_rom(delay).boot();
            gb.run_frames(10);
            let x = gb.oam_entry(0)[1];
            let dmas = gb.oam_dmas().len();
            gb.run_frames(60);
            (delay, gb, x, dmas)
        })
        .collect();
    // First the sprite, for every delay: before the fix it stood still from 1002 to 1007
    let stuck: Vec<(u16, u8, u8)> = runs
        .iter()
        .filter(|(_, gb, x, _)| gb.oam_entry(0)[1] != x.wrapping_add(60))
        .map(|(delay, gb, x, _)| (*delay, *x, gb.oam_entry(0)[1]))
        .collect();
    assert!(
        stuck.is_empty(),
        "the sprite did not move one pixel a frame (delay, X before, X 60 frames later): {:?}",
        stuck
    );
    // Then how it got there: one OAM DMA of the shadow OAM a frame, in VBlank
    for (delay, gb, _, dmas) in &runs {
        assert_eq!(
            gb.oam_dmas().len() - dmas,
            60,
            "delay {}: one OAM DMA a frame",
            delay
        );
        assert_sprites_shown_by_dma_in_vblank(gb);
    }
}

/// The VBlank handler of `rom` over 10 frames: the M-cycles from its first instruction to
/// the end of its `reti`, and `LY` / the PPU mode there (the first `reti` the CPU reaches
/// from `VBlankHandler`: the handler has one, at its end)
fn vblank_handler_runs(rom: &Rom, frames: usize) -> Vec<(u64, u8, Mode)> {
    let mut gb = rom.boot();
    gb.run_frames(5);
    let handler = gb.symbol("VBlankHandler");
    let mut runs = Vec::new();
    for _ in 0..frames {
        assert!(gb.run_until_pc(handler, 2), "the VBlank handler runs");
        let start = gb.cycles();
        while gb.read(gb.registers().pc) != 0xD9 {
            gb.step();
        }
        gb.step();
        let (ly, mode) = gb.ly_mode();
        runs.push((gb.cycles() - start, ly, mode));
    }
    runs
}

/// The VBlank handler's work fits in VBlank (1140 M-cycles from line 144): the OAM DMA of
/// the shadow OAM (B12) takes about 180 M-cycles with its call, and the handler ends long
/// before line 0, where OAM is the PPU's again
#[test]
fn engine_vblank_handler_fits_in_vblank() {
    if !rgbds_enabled() {
        return;
    }
    for delay in [0, 1004] {
        let runs = vblank_handler_runs(&moving_sprite_rom(delay), 10);
        for &(cycles, ly, mode) in &runs {
            assert!(
                (144..=153).contains(&ly) && mode == Mode::VBlank,
                "the handler ended on line {} ({:?}) after {} M-cycles",
                ly,
                mode,
                cycles
            );
        }
        let cycles = runs[0].0;
        assert!(
            runs.iter().all(|run| run.0 == cycles),
            "the same work every frame: {:?}",
            runs
        );
        // push af, the DMA (ld a, call, the 168 M-cycles of the routine), the frame flag,
        // pop af, reti
        assert!((180..=230).contains(&cycles), "{} M-cycles", cycles);
        eprintln!("VBlank handler with the OAM DMA: {} M-cycles", cycles);
    }
}
