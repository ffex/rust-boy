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
use rust_boy::stdlib::banking;
use rust_boy::stdlib::graphics::vram_queue;
use rust_boy::stdlib::sram;
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

#[test]
fn sram_control_routines_assemble_and_return() {
    if !rgbds_enabled() {
        return;
    }

    let mut game = RustBoy::new();
    game.vars.create_u8("wSramBankHigh", 0);
    game.vars.create_u8("wSramBankLow", 0);
    let enable = game.call_routine(&sram::enable());
    let select = game.call_routine(&sram::select_mbc5_bank());
    let disable = game.call_routine(&sram::disable());
    let mut init = Block::new();
    init.emit_all(enable)
        .ld_hl(0xABCD)
        .emit_all(select)
        .ld(R8::A, R8::H)
        .ld_addr_def_a("wSramBankHigh")
        .ld(R8::A, R8::L)
        .ld_addr_def_a("wSramBankLow")
        .emit_all(disable);
    game.init(init);

    let mut gb = engine_rom(&game).boot();
    gb.run_frames(5);
    assert_eq!(gb.read_symbol("wSramBankHigh"), 0xAB);
    assert_eq!(gb.read_symbol("wSramBankLow"), 0xCD);
}

#[test]
fn mbc_bank_selectors_assemble_and_return_to_the_caller() {
    if !rgbds_enabled() {
        return;
    }

    let mut game = RustBoy::new();
    for name in ["wBankHigh", "wBankLow"] {
        game.vars.create_u8(name, 0);
    }
    let mbc1 = game.call_routine(&banking::set_mbc1_rom_bank());
    let mbc5 = game.call_routine(&banking::set_mbc5_rom_bank());
    let mut init = Block::new();
    init.ld_hl(0x0041)
        .emit_all(mbc1)
        .ld(R8::A, R8::H)
        .ld_addr_def_a("wBankHigh")
        .ld(R8::A, R8::L)
        .ld_addr_def_a("wBankLow")
        .ld_hl(0x01FF)
        .emit_all(mbc5)
        .ld(R8::A, R8::H)
        .ld_addr_def_a("wBankHigh")
        .ld(R8::A, R8::L)
        .ld_addr_def_a("wBankLow");
    game.init(init);

    let mut gb = engine_rom(&game).boot();
    gb.run_frames(5);
    assert_eq!(gb.read_symbol("wBankHigh"), 1);
    assert_eq!(gb.read_symbol("wBankLow"), 0xFF);
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

#[test]
fn engine_input_edges_hold_and_repeat_are_separate_bindings() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = RustBoy::new();
    for name in ["wJustPressed", "wReleased", "wHeld", "wRepeats"] {
        gb.vars.create_u8(name, 0);
    }
    let mut inputs = InputManager::new();
    inputs
        .on_just_pressed(PadButton::A, count("wJustPressed").into_instrs())
        .on_release(PadButton::A, count("wReleased").into_instrs())
        .on_held(PadButton::A, count("wHeld").into_instrs())
        .on_repeat(PadButton::A, 3, 2, count("wRepeats").into_instrs());
    gb.add_inputs(inputs);
    let mut gb = engine_rom(&gb).boot();
    gb.run_frames(4);
    gb.run_script(&[(7, &[Button::A])]);
    assert_eq!(gb.read_symbol("wJustPressed"), 1);
    assert_eq!(
        gb.read_symbol("wRepeats"),
        3,
        "immediate, then after 3 and 5 frames"
    );
    assert!(
        gb.read_symbol("wHeld") >= 7,
        "held action runs each sampled frame"
    );
    gb.run_script(&[(3, &[])]);
    assert_eq!(gb.read_symbol("wReleased"), 1);
    gb.run_script(&[(2, &[Button::A]), (2, &[])]);
    assert_eq!(gb.read_symbol("wJustPressed"), 2, "a new press fires again");
    assert_eq!(gb.read_symbol("wReleased"), 2, "a new release fires again");
}

#[test]
fn engine_repeat_bindings_across_managers_keep_independent_intervals() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    game.vars.create_u8("wRepeatA", 0);
    game.vars.create_u8("wRepeatB", 0);
    game.vars.create_u8("wReleaseA", 0);
    game.vars.create_u8("wPressB", 0);
    game.vars.create_u8("wReleaseB", 0);

    let mut first = InputManager::new();
    first
        .on_repeat(PadButton::A, 2, 3, count("wRepeatA").into_instrs())
        .on_release(PadButton::A, count("wReleaseA").into_instrs());
    game.add_inputs(first);
    let mut second = InputManager::new();
    second
        .on_repeat(PadButton::B, 3, 2, count("wRepeatB").into_instrs())
        .on_just_pressed(PadButton::B, count("wPressB").into_instrs())
        .on_release(PadButton::B, count("wReleaseB").into_instrs());
    game.add_inputs(second);

    let mut gb = engine_rom(&game).boot();
    gb.run_frames(3);
    gb.run_script(&[(1, &[Button::A, Button::B])]);
    assert_eq!(
        (gb.read_symbol("wRepeatA"), gb.read_symbol("wRepeatB")),
        (1, 1)
    );
    assert_eq!(gb.read_symbol("wInputRepeat_0"), 2);
    assert_eq!(gb.read_symbol("wInputRepeat_2"), 3);
    assert_eq!(gb.read_symbol("wPressB"), 1);

    gb.run_script(&[(2, &[Button::A, Button::B])]);
    assert_eq!(
        (gb.read_symbol("wRepeatA"), gb.read_symbol("wRepeatB")),
        (2, 1)
    );
    gb.run_script(&[(2, &[Button::A, Button::B])]);
    assert_eq!(
        (gb.read_symbol("wRepeatA"), gb.read_symbol("wRepeatB")),
        (2, 2)
    );
    gb.run_script(&[(1, &[Button::A, Button::B])]);
    assert_eq!(
        gb.read_symbol("wRepeatA"),
        3,
        "A reloads its 3-frame interval"
    );

    // Releasing A while Left remains held records only A as released.
    gb.run_script(&[(1, &[Button::A, Button::Left]), (1, &[Button::Left])]);
    assert_eq!(gb.read_symbol("wReleaseA"), 1);
    assert_eq!(gb.read_symbol("wReleaseB"), 1);
    assert_eq!(gb.read_symbol("wPressB"), 1);
    assert_eq!(gb.read_symbol("wReleasedKeys"), hw::PADF_A.value);
    assert_eq!(gb.read_symbol("wCurKeys"), hw::PADF_LEFT.value);
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

// ----- Queued VRAM writes -----

/// Fill the queue in initialization (overflow is reported), then queue a full batch
/// every frame. Optionally delay the producer into mode 3. OAM DMA shares the handler.
fn vram_queue_rom(delay: u16) -> Rom {
    let mut game = RustBoy::new();
    game.add_sprite(
        "QueueDot",
        TileSource::from_raw(&[["`33333333"; 8]]),
        8,
        8,
        0,
    );
    game.vars.create_u8("wRejectedWrites", 0);
    let mut init = Block::new();
    for i in 0..=vram_queue::CAPACITY {
        init.emit_all(game.queue_vram_write(hw::SCRN0.value + u16::from(i), i + 1));
        let accepted = game.labels().local("queue_accepted");
        init.jr_cond(Condition::NC, &accepted)
            .ld(R16::HL, "wRejectedWrites")
            .inc(R8::AtHl)
            .label(&accepted);
    }
    init.label("QueueInitDone");
    game.init(init);
    let mut work = Block::new();
    if delay != 0 {
        work.ld(R16::BC, delay).call("Delay");
    }
    work.label("QueueBatchStart");
    for i in 0..vram_queue::CAPACITY {
        work.emit_all(game.queue_vram_write(hw::SCRN1.value + u16::from(i), 100 + i));
    }
    game.add_to_main_loop(work);
    Rom::build(&game.build().unwrap(), &[])
}

#[test]
fn engine_vram_queue_full_batch_fits_with_oam_dma() {
    if !rgbds_enabled() {
        return;
    }
    let rom = vram_queue_rom(0);
    let mut gb = rom.boot();
    assert!(gb.run_until_pc(gb.symbol("QueueInitDone"), 3));
    assert!(
        !gb.registers().ime,
        "enqueue preserved disabled IME during init"
    );
    assert_eq!(gb.read_symbol("wRejectedWrites"), 1, "overflow is rejected");
    assert_eq!(
        gb.vram()[usize::from(hw::SCRN0.value - hw::VRAM.value)],
        0,
        "enqueue did not write VRAM before the first flush"
    );
    gb.run_frames(5);
    for i in 0..vram_queue::CAPACITY {
        assert_eq!(
            gb.vram()[usize::from(hw::SCRN0.value - hw::VRAM.value) + usize::from(i)],
            i + 1
        );
        assert_eq!(
            gb.vram()[usize::from(hw::SCRN1.value - hw::VRAM.value) + usize::from(i)],
            100 + i
        );
    }
    assert_eq!(
        gb.vram()
            [usize::from(hw::SCRN0.value - hw::VRAM.value) + usize::from(vram_queue::CAPACITY)],
        0
    );
    let runs = vblank_handler_runs(&rom, 20);
    for &(cycles, ly, mode) in &runs {
        assert_eq!(
            mode,
            Mode::VBlank,
            "handler ended after {cycles} cycles, line {ly}"
        );
        assert!(
            cycles < u64::from(hw::VBLANK_MCYCLES) - 100,
            "full queue plus DMA left less than 100 M-cycles: {cycles}"
        );
    }
    eprintln!(
        "Full VRAM queue and OAM DMA handler: {} M-cycles",
        runs[0].0
    );
}

#[test]
fn engine_vram_queue_writes_in_vblank_even_with_delayed_game_logic() {
    if !rgbds_enabled() {
        return;
    }
    let rom = vram_queue_rom(1004);
    let mut gb = rom.boot();
    let end = u64::from(hw::LINE_MCYCLES) * 154 * 12;
    let write = gb.symbol("FlushVramQueue.write");
    let mut count = 0;
    while gb.cycles() < end {
        if gb.registers().pc == write {
            let (ly, mode) = gb.ly_mode();
            assert_eq!(mode, Mode::VBlank, "VRAM write on line {ly}");
            count += 1;
        }
        gb.step();
    }
    assert!(count > 50, "queue flushed only {count} entries");
    assert_eq!(gb.blocked_accesses().0, 0);
    assert_sprites_shown_by_dma_in_vblank(&gb);
    for i in 0..vram_queue::CAPACITY {
        assert_eq!(
            gb.vram()[usize::from(hw::SCRN1.value - hw::VRAM.value) + usize::from(i)],
            100 + i
        );
    }
}

#[test]
fn engine_vram_queue_publish_is_safe_at_every_instruction_boundary() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    let mut work = Block::new();
    work.emit_all(game.queue_vram_write(hw::SCRN0, 17))
        .label("QueueInjectionBegin")
        .emit_all(game.queue_vram_write(hw::SCRN0, 34))
        .label("QueueInjectionDone")
        .label(".park")
        .halt()
        .jr(".park");
    game.add_to_main_loop(work);
    let rom = Rom::build(&game.build().unwrap(), &[]);
    let begin = rom.symbols["QueueInjectionBegin"];
    let done = rom.symbols["QueueInjectionDone"];
    let enqueue = rom.symbols["QueueVramWrite"];
    let mut reference = rom.boot();
    assert!(reference.run_until_pc(begin, 3));
    assert!(reference.run_until_pc(enqueue, 1));
    let mut boundaries = Vec::new();
    while reference.registers().pc != done {
        boundaries.push(reference.registers().pc);
        reference.step();
        assert!(boundaries.len() < 100);
    }
    assert!(boundaries.len() > 20);
    for boundary in boundaries {
        let mut gb = rom.boot();
        assert!(gb.run_until_pc(begin, 3));
        assert!(gb.run_until_pc(enqueue, 1));
        assert!(gb.run_until_pc(boundary, 1));
        assert_eq!(gb.ly_mode().1, Mode::VBlank);
        let before = gb.registers();
        assert!(before.ime);
        // Only a published entry may be consumed, even if every data byte has already
        // been written. The interrupt must preserve the producer's registers and stack.
        let expected = if gb.read_symbol(vram_queue::WRITE) == 6 {
            34
        } else {
            17
        };
        gb.request_interrupt(hw::IEF_VBLANK.value);
        gb.step();
        assert!(gb.run_until_pc(boundary, 1));
        assert_eq!(gb.registers(), before, "registers at ${boundary:04X}");
        assert_eq!(
            gb.vram()[usize::from(hw::SCRN0.value - hw::VRAM.value)],
            expected,
            "partial entry at ${boundary:04X}"
        );
        assert!(gb.run_until_pc(done, 1));
        assert!(gb.registers().ime, "enqueue preserved enabled IME");
        gb.run_frames(2);
        assert_eq!(gb.vram()[usize::from(hw::SCRN0.value - hw::VRAM.value)], 34);
        assert_eq!(
            gb.read_symbol(vram_queue::READ),
            gb.read_symbol(vram_queue::WRITE)
        );
        assert_eq!(gb.blocked_accesses().0, 0);
    }
}

#[test]
fn engine_vram_queue_late_flush_leaves_entries_for_the_next_vblank() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    let mut work = Block::new();
    work.di()
        .emit_all(game.queue_vram_write(hw::SCRN0, 42))
        .label("WaitLastVblankLine")
        .ld_a_addr_def(hw::LY)
        .cp(hw::LY_LAST_LINE)
        .jr_cond(Condition::NZ, "WaitLastVblankLine")
        .call("FlushVramQueue")
        .label("QueueDeferred")
        .ei()
        .label(".park")
        .halt()
        .jr(".park");
    game.add_to_main_loop(work);
    let rom = Rom::build(&game.build().unwrap(), &[]);
    let mut gb = rom.boot();
    assert!(gb.run_until_pc(gb.symbol("QueueDeferred"), 3));
    assert_ne!(
        gb.read_symbol(vram_queue::READ),
        gb.read_symbol(vram_queue::WRITE)
    );
    assert_eq!(gb.vram()[usize::from(hw::SCRN0.value - hw::VRAM.value)], 0);
    assert!(!gb.registers().ime, "flush preserved disabled IME");
    gb.run_frames(2);
    assert_eq!(gb.vram()[usize::from(hw::SCRN0.value - hw::VRAM.value)], 42);
    assert_eq!(
        gb.read_symbol(vram_queue::READ),
        gb.read_symbol(vram_queue::WRITE)
    );
    assert_eq!(gb.blocked_accesses().0, 0);
}

// Background camera writes are visible in the registers (the emulator has no pixels).
#[test]
fn engine_background_camera_wraps_and_follows_world_pixels() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    game.init(game.set_background_scroll(250, 3));
    game.on_vblank(game.scroll_background_by(10, -5));
    let mut gb = engine_rom(&game).boot();
    // Compare successive frames: this avoids relying on boot's frame phase.
    gb.run_frames(2);
    let before = (gb.read(hw::SCX.value), gb.read(hw::SCY.value));
    gb.run_frames(1);
    assert_eq!(gb.read(hw::SCX.value), before.0.wrapping_add(10));
    assert_eq!(gb.read(hw::SCY.value), before.1.wrapping_sub(5));

    for (x, y) in [(0u8, 0u8), (255, 255), (80, 72), (12, 200)] {
        let mut game = RustBoy::new();
        let mut code = Block::new();
        code.ld(R8::B, x)
            .ld(R8::C, y)
            .emit_all(game.follow_background_point(80, 72))
            .ld_addr_def_a("wAfterFollow")
            .ld(R8::A, R8::B)
            .ld_addr_def_a("wAfterB")
            .ld(R8::A, R8::C)
            .ld_addr_def_a("wAfterC");
        game.vars.create_u8("wAfterFollow", 0);
        game.vars.create_u8("wAfterB", 0);
        game.vars.create_u8("wAfterC", 0);
        game.on_vblank(code);
        let mut gb = engine_rom(&game).boot();
        gb.run_frames(2);
        assert_eq!(gb.read(hw::SCX.value), x.wrapping_sub(80));
        assert_eq!(gb.read(hw::SCY.value), y.wrapping_sub(72));
        assert_eq!(gb.read_symbol("wAfterFollow"), y.wrapping_sub(72));
        // B and C, the target, are preserved
        assert_eq!(gb.read_symbol("wAfterB"), x);
        assert_eq!(gb.read_symbol("wAfterC"), y);
    }
}

#[test]
fn engine_background_scroll_init_survives_and_zero_delta_is_idle() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    game.init(game.set_background_scroll(255, 128));
    assert!(game.scroll_background_by(0, 0).is_empty());
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(3);
    assert_eq!(gb.read(hw::SCX.value), 255);
    assert_eq!(gb.read(hw::SCY.value), 128);
}

#[test]
fn arbitrary_metasprites_keep_irregular_layout_at_every_edge() {
    if !rgbds_enabled() {
        return;
    }
    type Move = fn(&rust_boy::engine::SpriteManager, CompositeSpriteId, u8, u8) -> Vec<Instr>;
    for size in [SpriteSize::Size8x8, SpriteSize::Size8x16] {
        for (movement, axis, increasing) in [
            (
                rust_boy::engine::SpriteManager::move_composite_left_limit as Move,
                1usize,
                false,
            ),
            (
                rust_boy::engine::SpriteManager::move_composite_right_limit as Move,
                1,
                true,
            ),
            (
                rust_boy::engine::SpriteManager::move_composite_up_limit as Move,
                0,
                false,
            ),
            (
                rust_boy::engine::SpriteManager::move_composite_down_limit as Move,
                0,
                true,
            ),
        ] {
            let mut game = RustBoy::with_config(RustBoyConfig::default().sprite_size(size));
            // Irregular, 32 pixels wide, with neither leading X/Y part first.
            let offsets = [(8, 8), (-8, 0), (16, -8), (0, -8), (-8, 8), (16, 8)];
            let count = usize::from(size.tiles_per_sprite());
            let parts: Vec<_> = offsets
                .iter()
                .enumerate()
                .map(|(index, &(x, y))| {
                    MetaspritePart::new(
                        TileSource::from_raw(&vec![["$ff"; 8]; count]),
                        x,
                        y,
                        (index as u8) << 4,
                    )
                })
                .collect();
            let meta = game.add_metasprite("LargeActor", 80, 72, &parts);
            assert_eq!(
                game.sprites.get_composite_sprites(meta).unwrap().len(),
                offsets.len()
            );
            let limit = if increasing { 160 } else { 32 };
            game.add_to_main_loop(movement(&game.sprites, meta, 19, limit));
            let mut gb = engine_rom(&game).boot();
            assert!(gb.run_until_pc(gb.symbol("Main"), 10));
            let initial: Vec<_> = (0..parts.len()).map(|index| gb.oam_entry(index)).collect();
            let lead = if increasing {
                initial.iter().map(|entry| entry[axis]).max().unwrap()
            } else {
                initial.iter().map(|entry| entry[axis]).min().unwrap()
            };
            gb.run_frames(12);
            for (index, before) in initial.iter().enumerate() {
                let entry = gb.oam_entry(index);
                assert_eq!(
                    i16::from(entry[axis]),
                    i16::from(limit) + i16::from(before[axis]) - i16::from(lead)
                );
                assert_eq!(entry[1 - axis], before[1 - axis]);
                assert_eq!(&entry[2..], &before[2..]);
            }
        }
    }
}

#[test]
fn typed_flags_reach_every_part_of_a_16x16_sprite_and_a_metasprite() {
    if !rgbds_enabled() {
        return;
    }
    let flags = SpriteFlags::new()
        .flip_y(true)
        .palette(ObjectPalette::Palette1)
        .priority(SpritePriority::BehindBackground);
    let mut game = RustBoy::with_config(RustBoyConfig::default().sprite_size(SpriteSize::Size8x16));
    let tiles = || TileSource::from_raw(&[["$ff"; 8]; 2]);
    game.add_sprite_16x16("Hero", tiles(), tiles(), 40, 40, flags);
    let parts = [
        MetaspritePart::new(tiles(), 0, 0, flags.flip_x(true)),
        MetaspritePart::new(tiles(), 8, 0, 0x10u8),
    ];
    game.add_metasprite("Actor", 80, 72, &parts);
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(3);
    for (entry, expected) in [
        (0, flags.bits()),
        (1, flags.bits()),
        (2, flags.flip_x(true).bits()),
        (3, 0x10),
    ] {
        assert_eq!(gb.oam_entry(entry)[3], expected, "OAM entry {entry}");
    }
}

#[test]
fn arbitrary_metasprite_animation_advances_every_part_in_both_sizes() {
    if !rgbds_enabled() {
        return;
    }
    for size in [SpriteSize::Size8x8, SpriteSize::Size8x16] {
        let mut game = RustBoy::with_config(
            RustBoyConfig::default()
                .sprite_size(size)
                .animation_delay(1),
        );
        let stride = size.tiles_per_sprite();
        let tiles = TileSource::from_raw(&vec![["$ff"; 8]; usize::from(stride) * 2]);
        let parts: Vec<_> = (0..3)
            .map(|i| MetaspritePart::new(tiles.clone(), i * 8, i * 8, 0u8))
            .collect();
        let meta = game.add_metasprite("AnimatedActor", 40, 40, &parts);
        game.sprites
            .add_composite_animation(meta, "Walk", 0, 1, AnimationType::Loop);
        game.sprites.set_composite_initial_animation(meta, 0);
        let mut gb = engine_rom(&game).boot();
        assert!(gb.run_until_pc(gb.symbol("Main"), 10));
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..8 {
            gb.run_frames(1);
            let first = gb.oam_entry(0)[2];
            assert!(first == 0 || first == stride);
            seen.insert(first);
            for index in 0..3 {
                let entry = gb.oam_entry(index);
                assert_eq!(entry[2], (index as u8) * stride * 2 + first);
                assert_eq!(entry[0], 40 + (index as u8) * 8 + hw::OAM_Y_OFFSET);
                assert_eq!(entry[1], 40 + (index as u8) * 8 + hw::OAM_X_OFFSET);
            }
        }
        assert_eq!(seen.len(), 2);
    }
}

#[test]
fn independent_sprite_and_metasprite_animation_periods_survive_dma() {
    if !rgbds_enabled() {
        return;
    }
    for size in [SpriteSize::Size8x8, SpriteSize::Size8x16] {
        let mut game = RustBoy::with_config(
            RustBoyConfig::default()
                .sprite_size(size)
                .animation_delay(3),
        );
        let stride = size.tiles_per_sprite();
        let tiles = TileSource::from_raw(&vec![["$ff"; 8]; usize::from(stride) * 2]);
        let shared = game.add_sprite("Shared", tiles.clone(), 8, 8, 0);
        let fast = game.add_sprite("Fast", tiles.clone(), 24, 8, 0);
        for sprite in [shared, fast] {
            game.sprites
                .add_animation(sprite, "Spin", 0, 1, AnimationType::Loop);
            game.sprites.set_initial_animation(sprite, 0);
        }
        game.sprites.set_animation_delay(fast, 0, 1);
        let parts: Vec<_> = (0..3)
            .map(|i| MetaspritePart::new(tiles.clone(), i * 8, 0, 0u8))
            .collect();
        let meta = game.add_metasprite("Group", 48, 48, &parts);
        game.sprites
            .add_composite_animation(meta, "Walk", 0, 1, AnimationType::Loop);
        game.sprites.set_composite_initial_animation(meta, 0);
        game.sprites.set_composite_animation_delay(meta, 0, 2);
        // Repeated enable is normal for held-key bindings; it must not stall a clock.
        game.add_to_main_loop(game.sprites.enable_composite_animation(meta, 0));
        let rom = engine_rom(&game);
        let mut gb = rom.boot();
        assert!(gb.run_until_pc(gb.symbol("Main"), 10));
        // First DMA copies the initial shadow state; following DMA copies previous logic.
        for visible in 0..16u16 {
            gb.run_frames(1);
            assert_eq!(gb.oam_entry(0)[2], stride * ((visible / 3) % 2) as u8);
            assert_eq!(
                gb.oam_entry(1)[2],
                stride * 2 + stride * (visible % 2) as u8
            );
            for part in 0..3 {
                assert_eq!(
                    gb.oam_entry(part + 2)[2],
                    stride * (4 + part as u8 * 2) + stride * ((visible / 2) % 2) as u8
                );
            }
        }
        assert_eq!(gb.blocked_accesses().1, 0);
        assert!(
            gb.oam_dmas()
                .iter()
                .filter(|dma| dma.lcd_on)
                .all(|dma| dma.first.1 == Mode::VBlank
                    && dma.last.is_some_and(|(_, mode)| mode == Mode::VBlank))
        );
    }
}

#[test]
fn animation_callbacks_run_once_after_all_metasprite_parts_and_can_chain() {
    if !rgbds_enabled() {
        return;
    }
    for size in [SpriteSize::Size8x8, SpriteSize::Size8x16] {
        let mut game = RustBoy::with_config(
            RustBoyConfig::default()
                .sprite_size(size)
                .animation_delay(1),
        );
        let stride = size.tiles_per_sprite();
        let tiles = TileSource::from_raw(&vec![["$ff"; 8]; usize::from(stride) * 2]);
        let parts: Vec<_> = (0..3)
            .map(|i| MetaspritePart::new(tiles.clone(), i * 8, 0, 0u8))
            .collect();
        let meta = game.add_metasprite("Actor", 40, 40, &parts);
        let once = game
            .sprites
            .add_composite_animation(meta, "Once", 0, 1, AnimationType::Once);
        let idle = game
            .sprites
            .add_composite_animation(meta, "Idle", 0, 1, AnimationType::Loop);
        game.sprites.set_composite_initial_animation(meta, once);
        game.sprites.set_composite_animation_delay(meta, once, 2);
        game.sprites
            .set_composite_animation_callback(meta, once, "AnimationDone");
        let calls = game.vars.create_u8("wCalls", 0);
        let coherent = game.vars.create_u8("wCoherent", 0);
        let events = game.vars.create_u8("wEvents", 0);
        let regs: Vec<_> = ["wB", "wC", "wD", "wE", "wH", "wL"]
            .iter()
            .map(|name| game.vars.create_u8(name, 0))
            .collect();
        let mut callback = Block::new();
        callback
            .ld_a_addr_def(calls.name())
            .inc(R8::A)
            .ld_addr_def_a(calls.name());
        for part in 0..3u8 {
            callback
                .ld_a_addr_def(
                    Expr::sym("wShadowOAM")
                        + i32::from(part * hw::OAM_ENTRY_SIZE.value + hw::OAMA_TILEID.value),
                )
                .cp(stride * (part * 2 + 1))
                .jr_cond(Condition::NZ, ".bad");
        }
        callback
            .ld_a(1)
            .ld_addr_def_a(coherent.name())
            .label(".bad")
            .ld(R16::BC, 0xdead)
            .ld(R16::DE, 0xbeef)
            .ld(R16::HL, 0x1234)
            .emit_all(game.sprites.restart_composite_animation(meta, idle));
        game.define_function_from("AnimationDone", callback);
        let mut init = Block::new();
        init.ld(R16::BC, 0x1122)
            .ld(R16::DE, 0x3344)
            .ld(R16::HL, 0x5566);
        game.init(init);
        let mut logic = Block::new();
        for (register, var) in [R8::B, R8::C, R8::D, R8::E, R8::H, R8::L].iter().zip(&regs) {
            logic.ld(R8::A, *register).ld_addr_def_a(var.name());
        }
        logic
            .push(R16Stack::BC)
            .emit_all(game.sprites.take_composite_animation_event(meta, once))
            .ld(R8::B, R8::A)
            .ld_a_addr_def(events.name())
            .add(R8::B)
            .ld_addr_def_a(events.name())
            .pop(R16Stack::BC);
        game.add_to_main_loop(logic);
        let rom = engine_rom(&game);
        let mut gb = rom.boot();
        gb.run_frames(20);
        assert_eq!(gb.read_symbol(calls.name()), 1);
        assert_eq!(gb.read_symbol(coherent.name()), 1);
        assert_eq!(gb.read_symbol(events.name()), 1);
        for (var, value) in regs.iter().zip([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]) {
            assert_eq!(gb.read_symbol(var.name()), value);
        }
        let relative = gb.oam_entry(0)[2];
        for part in 0..3 {
            assert_eq!(gb.oam_entry(part)[2], stride * 2 * part as u8 + relative);
        }
        assert_eq!(gb.blocked_accesses(), (0, 0, 0));
        assert_eq!(gb.dma_conflicts(), 0);
    }
}

#[test]
fn a_once_callback_can_restart_itself_without_duplicate_terminal_notifications() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::with_config(RustBoyConfig::default().animation_delay(1));
    let sprite = game.add_sprite("Actor", TileSource::from_raw(&[["$00"; 8]; 3]), 8, 8, 0);
    let once = game
        .sprites
        .add_animation(sprite, "Once", 0, 2, AnimationType::Once);
    game.sprites.set_animation_delay(sprite, once, 3);
    game.sprites.set_initial_animation(sprite, once);
    game.sprites
        .set_animation_callback(sprite, once, "RestartOnce");
    let calls = game.vars.create_u8("wCalls", 0);
    let mut callback = Block::new();
    callback
        .ld_a_addr_def(calls.name())
        .inc(R8::A)
        .ld_addr_def_a(calls.name())
        .cp(2)
        .jr_cond(Condition::NC, ".end")
        .emit_all(game.sprites.restart_animation(sprite, once))
        .label(".end");
    game.define_function_from("RestartOnce", callback);
    let rom = engine_rom(&game);
    let mut gb = rom.boot();
    gb.run_frames(40);
    assert_eq!(gb.read_symbol(calls.name()), 2);
    assert_eq!(gb.oam_entry(0)[2], 2);
    gb.run_frames(10);
    assert_eq!(gb.read_symbol(calls.name()), 2);
}

#[test]
fn typed_sprite_flags_reach_oam_and_runtime_changes_preserve_position() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    let mut expected = Vec::new();
    for bits in 0..16 {
        let flags = SpriteFlags::new()
            .flip_x(bits & 1 != 0)
            .flip_y(bits & 2 != 0)
            .palette(if bits & 4 != 0 {
                ObjectPalette::Palette1
            } else {
                ObjectPalette::Palette0
            })
            .priority(if bits & 8 != 0 {
                SpritePriority::BehindBackground
            } else {
                SpritePriority::AboveBackground
            });
        let id = game.add_sprite(
            &format!("FlagSprite{bits}"),
            TileSource::from_raw(&[["$ff"; 8]]),
            24,
            32,
            flags,
        );
        let changed =
            SpriteFlags::from(flags.bits() ^ (hw::OAMF_XFLIP.value | hw::OAMF_YFLIP.value));
        game.on_vblank(game.sprites.set_flags(id, changed));
        expected.push((flags.bits(), changed.bits()));
    }
    let mut gb = engine_rom(&game).boot();
    assert!(gb.run_until_pc(gb.symbol("Main"), 10));
    for (index, (initial, _)) in expected.iter().enumerate() {
        assert_eq!(
            gb.oam_entry(index),
            [
                32 + hw::OAM_Y_OFFSET,
                24 + hw::OAM_X_OFFSET,
                index as u8,
                *initial
            ]
        );
    }
    gb.run_frames(3);
    for (index, (_, changed)) in expected.iter().enumerate() {
        assert_eq!(
            gb.oam_entry(index),
            [
                32 + hw::OAM_Y_OFFSET,
                24 + hw::OAM_X_OFFSET,
                index as u8,
                *changed
            ]
        );
    }
}

#[test]
fn random_u8_advances_a_nonzero_state_and_returns_it_in_a() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    game.vars.create_u8("wRandomOutput", 0);
    let random = rust_boy::stdlib::random::random_u8();
    let mut call = game.call_routine(&random);
    let mut store = rust_boy::asm::Block::new();
    store.ld_addr_def_a("wRandomOutput");
    call.extend(store.into_instrs());
    game.add_to_main_loop(call);

    let mut gb = engine_rom(&game).boot();
    assert!(gb.run_until_pc(gb.symbol("Main"), 10));
    gb.run_frames(3);
    let seed = gb.read_symbol("wRandomSeed");
    assert_ne!(
        seed, 0,
        "a zero initial state is replaced by the default seed"
    );
    assert_eq!(gb.read_symbol("wRandomOutput"), seed);
    assert_eq!(seed, 0xDE, "three LFSR steps from the default $A7 seed");
}

#[test]
fn random_u8_cycles_custom_seeds_and_recovers_a_runtime_zero() {
    if !rgbds_enabled() {
        return;
    }
    for seed in [1, 0x42, 0xA7] {
        let mut game = RustBoy::new();
        game.vars.create_u8("wRandomSeed", seed);
        let call = game.call_routine(&rust_boy::stdlib::random::random_u8());
        game.add_to_main_loop(call);
        let mut gb = engine_rom(&game).boot();
        assert!(gb.run_until_pc(gb.symbol("Main"), 10));
        assert_eq!(gb.read_symbol("wRandomSeed"), seed);
        let mut seen = std::collections::BTreeSet::new();
        seen.insert(seed);
        for step in 1..=255 {
            gb.run_frames(1);
            let state = gb.read_symbol("wRandomSeed");
            assert_ne!(state, 0);
            if step < 255 {
                assert!(seen.insert(state), "seed {seed}: repeated at step {step}");
            } else {
                assert_eq!(state, seed);
            }
        }
        assert_eq!(seen.len(), 255);
        gb.write_ram(gb.symbol("wRandomSeed"), 0);
        gb.run_frames(1);
        assert_eq!(
            gb.read_symbol("wRandomSeed"),
            0xEB,
            "default seed is stepped after runtime zero"
        );
    }
}

#[test]
fn palettes_and_nonblocking_fades_follow_progress_in_vblank() {
    if !rgbds_enabled() {
        return;
    }
    for solid in [Shade::White, Shade::Black] {
        for fade_in in [true, false] {
            let mut game = RustBoy::new();
            let bg = Palette::from(0xe4);
            let ob0 = Palette::from(0x1b);
            let ob1 = Palette::from(0x72);
            let fades = [
                (PaletteRegister::Background, bg),
                (PaletteRegister::Object0, ob0),
                (PaletteRegister::Object1, ob1),
            ]
            .map(|(register, palette)| {
                (
                    register,
                    if fade_in {
                        PaletteFade::fade_in(palette, solid)
                    } else {
                        PaletteFade::fade_out(palette, solid)
                    },
                )
            });
            let progress = game.vars.create_u8("wPaletteProgress", 0);
            let mut code = Block::new();
            for (register, fade) in fades {
                code.emit_all(progress.get())
                    .emit_all(game.fade_palette(register, fade));
            }
            game.on_vblank(code);
            let mut gb = engine_rom(&game).boot();
            assert!(gb.run_until_pc(gb.symbol("Main"), 5));
            for step in [0, 1, 2, 3, 4, 255] {
                gb.write_ram(gb.symbol("wPaletteProgress"), step);
                gb.run_frames(1);
                for (register, fade) in fades {
                    assert_eq!(gb.read(register.register().value), fade.at(step).byte());
                }
            }
        }
    }
}

#[test]
fn runtime_palette_writes_override_startup_independently() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    game.init(game.set_palette(PaletteRegister::Background, Palette::from(0x1b)));
    game.on_vblank(game.set_palette(PaletteRegister::Object1, Palette::solid(Shade::Dark)));
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(2);
    assert_eq!(gb.read(hw::BGP.value), 0x1b);
    assert_eq!(gb.read(hw::OBP0.value), Palettes::IDENTITY);
    assert_eq!(gb.read(hw::OBP1.value), 0xaa);
}

#[test]
fn rom_tilemaps_copy_multiple_levels_to_either_map_at_runtime() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    let first = game.tiles.add_tilemap_rom("LevelOne", &[[11; 32]; 32]);
    let second = game.tiles.add_tilemap_rom("LevelTwo", &[[29; 32]; 32]);
    let empty = game.tiles.add_tilemap_rom("EmptyLevel", &[]);
    assert_eq!(game.tiles.get_address(first), None);
    let ready = game.vars.create_u8("wLevelReady", 0);
    let mut load = Block::new();
    load.call("WaitVBlank")
        .emit_all(rust_boy::stdlib::graphics::utility::lcd_off())
        .emit_all(game.tiles.copy_tilemap(first, TilemapArea::Map9800))
        .emit_all(game.tiles.copy_tilemap(second, TilemapArea::Map9C00))
        .emit_all(game.tiles.copy_tilemap(empty, TilemapArea::Map9800))
        .emit_all(rust_boy::stdlib::graphics::utility::lcd_on())
        .emit_all(ready.set(1));
    game.add_to_main_loop(ready.get());
    game.add_to_main_loop(IfA::eq(0, load.into_instrs()));
    let mut gb = engine_rom(&game).boot();
    assert!(gb.run_until_pc(gb.symbol("Main"), 5));
    // ROM-only maps have not been copied by startup.
    assert!(
        gb.vram()[usize::from(hw::SCRN0.value - hw::VRAM.value)..]
            .iter()
            .all(|&b| b == 0)
    );
    gb.run_frames(8);
    assert_eq!(gb.read_symbol("wLevelReady"), 1);
    for (area, value) in [(TilemapArea::Map9800, 11), (TilemapArea::Map9C00, 29)] {
        let offset = usize::from(area.address() - hw::VRAM.value);
        assert!(gb.vram()[offset..offset + 1024].iter().all(|&b| b == value));
    }
    assert_eq!(gb.blocked_accesses().0, 0);
}

#[test]
fn rom_tilemap_copy_keeps_a_configured_lcdc_and_short_maps_keep_trailing_rows() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::with_config(
        RustBoyConfig::default()
            .background_tilemap(TilemapArea::Map9C00)
            .sprite_size(SpriteSize::Size8x16)
            .window(Window::new(TilemapArea::Map9800, 0, 112)),
    );
    let full = game.tiles.add_tilemap_rom("FullLevel", &[[11; 32]; 32]);
    let short = game.tiles.add_tilemap_rom("ShortLevel", &[[29; 32]; 2]);
    let ready = game.vars.create_u8("wLevelReady", 0);
    // The LCDC value of this configuration, without any runtime copy
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(2);
    let lcdc = gb.read(hw::LCDC.value);
    for flag in [
        hw::LCDCF_ON,
        hw::LCDCF_BG9C00,
        hw::LCDCF_OBJ16,
        hw::LCDCF_WINON,
    ] {
        assert_ne!(lcdc & flag.value, 0, "{}", flag.name);
    }
    let mut load = Block::new();
    load.call("WaitVBlank")
        .emit_all(rust_boy::stdlib::graphics::utility::lcd_off())
        .emit_all(game.tiles.copy_tilemap(full, TilemapArea::Map9C00))
        .emit_all(game.tiles.copy_tilemap(short, TilemapArea::Map9C00))
        .emit_all(rust_boy::stdlib::graphics::utility::lcd_on())
        .emit_all(ready.set(1));
    game.add_to_main_loop(ready.get());
    game.add_to_main_loop(IfA::eq(0, load.into_instrs()));
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(8);
    assert_eq!(gb.read_symbol("wLevelReady"), 1);
    assert_eq!(gb.read(hw::LCDC.value), lcdc);
    // The short map overwrote its 2 rows; the 30 rows after them keep the full level
    let map = usize::from(hw::SCRN1.value - hw::VRAM.value);
    assert!(gb.vram()[map..map + 64].iter().all(|&b| b == 29));
    assert!(gb.vram()[map + 64..map + 1024].iter().all(|&b| b == 11));
    assert_eq!(gb.blocked_accesses().0, 0);
}

#[test]
fn window_startup_and_vblank_preserve_other_lcdc_bits() {
    if !rgbds_enabled() {
        return;
    }
    for background in [TilemapArea::Map9800, TilemapArea::Map9C00] {
        for window in [TilemapArea::Map9800, TilemapArea::Map9C00] {
            let mut game = RustBoy::with_config(
                RustBoyConfig::default()
                    .background_tilemap(background)
                    .window(Window::new(window, 0, 112)),
            );
            game.tiles.add_tilemap_at("WindowMap", window, &[[42; 32]]);
            let mut gb = engine_rom(&game).boot();
            gb.run_frames(2);
            assert_eq!(gb.read(hw::WX.value), 7);
            assert_eq!(gb.read(hw::WY.value), 112);
            let lcdc = gb.read(hw::LCDC.value);
            assert_ne!(lcdc & hw::LCDCF_WINON.value, 0);
            assert_eq!(
                lcdc & hw::LCDCF_WIN9C00.value,
                if window == TilemapArea::Map9C00 {
                    64
                } else {
                    0
                }
            );
            assert_eq!(
                lcdc & hw::LCDCF_BG9C00.value,
                if background == TilemapArea::Map9C00 {
                    8
                } else {
                    0
                }
            );
            assert_eq!(
                gb.vram()[usize::from(window.address() - hw::VRAM.value)],
                42
            );
            let mut code = Block::new();
            code.emit_all(game.show_window(false))
                .emit_all(game.set_window_position(158, 143))
                .emit_all(
                    game.select_window_tilemap(if window == TilemapArea::Map9800 {
                        TilemapArea::Map9C00
                    } else {
                        TilemapArea::Map9800
                    }),
                );
            game.on_vblank(code);
            let mut gb = engine_rom(&game).boot();
            gb.run_frames(2);
            assert_eq!(gb.read(hw::WX.value), 165);
            assert_eq!(gb.read(hw::WY.value), 143);
            assert_eq!(
                gb.read(hw::LCDC.value),
                (lcdc & !hw::LCDCF_WINON.value) ^ hw::LCDCF_WIN9C00.value
            );
            // Appended after the code above: the window is shown again, on the switched map
            game.on_vblank(game.show_window(true));
            let mut gb = engine_rom(&game).boot();
            gb.run_frames(2);
            assert_eq!(gb.read(hw::WX.value), 165);
            assert_eq!(gb.read(hw::WY.value), 143);
            assert_eq!(gb.read(hw::LCDC.value), lcdc ^ hw::LCDCF_WIN9C00.value);
        }
    }
}

#[test]
fn window_init_position_override_and_disabled_default() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(2);
    assert_eq!(gb.read(hw::LCDC.value) & hw::LCDCF_WINON.value, 0);
    game.set_window(Some(Window::new(TilemapArea::Map9C00, 0, 0)));
    game.init(game.set_window_position(40, 80));
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(2);
    assert_eq!(gb.read(hw::WX.value), 47);
    assert_eq!(gb.read(hw::WY.value), 80);
}

#[test]
fn get_tile_by_pixel_follows_active_background_map() {
    if !rgbds_enabled() {
        return;
    }
    for map in [TilemapArea::Map9800, TilemapArea::Map9C00] {
        for (x, y) in [(0u8, 0u8), (7, 7), (8, 8), (159, 143), (255, 255)] {
            let mut game = RustBoy::with_config(RustBoyConfig::default().background_tilemap(map));
            game.tiles
                .add_tilemap_at("FirstMap", TilemapArea::Map9800, &[[17; 32]; 32]);
            game.tiles
                .add_tilemap_at("SecondMap", TilemapArea::Map9C00, &[[93; 32]; 32]);
            let tile = game.vars.create_u8("wTile", 0);
            let address = game.vars.create_u16("wAddress", 0);
            let second = game.vars.create_u8("wSecondTile", 0);
            let second_address = game.vars.create_u16("wSecondAddress", 0);
            let mut code = Block::new();
            code.ld(R8::B, x)
                .ld(R8::C, y)
                .call("GetTileByPixel")
                .ld_addr_def_a(tile.name())
                .ld(R8::A, R8::L)
                .ld_addr_def_a(address.name())
                .ld(R8::A, R8::H)
                .ld(Mem::addr(Expr::sym(address.name()) + 1), R8::A);
            // Switch map in the same handler: lookup follows the hardware, not config.
            code.ld_a_addr_def(hw::LCDC)
                .xor(hw::LCDCF_BG9C00)
                .ld_addr_def_a(hw::LCDC)
                .ld(R8::B, x)
                .ld(R8::C, y)
                .call("GetTileByPixel")
                .ld_addr_def_a(second.name())
                .ld(R8::A, R8::L)
                .ld_addr_def_a(second_address.name())
                .ld(R8::A, R8::H)
                .ld(Mem::addr(Expr::sym(second_address.name()) + 1), R8::A)
                .ld_a_addr_def(hw::LCDC)
                .xor(hw::LCDCF_BG9C00)
                .ld_addr_def_a(hw::LCDC);
            game.on_vblank(code);
            let mut gb = engine_rom(&game).boot();
            assert!(gb.run_until_pc(gb.symbol("Main"), 10));
            gb.run_frames(2);
            assert_eq!(
                gb.read_symbol("wTile"),
                if map == TilemapArea::Map9800 { 17 } else { 93 }
            );
            assert_eq!(
                gb.read_symbol("wSecondTile"),
                if map == TilemapArea::Map9800 { 93 } else { 17 }
            );
            let stored = gb.symbol("wAddress");
            let actual = u16::from_le_bytes([gb.read(stored), gb.read(stored + 1)]);
            let offset = u16::from(y / 8) * 32 + u16::from(x / 8);
            assert_eq!(actual, map.address() + offset);
            let other = if map == TilemapArea::Map9800 {
                TilemapArea::Map9C00
            } else {
                TilemapArea::Map9800
            };
            let stored = gb.symbol("wSecondAddress");
            let actual = u16::from_le_bytes([gb.read(stored), gb.read(stored + 1)]);
            assert_eq!(actual, other.address() + offset);
            assert_eq!(gb.blocked_accesses().0, 0);
        }
    }
}
#[test]
fn png_and_binary_tile_sources_load_exact_bytes_into_vram() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::with_config(RustBoyConfig::default().sprite_size(SpriteSize::Size8x16));
    let source = TileSource::from_png(
        support::examples::root().join("tests/fixtures/png/adam7-rgb.png"),
        &PngOptions::default(),
    )
    .unwrap();
    assert_eq!(source.tile_count(), 1);
    let background = game.tiles.add_background("PngBackground", source);
    let binary =
        TileSource::from_2bpp(support::examples::root().join("examples/coin-anim/coin.2bpp"))
            .unwrap();
    assert_eq!(binary.tile_count(), 7);
    let binary_id = game.tiles.add_background("BinaryBackground", binary);
    let player = game.add_sprite(
        "PngSprite",
        TileSource::from_png(
            support::examples::root().join("examples/fosdem/char-dx.png"),
            &PngOptions::new([[255; 3], [207; 3], [104; 3], [0, 0, 1]])
                .tile_order(TileOrder::Sprite8x16),
        )
        .unwrap(),
        80,
        72,
        0,
    );
    let base = game.tiles.get_address(background).unwrap();
    let binary_base = game.tiles.get_address(binary_id).unwrap();
    let asm = game.build().unwrap();
    assert_eq!(asm, game.build().unwrap());
    assert!(!asm.contains("INCBIN"));
    let rom = Rom::build(&asm, &[]);
    let mut gb = rom.boot();
    gb.run_frames(15);
    let expected = [
        0x55, 0x33, 0xaa, 0x66, 0x55, 0xcc, 0xaa, 0x99, 0x55, 0x33, 0xaa, 0x66, 0x55, 0xcc, 0xaa,
        0x99,
    ];
    for (i, byte) in expected.into_iter().enumerate() {
        assert_eq!(gb.read(base + i as u16), byte);
    }
    for (i, &byte) in include_bytes!("../examples/coin-anim/coin.2bpp")
        .iter()
        .enumerate()
    {
        assert_eq!(gb.read(binary_base + i as u16), byte);
    }
    for (i, &byte) in include_bytes!("../examples/fosdem/char-dx.2bpp")
        .iter()
        .enumerate()
    {
        assert_eq!(gb.read(hw::VRAM_OBJ_TILES + i as u16), byte);
    }
    assert_eq!(gb.oam_entry(0), [88, 88, 0, 0]);
    let _ = player;
    assert_eq!(gb.blocked_accesses().0, 0);
}

#[test]
fn engine_audio_power_and_mixer_are_configured_before_user_init() {
    if !rgbds_enabled() {
        return;
    }
    for (config, power, volume, routing) in [
        (AudioConfig::Off, 0, None, None),
        (
            AudioConfig::On(Mixer::default()),
            0x80,
            Some(0x77),
            Some(0xff),
        ),
        (
            AudioConfig::On(Mixer::new(2, 4, hw::AUDTERM_2_LEFT.value)),
            0x80,
            Some(0x24),
            Some(0x20),
        ),
    ] {
        let game = RustBoy::with_config(RustBoyConfig::default().audio(config));
        let rom = engine_rom(&game);
        let mut gb = rom.boot();
        gb.run_frames(3);
        // This emulator stores audio IO bytes but does not synthesize/clock an APU.
        assert_eq!(gb.read(hw::NR52.value) & hw::AUDENA_ON.value, power);
        if let Some(volume) = volume {
            assert_eq!(gb.read(hw::NR50.value), volume);
        }
        if let Some(routing) = routing {
            assert_eq!(gb.read(hw::NR51.value), routing);
        }
    }
    let mut game =
        RustBoy::with_config(RustBoyConfig::default().audio(AudioConfig::On(Mixer::default())));
    game.init(rust_boy::stdlib::audio::set_mixer(Mixer::new(1, 2, 0)));
    let rom = engine_rom(&game);
    let mut gb = rom.boot();
    gb.run_frames(3);
    assert_eq!(gb.read(hw::NR50.value), 0x12);
    assert_eq!(gb.read(hw::NR51.value), 0);
}

#[test]
fn engine_audio_configuration_is_reapplied_on_soft_reset() {
    if !rgbds_enabled() {
        return;
    }
    let mixer = Mixer::new(2, 4, hw::AUDTERM_2_LEFT.value);
    let game = RustBoy::with_config(
        RustBoyConfig::default()
            .audio(AudioConfig::On(mixer))
            .soft_reset(true),
    );
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(3);
    assert_eq!(gb.read(hw::NR52.value) & hw::AUDENA_ON.value, 0x80);
    assert_eq!(gb.read(hw::NR50.value), 0x24);
    assert_eq!(gb.read(hw::NR51.value), hw::AUDTERM_2_LEFT.value);

    gb.write_io(hw::NR50.value, 0);
    assert_eq!(gb.read(hw::NR50.value), 0);
    gb.run_script(&[(3, &[Button::A, Button::B, Button::Select, Button::Start])]);

    assert_eq!(gb.read(hw::NR52.value) & hw::AUDENA_ON.value, 0x80);
    assert_eq!(gb.read(hw::NR50.value), 0x24);
    assert_eq!(gb.read(hw::NR51.value), hw::AUDTERM_2_LEFT.value);
}

#[test]
fn sound_effects_program_all_channels_without_changing_power_or_mixer() {
    if !rgbds_enabled() {
        return;
    }
    let pattern = WavePattern::from_samples(std::array::from_fn(|i| (i % 16) as u8));
    let envelope = Envelope::new(10, EnvelopeDirection::Decrease, 2);
    let pulse = PulseEffect::new(0x345, 32, Duty::Quarter, envelope);
    let effects = [
        Sfx::Pulse1(pulse, Sweep::new(3, SweepDirection::Subtract, 2)),
        Sfx::Pulse2(pulse),
        Sfx::Wave(WaveEffect::new(0x456, 256, WaveLevel::Full, pattern)),
        Sfx::Noise(NoiseEffect::new(64, envelope, 5, NoiseWidth::Seven, 3)),
    ];
    let mut game = RustBoy::with_config(
        RustBoyConfig::default().audio(AudioConfig::On(Mixer::new(2, 3, 0xff))),
    );
    let mut init = Block::new();
    // Replacements use fresh local labels and upload wave RAM twice safely.
    for effect in effects.iter().cycle().take(8) {
        init.emit_all(game.play_sfx(effect));
    }
    game.init(init.into_instrs());
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(3);
    // The emulator stores IO bytes, not APU playback. This verifies linked snippets
    // executing on SM83, addressing and channel isolation; unit traces check ordering.
    for (register, value) in [
        (hw::NR52, 0x80),
        (hw::NR50, 0x23),
        (hw::NR51, 0xff),
        (hw::NR10, 0x3a),
        (hw::NR11, 0x60),
        (hw::NR12, 0xa2),
        (hw::NR13, 0x45),
        (hw::NR14, 0xc3),
        (hw::NR21, 0x60),
        (hw::NR22, 0xa2),
        (hw::NR23, 0x45),
        (hw::NR24, 0xc3),
        (hw::NR30, 0x80),
        (hw::NR31, 0),
        (hw::NR32, 0x20),
        (hw::NR33, 0x56),
        (hw::NR34, 0xc4),
        (hw::NR41, 0),
        (hw::NR42, 0xa2),
        (hw::NR43, 0x5b),
        (hw::NR44, 0xc0),
    ] {
        assert_eq!(gb.read(register.value), value, "{}", register.name);
    }
    for (i, &value) in pattern.bytes().iter().enumerate() {
        assert_eq!(gb.read(hw::AUD3WAVERAM.value + i as u16), value);
    }
    // A runtime attempt while off returns carry without touching even writable wave RAM.
    let mut game = RustBoy::with_config(RustBoyConfig::default().audio(AudioConfig::Off));
    let mut init = Block::new();
    for i in 0..16 {
        init.ld_a(0xab)
            .ldh(Mem::addr(Expr::from(hw::AUD3WAVERAM) + i), R8::A);
    }
    init.emit_all(game.play_sfx(&effects[2]));
    // Stop before any flag-changing instruction, then inspect carry.
    init.label("SfxRejected").jr("SfxRejected");
    game.init(init.into_instrs());
    let mut gb = engine_rom(&game).boot();
    assert!(gb.run_until_pc(gb.symbol("SfxRejected"), 3));
    assert_ne!(gb.registers().f & 0x10, 0);
    for i in 0..16 {
        assert_eq!(gb.read(hw::AUD3WAVERAM.value + i), 0xab);
    }
}

// hUGEDriver's pinned WRAM offsets (verified against driver()'s generated constants).
const MUSIC_WAVE: u16 = 28;
const MUSIC_MASK: u16 = 29;
const MUSIC_TICK: u16 = 31;
const MUSIC_ROW: u16 = 34;
fn music_byte(gb: &GameBoy, offset: u16) -> u8 {
    gb.read(gb.symbol(rust_boy::stdlib::audio::music::BUFFER) + offset)
}
fn music_game(clock: MusicClock) -> RustBoy {
    let export = include_str!("../examples/music/song.asm");
    let first = Song::from_rgbds(export, "DemoSong");
    let second = Song::from_rgbds(
        &export
            .replace("DemoSong", "SecondSong")
            .replacen("    db 2", "    db 1", 1)
            .replace("dn ___, 0, 0", "dn C_4, 1, 0"),
        "SecondSong",
    );
    let mut game = RustBoy::new();
    game.enable_music(first, clock).add_music_song(second);
    game.vars.create_u8("wMusicCommand", 0);
    game.vars.create_u8("wMusicResult", 0);
    let effect = Sfx::Wave(WaveEffect::new(
        1000,
        16,
        WaveLevel::Half,
        WavePattern::from_bytes([0xaa; 16]),
    ));
    let unreserved = Sfx::Pulse2(PulseEffect::new(
        0,
        1,
        Duty::Half,
        Envelope::new(1, EnvelopeDirection::Decrease, 0),
    ));
    let mut code = Block::new();
    code.label("MusicFrame")
        .ld(R8::A, Mem::addr("wMusicCommand"))
        .and(R8::A)
        .jr_cond(Condition::Z, ".done");
    let branches = [
        game.pause_music(),
        game.resume_music(),
        game.stop_music(),
        game.start_music("SecondSong"),
        [
            game.reserve_music_channel(Channel::Wave),
            game.play_sfx(&effect),
        ]
        .concat(),
        game.release_music_channel(Channel::Wave),
        game.play_sfx(&unreserved),
        rust_boy::stdlib::audio::power_off(),
        rust_boy::stdlib::audio::power_on(Mixer::default()),
    ];
    for (i, branch) in branches.into_iter().enumerate() {
        let skip = format!(".command_skip_{i}");
        code.cp_imm(i as u8 + 1)
            .jr_cond(Condition::NZ, &skip)
            .emit_all(branch)
            .push(R16Stack::AF)
            .pop(R16Stack::BC)
            .ld(R8::A, R8::C)
            .ld(Mem::addr("wMusicResult"), R8::A)
            .xor(R8::A)
            .ld(Mem::addr("wMusicCommand"), R8::A)
            .jr(".done")
            .label(&skip);
    }
    code.label(".done").label("MusicFrameDone").nop();
    game.add_to_main_loop(code.into_instrs());
    game
}
fn music_frame(gb: &mut GameBoy) {
    gb.run_until_cycle(gb.cycles() + 1);
    assert!(gb.run_until_pc(gb.symbol("MusicFrameDone"), 3));
}
fn music_command(gb: &mut GameBoy, command: u8) {
    gb.write_ram(gb.symbol("wMusicCommand"), command);
    music_frame(gb);
    assert_eq!(gb.read_symbol("wMusicCommand"), 0);
}
#[test]
fn huge_music_advances_pauses_stops_switches_and_arbitrates_wave_sfx() {
    if !rgbds_enabled() {
        return;
    }
    for clock in [MusicClock::MainFrame, MusicClock::VBlank] {
        let mut gb = engine_rom(&music_game(clock)).boot();
        assert!(gb.run_until_pc(gb.symbol("MusicFrameDone"), 3));
        assert_eq!(music_byte(&gb, MUSIC_TICK), 1);
        assert_eq!(music_byte(&gb, MUSIC_ROW), 0);
        let first_period = (gb.read(hw::NR14.value) & 7, gb.read(hw::NR13.value));
        let first_wave = gb.read(hw::NR33.value);
        music_frame(&mut gb);
        music_frame(&mut gb);
        assert_eq!(music_byte(&gb, MUSIC_ROW), 1);
        assert_ne!(
            first_period,
            (gb.read(hw::NR14.value) & 7, gb.read(hw::NR13.value))
        );
        assert_ne!(first_wave, gb.read(hw::NR33.value));
        assert_ne!(gb.read(hw::NR44.value) & 0x80, 0);
        assert_eq!(music_byte(&gb, MUSIC_WAVE), 0);
        music_command(&mut gb, 7); // Playback without reserving CH2 rejects.
        assert_ne!(gb.read_symbol("wMusicResult") & 0x10, 0);
        music_command(&mut gb, 1); // Pause freezes song position and cuts its channels.
        let position = (music_byte(&gb, MUSIC_ROW), music_byte(&gb, MUSIC_TICK));
        gb.run_frames(5);
        assert_eq!(
            position,
            (music_byte(&gb, MUSIC_ROW), music_byte(&gb, MUSIC_TICK))
        );
        assert_eq!(gb.read(hw::NR12.value), 0);
        assert_eq!(gb.read(hw::NR30.value), 0);
        music_command(&mut gb, 2);
        assert_eq!(gb.read_symbol("wMusicResult") & 0x10, 0);
        music_frame(&mut gb);
        assert_ne!(
            position,
            (music_byte(&gb, MUSIC_ROW), music_byte(&gb, MUSIC_TICK))
        );
        music_command(&mut gb, 3);
        music_command(&mut gb, 2);
        assert_ne!(gb.read_symbol("wMusicResult") & 0x10, 0);
        assert_eq!(gb.read_symbol(rust_boy::stdlib::audio::music::PLAYING), 0);
        music_command(&mut gb, 4); // Start another imported song (one tick per row).
        assert_eq!(music_byte(&gb, MUSIC_ROW), 0);
        assert_eq!(music_byte(&gb, MUSIC_TICK), 0);
        music_frame(&mut gb);
        assert_eq!(music_byte(&gb, MUSIC_ROW), 1);
        music_command(&mut gb, 5);
        assert_eq!(music_byte(&gb, MUSIC_MASK), 4);
        assert_eq!(gb.read_symbol("wMusicResult") & 0x10, 0);
        for i in 0..16 {
            assert_eq!(gb.read(hw::AUD3WAVERAM.value + i), 0xaa);
        }
        music_frame(&mut gb);
        music_frame(&mut gb);
        for i in 0..16 {
            assert_eq!(gb.read(hw::AUD3WAVERAM.value + i), 0xaa);
        }
        music_command(&mut gb, 1); // Pausing music leaves the reserved SFX alone.
        assert_eq!(gb.read(hw::NR30.value), 0x80);
        assert_eq!(gb.read(hw::NR32.value), 0x40);
        music_command(&mut gb, 2);
        music_command(&mut gb, 6);
        assert_eq!(music_byte(&gb, MUSIC_MASK), 0);
        assert_eq!(music_byte(&gb, MUSIC_WAVE), 100);
        music_frame(&mut gb);
        assert_eq!(music_byte(&gb, MUSIC_WAVE), 0);
        assert_eq!(gb.read(hw::AUD3WAVERAM.value), 0x01);
        music_command(&mut gb, 8); // Power off gates driver ticks without losing position.
        let position = (music_byte(&gb, MUSIC_ROW), music_byte(&gb, MUSIC_TICK));
        gb.run_frames(4);
        assert_eq!(
            position,
            (music_byte(&gb, MUSIC_ROW), music_byte(&gb, MUSIC_TICK))
        );
        music_command(&mut gb, 9);
        music_frame(&mut gb);
        assert_ne!(
            position,
            (music_byte(&gb, MUSIC_ROW), music_byte(&gb, MUSIC_TICK))
        );
    }
}

#[test]
fn huge_vblank_tick_skips_initialization_and_preserves_the_interrupted_registers() {
    if !rgbds_enabled() {
        return;
    }
    let mut gb = engine_rom(&music_game(MusicClock::VBlank)).boot();
    assert!(gb.run_until_pc(gb.symbol("MusicFrameDone"), 3));
    gb.write_ram(gb.symbol("wMusicCommand"), 4);
    gb.run_until_cycle(gb.cycles() + 1);
    assert!(gb.run_until_pc(
        gb.symbol("RustBoyHuge_hUGE_init.copy_song_descriptor_loop"),
        3
    ));
    assert_eq!(gb.read_symbol(rust_boy::stdlib::audio::music::PLAYING), 0);
    let before = gb.registers();
    gb.request_interrupt(hw::IEF_VBLANK.value);
    gb.run_until_cycle(gb.cycles() + 1);
    assert!(gb.run_until_pc(before.pc, 1));
    let after = gb.registers();
    assert_eq!(
        (
            before.af(),
            before.bc(),
            before.de(),
            before.hl(),
            before.sp
        ),
        (after.af(), after.bc(), after.de(), after.hl(), after.sp)
    );
    assert!(gb.run_until_pc(gb.symbol("MusicFrameDone"), 3));
    assert_eq!(music_byte(&gb, MUSIC_ROW), 0);
    assert_eq!(music_byte(&gb, MUSIC_TICK), 0);
    let order1 = gb.read16(gb.symbol(rust_boy::stdlib::audio::music::BUFFER) + 1);
    assert_eq!(order1, gb.symbol("RustBoySong_SecondSong_order1"));
}

#[test]
fn huge_music_coexists_with_full_vram_queue_dma_and_soft_reset() {
    if !rgbds_enabled() {
        return;
    }
    for clock in [MusicClock::MainFrame, MusicClock::VBlank] {
        let mut game = music_game(clock);
        game.set_soft_reset(true);
        game.add_sprite(
            "MusicDot",
            TileSource::from_raw(&[["`33333333"; 8]]),
            8,
            8,
            0,
        );
        let mut writes = Block::new();
        for i in 0..vram_queue::CAPACITY {
            writes.emit_all(game.queue_vram_write(hw::SCRN0.value + u16::from(i), i + 1));
        }
        game.add_to_main_loop(writes.into_instrs());
        let rom = engine_rom(&game);
        let mut gb = rom.boot();
        gb.run_frames(8);
        let music = gb.symbol(rust_boy::stdlib::audio::music::BUFFER);
        for buffer in [
            vram_queue::BUFFER,
            vram_queue::READ,
            vram_queue::WRITE,
            "wShadowOAM",
        ] {
            assert!(music + 100 <= gb.symbol(buffer));
        }
        assert_eq!(gb.symbol("wShadowOAM"), hw::RAM.value + 0xf00);
        for i in 0..vram_queue::CAPACITY {
            assert_eq!(gb.read(hw::SCRN0.value + u16::from(i)), i + 1);
        }
        for dma in gb.oam_dmas().iter().filter(|d| d.lcd_on) {
            assert_eq!(dma.first.1, Mode::VBlank);
            assert_eq!(dma.last.unwrap().1, Mode::VBlank);
        }
        assert_eq!(gb.dma_conflicts(), 0);
        assert_eq!(gb.blocked_accesses().0, 0);
        // Select the second song, then enter the soft-reset chord. Stop exactly before
        // main starts again to inspect the fresh first-song state and mixer.
        music_command(&mut gb, 4);
        for button in [Button::A, Button::B, Button::Start, Button::Select] {
            gb.press(button);
        }
        assert!(gb.run_until_pc(gb.symbol("EntryPoint"), 3));
        gb.run_until_cycle(gb.cycles() + 1);
        assert!(gb.run_until_pc(gb.symbol("Main"), 3));
        assert_eq!(music_byte(&gb, MUSIC_ROW), 0);
        assert_eq!(music_byte(&gb, MUSIC_TICK), 0);
        assert_eq!(
            gb.read16(gb.symbol(rust_boy::stdlib::audio::music::BUFFER) + 1),
            gb.symbol("RustBoySong_DemoSong_order1")
        );
        assert_eq!(gb.read(hw::NR50.value), 0x77);
        assert_eq!(gb.read(hw::NR51.value), 0xff);
    }
}

#[test]
fn starting_an_empty_song_silences_previous_sustained_notes() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = music_game(MusicClock::MainFrame);
    let mut empty = include_str!("../examples/music/song.asm").replace("DemoSong", "EmptySong");
    for note in ["C_5", "E_5", "C_4", "E_4", "C_3", "D_3"] {
        empty = empty.replace(&format!("dn {note}, 1, 0"), "dn ___, 0, 0");
    }
    game.add_music_song(Song::from_rgbds(&empty, "EmptySong"));
    let mut code = Block::new();
    code.ld(R8::A, Mem::addr("wMusicResult"))
        .cp_imm(0x42)
        .jr_cond(Condition::NZ, ".skip_empty")
        .emit_all(game.start_music("EmptySong"))
        .label(".skip_empty");
    game.add_to_main_loop(code.into_instrs());
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(3);
    assert_ne!(gb.read(hw::NR12.value), 0);
    gb.write_ram(gb.symbol("wMusicResult"), 0x42);
    gb.run_frames(2);
    for dac in [hw::NR12, hw::NR22, hw::NR30, hw::NR42] {
        assert_eq!(gb.read(dac.value), 0);
    }
}

#[test]
fn huge_reservation_protects_sound_effect_registers_on_each_channel() {
    if !rgbds_enabled() {
        return;
    }
    let envelope = Envelope::new(5, EnvelopeDirection::Decrease, 3);
    let pulse = PulseEffect::new(1234, 11, Duty::ThreeQuarters, envelope);
    let cases = [
        (
            Sfx::Pulse1(pulse, Sweep::default()),
            1,
            vec![
                (hw::NR11, 0xf5),
                (hw::NR12, 0x53),
                (hw::NR13, 0xd2),
                (hw::NR14, 0xc4),
            ],
        ),
        (
            Sfx::Pulse2(pulse),
            2,
            vec![
                (hw::NR21, 0xf5),
                (hw::NR22, 0x53),
                (hw::NR23, 0xd2),
                (hw::NR24, 0xc4),
            ],
        ),
        (
            Sfx::Wave(WaveEffect::new(
                1234,
                11,
                WaveLevel::Quarter,
                WavePattern::from_bytes([0x99; 16]),
            )),
            4,
            vec![
                (hw::NR30, 0x80),
                (hw::NR31, 245),
                (hw::NR32, 0x60),
                (hw::NR33, 0xd2),
                (hw::NR34, 0xc4),
            ],
        ),
        (
            Sfx::Noise(NoiseEffect::new(11, envelope, 7, NoiseWidth::Seven, 3)),
            8,
            vec![
                (hw::NR41, 53),
                (hw::NR42, 0x53),
                (hw::NR43, 0x7b),
                (hw::NR44, 0xc0),
            ],
        ),
    ];
    for (effect, mask, registers) in cases {
        let mut game = RustBoy::new();
        game.enable_music(
            Song::from_rgbds(include_str!("../examples/music/song.asm"), "DemoSong"),
            MusicClock::VBlank,
        );
        game.init(
            [
                game.reserve_music_channel(effect.channel()),
                game.play_sfx(&effect),
            ]
            .concat(),
        );
        let mut gb = engine_rom(&game).boot();
        gb.run_frames(5);
        assert_eq!(music_byte(&gb, MUSIC_MASK), mask);
        for (register, value) in registers {
            assert_eq!(
                gb.read(register.value),
                value,
                "{effect:?}: {}",
                register.name
            );
        }
    }
}

#[test]
fn background_tile_animation_sequences_pause_and_restart() {
    if !rgbds_enabled() {
        return;
    }
    for area in [TilemapArea::Map9800, TilemapArea::Map9C00] {
        for (kind, expected) in [
            (AnimationType::Loop, vec![0, 0, 1, 1, 2, 2, 0, 0, 1]),
            (AnimationType::PingPong, vec![0, 0, 1, 1, 2, 2, 1, 1, 0]),
            (AnimationType::Once, vec![0, 0, 1, 1, 2, 2, 2, 2, 2]),
        ] {
            let mut game = RustBoy::new();
            game.tiles
                .add_background("Patterns", TileSource::from_raw(&[["$00"; 8]; 3]));
            let animator = game.create_background_animation(
                "Water",
                BackgroundAnimationConfig::new(
                    area,
                    &[(0, 0), (31, 31)],
                    &[vec![0, 2], vec![1, 1], vec![2, 0]],
                )
                .delay(2)
                .playback(kind),
            );
            let request = game.vars.create_u8("wRequest", 0);
            let finished = game.vars.create_u8("wFinished", 0);
            let mut drive = Block::new();
            drive
                .label("DriveAnimation")
                .ld_a_addr_def(request.name())
                .and(R8::A)
                .ret_cond(Condition::Z)
                .ld(R8::B, R8::A)
                .xor(R8::A)
                .ld_addr_def_a(request.name())
                .ld(R8::A, R8::B)
                .cp(1)
                .jr_cond(Condition::NZ, ".pause")
                .emit_all(animator.step())
                .jr(".end")
                .label(".pause")
                .cp(2)
                .jr_cond(Condition::NZ, ".resume")
                .emit_all(animator.pause())
                .jr(".end")
                .label(".resume")
                .cp(3)
                .jr_cond(Condition::NZ, ".restart")
                .emit_all(animator.resume())
                .jr(".end")
                .label(".restart")
                .emit_all(animator.restart())
                .label(".end")
                .emit_all(animator.finished())
                .ld_addr_def_a(finished.name())
                .ret();
            game.define_function("DriveAnimation", drive.into_instrs());
            let call = game.call("DriveAnimation");
            game.add_to_main_loop(call);
            let rom = Rom::build(&game.build().unwrap(), &[]);
            let mut gb = rom.boot();
            gb.run_frames(3);
            let address = gb.symbol(request.name());
            for (i, &frame) in expected.iter().enumerate() {
                gb.write_ram(address, 1);
                gb.run_frames(2);
                assert_eq!(gb.read(area.address()), frame, "step {i}");
                assert_eq!(gb.read(area.address() + 1023), 2 - frame);
            }
            let last = *expected.last().unwrap();
            gb.write_ram(address, 2);
            gb.run_frames(2);
            for _ in 0..5 {
                gb.write_ram(address, 1);
                gb.run_frames(2);
                assert_eq!(gb.read(area.address()), last);
            }
            gb.write_ram(address, 3);
            gb.run_frames(2);
            gb.write_ram(address, 4);
            gb.run_frames(2);
            assert_eq!(gb.read_symbol("wFinished"), 0);
            gb.write_ram(address, 1);
            gb.run_frames(2);
            assert_eq!(gb.read(area.address()), 0);
            assert_eq!(gb.blocked_accesses().0, 0);
        }
    }
}

#[test]
fn background_animation_resumes_a_large_frame_before_advancing() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    let cells: Vec<_> = (0..300u16)
        .map(|i| ((i % 32) as u8, (i / 32) as u8))
        .collect();
    let animator = game.create_background_animation(
        "Large",
        BackgroundAnimationConfig::new(TilemapArea::Map9C00, &cells, &[vec![7; 300], vec![9; 300]])
            .playback(AnimationType::Once),
    );
    game.add_to_main_loop(animator.step());
    let rom = Rom::build(&game.build().unwrap(), &[]);
    let mut gb = rom.boot();
    gb.run_frames(15);
    assert_eq!(gb.read_symbol("wBgAnimLargeFrame"), 0);
    let cursor = u16::from(gb.read_symbol("wBgAnimLargeCursor"))
        | u16::from(gb.read(gb.symbol("wBgAnimLargeCursor") + 1)) << 8;
    assert!(cursor > 12 && cursor < 300);
    gb.run_frames(50);
    assert_eq!(gb.read_symbol("wBgAnimLargeFinished"), 1);
    for offset in 0..300 {
        assert_eq!(gb.read(hw::SCRN1.value + offset), 9);
    }
    gb.run_frames(5);
    assert_eq!(gb.read_symbol("wBgAnimLargeFrame"), 1);
    assert_eq!(gb.blocked_accesses().0, 0);
}

#[test]
fn background_animation_handles_single_and_256_frame_ranges() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    let once = game.create_background_animation(
        "All",
        BackgroundAnimationConfig::new(
            TilemapArea::Map9800,
            &[(0, 0)],
            &(0..=255).map(|i| vec![i]).collect::<Vec<_>>(),
        )
        .playback(AnimationType::Once),
    );
    let single = game.create_background_animation(
        "Single",
        BackgroundAnimationConfig::new(TilemapArea::Map9800, &[(1, 0)], &[vec![42]])
            .delay(255)
            .playback(AnimationType::PingPong),
    );
    game.add_to_main_loop(once.step())
        .add_to_main_loop(single.step());
    let rom = Rom::build(&game.build().unwrap(), &[]);
    let mut gb = rom.boot();
    gb.run_frames(270);
    assert_eq!(gb.read(hw::SCRN0.value), 255);
    assert_eq!(gb.read_symbol("wBgAnimAllFinished"), 1);
    assert_eq!(gb.read_symbol("wBgAnimAllFrame"), 255);
    assert_eq!(gb.read(hw::SCRN0.value + 1), 42);
    assert_eq!(gb.read_symbol("wBgAnimSingleFrame"), 0);
    assert_eq!(gb.read_symbol("wBgAnimSingleFinished"), 0);
    assert_eq!(gb.blocked_accesses().0, 0);
}

#[test]
fn wram_arrays_initialize_and_check_runtime_indices_across_pages() {
    if !rgbds_enabled() {
        return;
    }
    for element in [
        ArrayElement::U8,
        ArrayElement::I8,
        ArrayElement::U16,
        ArrayElement::I16,
    ] {
        let word = matches!(element, ArrayElement::U16 | ArrayElement::I16);
        let mut game = RustBoy::new();
        let initial = match element {
            ArrayElement::U8 => 126,
            ArrayElement::I8 => -5,
            ArrayElement::U16 => 0x1234,
            ArrayElement::I16 => -300,
        };
        let before = game.vars.create_u8("wArrayBefore", 0x5a);
        let array = game.vars.create_array("wArray", element, 300, initial);
        let after = game.vars.create_u8("wArrayAfter", 0xa5);
        let mut code = Block::new();
        // 512: high byte above 300's, low byte below it (only the high-byte compare rejects it)
        for (case, index) in [0u16, 1, 255, 256, 299, 300, 512, 65535]
            .into_iter()
            .enumerate()
        {
            let result = game.vars.create_u16(&format!("wArrayResult{case}"), 0);
            let status = game.vars.create_u8(&format!("wArrayStatus{case}"), 0);
            let valid = index < 300;
            let end = game.labels().local("array_test_end");
            code.ld(R16::BC, index)
                .ld(R16::DE, 0xbeefu16)
                .emit_all(array.set_indexed(game.labels()))
                .ld_a(0)
                .adc(0)
                .ld_addr_def_a(status.name())
                .emit_all(array.get_indexed(game.labels()))
                .jr_cond(Condition::C, &end);
            if word {
                code.ld(R8::A, R8::L)
                    .ld_addr_def_a(result.name())
                    .ld(R8::A, R8::H)
                    .ld(Mem::addr(Expr::sym(result.name()) + 1), R8::A);
            } else {
                code.ld_addr_def_a(result.name());
            }
            code.label(&end);
            assert_eq!(valid, case < 5);
        }
        // Also exercise statically addressed signed/unsigned values.
        code.emit_all(array.set(42, initial))
            .emit_all(array.get(42));
        game.init(code);
        let mut gb = engine_rom(&game).boot();
        assert!(gb.run_until_pc(gb.symbol("Main"), 20));
        let base = gb.symbol("wArray");
        assert_eq!(gb.read_symbol(before.name()), 0x5a);
        assert_eq!(gb.read_symbol(after.name()), 0xa5);
        let stride = if word { 2 } else { 1 };
        assert_eq!(base, game.vars.get_address(array.id()).unwrap());
        for index in 0..300u16 {
            let touched = [0, 1, 255, 256, 299].contains(&index);
            let value = if touched { 0xbeef } else { initial as u16 };
            assert_eq!(gb.read(base + index * stride), value as u8);
            if word {
                assert_eq!(gb.read(base + index * stride + 1), (value >> 8) as u8);
            }
        }
        for case in 0..7 {
            assert_eq!(
                gb.read_symbol(&format!("wArrayStatus{case}")),
                u8::from(case >= 5)
            );
            let result = gb.symbol(&format!("wArrayResult{case}"));
            let expected = if case >= 5 {
                0
            } else if word {
                0xbeef
            } else {
                0xef
            };
            assert_eq!(
                u16::from_le_bytes([gb.read(result), gb.read(result + 1)]),
                expected
            );
        }
    }
}

#[test]
fn a_nearly_full_wram_array_uses_compact_startup_code() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    game.vars
        .create_array("wWholeWram", ArrayElement::U8, 4095, 0x5a);
    let output = game.build().unwrap();
    let init = output
        .split("EntryPoint:")
        .nth(1)
        .unwrap()
        .split("Main:")
        .next()
        .unwrap();
    assert!(
        init.lines().count() < 100,
        "array initialization should be a loop"
    );
    let mut gb = engine_rom(&game).boot();
    assert!(gb.run_until_pc(gb.symbol("Main"), 20));
    for offset in 0..4095 {
        assert_eq!(gb.read(hw::RAM.value + offset), 0x5a);
    }
}

#[test]
fn fonts_after_other_tiles_print_their_own_glyph_tiles() {
    // Three other background tiles first, then a letter font ('A'..='C', tiles 3..=5) and a
    // digit font (tiles 6..=15): the printed map bytes are the fonts' tiles, not 0-based
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    game.tiles
        .add_background("Other", TileSource::from_raw(&[["$00"; 8]; 3]));
    let letters = game.load_font("Letters", TileSource::from_raw(&[["$00"; 8]; 3]), b'A');
    let digits = game.load_font("Digits", TileSource::from_raw(&[["$00"; 8]; 10]), b'0');
    let area = TilemapArea::Map9800;
    let text = game.create_text_printer("Word", letters, area, 2, 0, "CAB");
    let score = game.create_number_printer("Score", digits, area, 0, 1, NumberFormat::DecimalU16);
    let mut init = Block::new();
    init.ld(R16::DE, 12034)
        .emit_all(score.set_value(game.labels()));
    game.init(init);
    game.add_to_main_loop(text.step())
        .add_to_main_loop(score.step());
    let mut gb = Rom::build(&game.build().unwrap(), &[]).boot();
    gb.run_frames(10);
    for (i, tile) in [5u8, 3, 4].into_iter().enumerate() {
        assert_eq!(gb.read(area.address() + 2 + i as u16), tile, "letter {i}");
    }
    for (i, digit) in [1u8, 2, 0, 3, 4].into_iter().enumerate() {
        assert_eq!(
            gb.read(area.address() + 32 + i as u16),
            6 + digit,
            "digit {i}"
        );
    }
}

#[test]
fn text_printer_resumes_long_strings_and_formats_scores() {
    if !rgbds_enabled() {
        return;
    }
    for area in [TilemapArea::Map9800, TilemapArea::Map9C00] {
        let mut game = RustBoy::new();
        let font = game.load_font("Digits", TileSource::from_raw(&[["$00"; 8]; 10]), b'0');
        let text = game.create_text_printer(
            "Long",
            font,
            area,
            0,
            1,
            "0123456789012345678901234567890123456789\n9",
        );
        let decimal =
            game.create_number_printer("Decimal", font, area, 0, 4, NumberFormat::DecimalU16);
        let bcd = game.create_number_printer("Bcd", font, area, 0, 5, NumberFormat::PackedBcd);
        let mut init = Block::new();
        init.ld(R16::DE, 65535)
            .emit_all(decimal.set_value(game.labels()))
            .ld(R16::DE, 0x1234)
            .emit_all(bcd.set_value(game.labels()));
        game.init(init);
        game.add_to_main_loop(text.step())
            .add_to_main_loop(decimal.step())
            .add_to_main_loop(bcd.step());
        let rom = Rom::build(&game.build().unwrap(), &[]);
        let mut gb = rom.boot();
        gb.run_frames(30);
        for i in 0..40u16 {
            assert_eq!(gb.read(area.address() + 32 + i), (i % 10) as u8);
        }
        assert_eq!(gb.read(area.address() + 96), 9);
        for (i, digit) in [6, 5, 5, 3, 5].into_iter().enumerate() {
            assert_eq!(gb.read(area.address() + 128 + i as u16), digit);
        }
        for (i, digit) in [1, 2, 3, 4].into_iter().enumerate() {
            assert_eq!(gb.read(area.address() + 160 + i as u16), digit);
        }
        assert_eq!(gb.read_symbol("wTextLongCursor"), 41);
        assert_eq!(gb.blocked_accesses().0, 0);
    }
}

#[test]
fn a_text_job_of_more_than_256_glyphs_prints_all_and_stops() {
    // The cursor is a 16-bit word: past 255 glyphs its high byte counts too
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    let font = game.load_font("Digits", TileSource::from_raw(&[["$00"; 8]; 10]), b'0');
    let text: String = (0..300)
        .map(|i| char::from(b'0' + (i % 10) as u8))
        .collect();
    let printer = game.create_text_printer("Long", font, TilemapArea::Map9800, 0, 0, &text);
    game.add_to_main_loop(printer.step());
    let mut gb = Rom::build(&game.build().unwrap(), &[]).boot();
    gb.run_frames(60);
    for i in 0..300u16 {
        assert_eq!(gb.read(hw::SCRN0.value + i), (i % 10) as u8, "glyph {i}");
    }
    let cursor = gb.symbol("wTextLongCursor");
    assert_eq!(
        u16::from_le_bytes([gb.read(cursor), gb.read(cursor + 1)]),
        300
    );
    assert_eq!(gb.blocked_accesses().0, 0);
}

#[test]
fn score_encoders_cover_decimal_boundaries_and_reject_invalid_bcd() {
    if !rgbds_enabled() {
        return;
    }
    for value in [0, 1, 9, 10, 99, 100, 999, 1000, 9999, 10000, 32768, 65535] {
        let mut game = RustBoy::new();
        let font = game.load_font("Digits", TileSource::from_raw(&[["$00"; 8]; 10]), b'0');
        let score = game.create_number_printer(
            "Score",
            font,
            TilemapArea::Map9800,
            0,
            0,
            NumberFormat::DecimalU16,
        );
        let mut init = Block::new();
        init.ld(R16::DE, value)
            .emit_all(score.set_value(game.labels()));
        game.init(init);
        game.add_to_main_loop(score.step());
        let rom = Rom::build(&game.build().unwrap(), &[]);
        let mut gb = rom.boot();
        gb.run_frames(10);
        for (i, ch) in format!("{value:05}").bytes().enumerate() {
            assert_eq!(
                gb.read(hw::SCRN0.value + i as u16),
                ch - b'0',
                "value {value}"
            );
        }
    }
    for value in [0, 0x9999, 0xa123, 0x1b23, 0x12c3, 0x123f] {
        let mut game = RustBoy::new();
        let font = game.load_font("Digits", TileSource::from_raw(&[["$00"; 8]; 10]), b'0');
        let score = game.create_number_printer(
            "Score",
            font,
            TilemapArea::Map9800,
            0,
            0,
            NumberFormat::PackedBcd,
        );
        let invalid = game.vars.create_u8("wCarry", 0);
        let mut init = Block::new();
        // The old value shares no digit with the invalid ones: a partial write would show
        init.ld(R16::DE, 0x5678)
            .emit_all(score.set_value(game.labels()))
            .ld(R16::DE, value)
            .emit_all(score.set_value(game.labels()))
            .ld_a(0)
            .adc(0)
            .ld_addr_def_a(invalid.name());
        game.init(init);
        game.add_to_main_loop(score.step());
        let rom = Rom::build(&game.build().unwrap(), &[]);
        let mut gb = rom.boot();
        gb.run_frames(10);
        let bad = value != 0 && value != 0x9999;
        assert_eq!(gb.read_symbol(invalid.name()), u8::from(bad));
        let expected = if bad {
            [5, 6, 7, 8]
        } else if value == 0 {
            [0; 4]
        } else {
            [9; 4]
        };
        for (i, digit) in expected.into_iter().enumerate() {
            assert_eq!(gb.read(hw::SCRN0.value + i as u16), digit);
        }
    }
}

#[test]
fn runtime_oam_pools_allocate_hide_release_and_reuse_disjoint_slots() {
    if !rgbds_enabled() {
        return;
    }
    for size in [SpriteSize::Size8x8, SpriteSize::Size8x16] {
        let mut game = RustBoy::with_config(RustBoyConfig::default().sprite_size(size));
        let tiles = TileSource::from_raw(&vec![["$ff"; 8]; usize::from(size.tiles_per_sprite())]);
        let before = game.add_sprite("StaticBefore", tiles.clone(), 8, 8, 0);
        let pool = game.create_sprite_pool("Actors", 3);
        game.add_sprite("StaticAfter", tiles, 120, 120, 0);
        let other = game.create_sprite_pool("Other", 2);
        assert_eq!((pool.first_slot(), other.first_slot()), (1, 5));
        let action = game.vars.create_u8("wPoolAction", 0);
        let result = game.vars.create_u8("wPoolResult", 0);
        let full = game.vars.create_u8("wPoolFull", 0);
        let mut init = Block::new();
        for i in 0..3u8 {
            init.emit_all(pool.allocate())
                .ld_b(20 + i)
                .ld_c(30 + i)
                .ld_d(0)
                .ld_e(0x10)
                .emit_all(pool.write());
        }
        init.emit_all(pool.allocate()).ld_addr_def_a(full.name());
        game.init(init);
        let mut hide = Block::new();
        hide.ld_a(0).emit_all(pool.hide()).emit_all(action.set(0));
        let mut release = Block::new();
        release
            .ld_a(0)
            .emit_all(pool.release())
            .emit_all(action.set(0));
        let mut reuse = Block::new();
        reuse
            .emit_all(pool.allocate())
            .ld_addr_def_a(result.name())
            .ld_b(60)
            .ld_c(70)
            .ld_d(0)
            .ld_e(0x20)
            .emit_all(pool.write())
            .emit_all(action.set(0));
        for (event, code) in [(1, hide), (2, release), (3, reuse)] {
            game.add_to_main_loop(action.get());
            game.add_to_main_loop(IfA::eq(event, code.into_instrs()));
        }
        let mut gb = engine_rom(&game).boot();
        assert!(gb.run_until_pc(gb.symbol("Main"), 10));
        gb.run_frames(2);
        assert_eq!(gb.read_symbol(full.name()), 255);
        assert_eq!(gb.oam_entry(0), [24, 16, 0, 0]);
        let static_after = gb.oam_entry(4);
        let active = gb.oam_entry(1);
        assert_eq!(active, [46, 28, 0, 0x10]);
        for i in 0..3 {
            assert_eq!(gb.read(gb.symbol("wPoolActorsUsed") + i), 1);
        }
        gb.write_ram(gb.symbol(action.name()), 1);
        gb.run_frames(3);
        assert_eq!(gb.oam_entry(1), [0, active[1], active[2], active[3]]);
        assert_eq!(gb.read_symbol("wPoolActorsUsed"), 1);
        gb.write_ram(gb.symbol(action.name()), 2);
        gb.run_frames(3);
        assert_eq!(gb.oam_entry(1), [0; 4]);
        assert_eq!(gb.read_symbol("wPoolActorsUsed"), 0);
        gb.write_ram(gb.symbol(action.name()), 3);
        gb.run_frames(3);
        assert_eq!(gb.read_symbol(result.name()), 0);
        assert_eq!(gb.oam_entry(1), [86, 68, 0, 0x20]);
        assert_eq!(gb.oam_entry(4), static_after);
        assert_eq!(gb.oam_entry(5), [0; 4]);
        // Static sprite visibility leaves tile/flags alone and accepts a new screen position.
        let mut game2 = RustBoy::new();
        let sprite = game2.add_sprite("Visible", TileSource::from_raw(&[["$ff"; 8]]), 8, 8, 0x10);
        game2.init(game2.sprites.hide(sprite));
        let mut hidden = engine_rom(&game2).boot();
        hidden.run_frames(2);
        assert_eq!(hidden.oam_entry(0), [0, 16, 0, 0x10]);
        game2.add_to_main_loop(game2.sprites.show_at(sprite, 24, 32));
        let mut shown = engine_rom(&game2).boot();
        shown.run_frames(3);
        assert_eq!(shown.oam_entry(0), [48, 32, 0, 0x10]);
        assert!(!game.sprites.hide(before).is_empty());
    }
}

#[test]
fn composites_hide_and_show_at_keep_every_part_and_offset() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    let tile = || TileSource::from_raw(&[["$ff"; 8]]);
    let parts = [
        MetaspritePart::new(tile(), 0, 0, 0x10u8),
        MetaspritePart::new(tile(), 8, 0, 0x20u8),
        MetaspritePart::new(tile(), -8, 8, 0x40u8),
    ];
    let actor = game.add_metasprite("Actor", 40, 40, &parts);
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(3);
    let initial: Vec<_> = (0..3).map(|entry| gb.oam_entry(entry)).collect();
    game.init(game.sprites.hide_composite(actor));
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(3);
    for (entry, before) in initial.iter().enumerate() {
        let hidden = gb.oam_entry(entry);
        assert_eq!(hidden[0], 0, "part {entry} hidden by Y=0");
        assert_eq!(
            &hidden[1..],
            &before[1..],
            "part {entry} keeps X, tile and flags"
        );
    }
    // Shown again elsewhere: the first part at (60, 50), the others at their offsets
    game.init(game.sprites.show_composite_at(actor, 60, 50));
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(3);
    for (entry, (before, part)) in initial.iter().zip(&parts).enumerate() {
        let shown = gb.oam_entry(entry);
        let x = (60 + part.x + i16::from(hw::OAM_X_OFFSET)) as u8;
        let y = (50 + part.y + i16::from(hw::OAM_Y_OFFSET)) as u8;
        assert_eq!((shown[0], shown[1]), (y, x), "part {entry}");
        assert_eq!(
            &shown[2..],
            &before[2..],
            "part {entry} keeps tile and flags"
        );
    }
}

#[test]
fn pool_allocation_returns_its_cleared_hidden_entry_in_hl() {
    if !rgbds_enabled() {
        return;
    }
    let mut game = RustBoy::new();
    game.add_sprite("Static", TileSource::from_raw(&[["$ff"; 8]]), 8, 8, 0);
    let pool = game.create_sprite_pool("Actors", 3);
    let entries = game.vars.create_u16("wPoolEntries", 0);
    let second = game.vars.create_u16("wPoolSecondEntry", 0);
    let mut init = Block::new();
    // Fill slot 0 with nonzero bytes, release it, then allocate twice: slot 0 comes back
    // cleared, slot 1 is fresh
    init.emit_all(pool.allocate())
        .ld_b(20)
        .ld_c(30)
        .ld_d(4)
        .ld_e(0x30)
        .emit_all(pool.write())
        .ld_a(0)
        .emit_all(pool.release())
        .emit_all(pool.allocate())
        .ld(R8::A, R8::L)
        .ld_addr_def_a(entries.name())
        .ld(R8::A, R8::H)
        .ld_addr_def_a(Expr::sym(entries.name()) + 1)
        .emit_all(pool.allocate())
        .ld(R8::A, R8::L)
        .ld_addr_def_a(second.name())
        .ld(R8::A, R8::H)
        .ld_addr_def_a(Expr::sym(second.name()) + 1);
    game.init(init);
    let mut gb = engine_rom(&game).boot();
    gb.run_frames(2);
    let shadow = gb.symbol("wShadowOAM");
    let word = |name: &str| {
        let address = gb.symbol(name);
        u16::from_le_bytes([gb.read(address), gb.read(address + 1)])
    };
    let first_slot = u16::from(pool.first_slot());
    for (name, slot) in [("wPoolEntries", 0), ("wPoolSecondEntry", 1)] {
        let entry = shadow + 4 * (first_slot + slot);
        assert_eq!(
            word(name),
            entry,
            "{name}: HL is the slot's shadow OAM entry"
        );
        for byte in 0..4 {
            assert_eq!(gb.read(entry + byte), 0, "{name}: byte {byte} cleared");
        }
    }
}

#[test]
fn sprite_pool_rejects_free_invalid_and_unrepresentable_writes_without_mutation() {
    if !rgbds_enabled() {
        return;
    }
    for size in [SpriteSize::Size8x8, SpriteSize::Size8x16] {
        let mut game = RustBoy::with_config(RustBoyConfig::default().sprite_size(size));
        let pool = game.create_sprite_pool("Checked", 2);
        let cases = game.vars.create_array("wPoolCarry", ArrayElement::U8, 9, 0);
        let mut code = Block::new();
        code.emit_all(pool.allocate())
            .ld_b(24)
            .ld_c(32)
            .ld_d(0)
            .ld_e(0x10)
            .emit_all(pool.write());
        let checks = [
            (pool.release(), 255u8, 24u8, 32u8, 0u8, true),
            (pool.hide(), 2, 24, 32, 0, true),
            (pool.write(), 1, 24, 32, 0, true), // free slot
            (pool.release(), 1, 24, 32, 0, true),
            (pool.write(), 0, 248, 32, 0, true),
            (pool.write(), 0, 24, 240, 0, true),
            (pool.write(), 0, 24, 32, 1, size == SpriteSize::Size8x16),
        ];
        for (index, (operation, slot, x, y, tile, _)) in checks.iter().enumerate() {
            code.ld_a(*slot)
                .ld_b(*x)
                .ld_c(*y)
                .ld_d(*tile)
                .ld_e(0x10)
                .emit_all(operation.clone())
                .ld_a(0)
                .adc(0)
                .ld_addr_def_a(cases.address(index as u16));
        }
        code.emit_all(pool.allocate())
            .emit_all(pool.allocate()) // third claim: full
            .ld_a(0)
            .adc(0)
            .ld_addr_def_a(cases.address(7))
            .ld_a(1)
            .emit_all(pool.release())
            .ld_a(1)
            .emit_all(pool.release())
            .ld_a(0)
            .adc(0)
            .ld_addr_def_a(cases.address(8));
        game.init(code);
        let mut gb = engine_rom(&game).boot();
        assert!(gb.run_until_pc(gb.symbol("Main"), 10));
        gb.run_frames(2);
        let base = gb.symbol(cases.name());
        for (index, (_, _, _, _, _, rejected)) in checks.iter().enumerate() {
            assert_eq!(gb.read(base + index as u16), u8::from(*rejected));
        }
        assert_eq!(gb.read(base + 7), 1);
        assert_eq!(gb.read(base + 8), 1);
        assert_eq!(
            gb.oam_entry(0),
            [
                48,
                32,
                if size == SpriteSize::Size8x16 { 0 } else { 1 },
                0x10
            ]
        );
        assert_eq!(gb.oam_entry(1), [0; 4]);
        assert_eq!(gb.read_symbol("wPoolCheckedUsed"), 1);
        assert_eq!(gb.read(gb.symbol("wPoolCheckedUsed") + 1), 0);
    }
}
