//! Headless-emulator tests: build a ROM with RGBDS, run it on `support::gameboy` with
//! scripted input, and check its memory and registers.
//!
//! They need RGBDS: they run when `RGBDS_LINK_CHECK` is set (CI sets it) and return early
//! otherwise. The emulator's own checks against Blargg's test ROMs run when
//! `GB_TEST_ROMS` names a directory that holds them (`scripts/fetch-test-roms.sh`; CI
//! sets it too).

mod support;

use std::collections::BTreeSet;

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

// ----- The examples -----

/// Every CPU write to OAM with the LCD on happened in VBlank and none was dropped: the
/// main loop of the example fits in VBlank, so it does not run into [B12] today
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
    assert_oam_written_in_vblank(&gb);
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
}

#[test]
fn unbricked_ball_bounces_paddle_moves_bricks_break() {
    if !rgbds_enabled() {
        return;
    }
    let gb = check_unbricked("unbricked");
    assert!(bricks(&gb) < 32, "bricks broke: {} left", bricks(&gb));
    assert!(gb.read_symbol("wScore") > 0, "the score went up");
}

#[test]
fn unbricked_std_ball_bounces_paddle_moves() {
    if !rgbds_enabled() {
        return;
    }
    check_unbricked("unbricked_std");
}

#[test]
fn unbricked_rustboy_ball_bounces_paddle_moves_bricks_break() {
    if !rgbds_enabled() {
        return;
    }
    let gb = check_unbricked("unbricked_rustboy");
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
    assert_oam_written_in_vblank(&gb);
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
    assert_oam_written_in_vblank(&gb);
}

// ----- B12: OAM written outside VBlank -----

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

/// [B12], today's behaviour: the engine moves a sprite by writing OAM from the main loop.
/// While the main loop fits in VBlank the sprite moves one pixel a frame; once the code
/// before the move outlasts VBlank (~1140 M-cycles), the move runs in the visible part of
/// the frame, and what happens depends on where in the line it lands: in HBlank it works,
/// in mode 2 or 3, while the PPU owns OAM, the read gives `$FF` and the write is dropped.
/// With 1008 iterations of `Delay` (1006 to 1011 give the same) it lands in mode 3 of line
/// 52 on every frame, so the sprite never moves.
///
/// When shadow OAM + DMA lands (Phase 3, graphics), the delayed program must move its
/// sprite like the other one: turn the second half of this test into that check.
///
/// [B12]: ../CONTEXT.md#b12
#[test]
fn b12_oam_written_outside_vblank_is_lost() {
    if !rgbds_enabled() {
        return;
    }
    // Within VBlank: the sprite (OAM X = 8 + 8) moves 1 pixel a frame
    let mut gb = moving_sprite_rom(0).boot();
    gb.run_frames(10);
    let x = gb.oam_entry(0)[1];
    gb.run_frames(60);
    assert_eq!(gb.oam_entry(0)[1], x.wrapping_add(60));
    assert_oam_written_in_vblank(&gb);

    // After the delay the move runs mid-frame, in mode 3
    let mut gb = moving_sprite_rom(1008).boot();
    gb.run_frames(10);
    let x = gb.oam_entry(0)[1];
    let before = gb.oam_writes().len();
    let blocked_reads = gb.blocked_accesses().2;
    gb.run_frames(60);
    let moves: Vec<_> = gb.oam_writes()[before..].to_vec();
    assert_eq!(moves.len(), 60, "one OAM write a frame");
    assert!(
        moves
            .iter()
            .all(|write| write.lcd_on && write.mode == Mode::Drawing && write.dropped),
        "every write lands in mode 3 and is dropped: {:?}",
        moves.first()
    );
    assert_eq!(
        gb.oam_entry(0)[1],
        x,
        "today (B12) the sprite does not move"
    );
    assert_eq!(
        gb.blocked_accesses().2 - blocked_reads,
        60,
        "and each read of the sprite's X gave $FF"
    );
}
