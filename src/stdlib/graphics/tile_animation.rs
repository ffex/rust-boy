//! Background/window tile-ID animation through the VBlank write queue.
//!
//! All frame graphics must already be loaded. This animates map references, not tile
//! pattern bytes. There is one init/main producer; do not call from interrupt handlers.
use super::vram_queue;
use crate::asm::{Block, Condition, Expr, Mem, R8, R16, R16Stack};
use crate::stdlib::routine::{Regs, Routine};

/// Sequence behavior after the final frame.
#[derive(Debug, Clone, Copy)]
pub enum TilePlayback {
    Loop,
    PingPong,
    Once,
}

/// WRAM labels required by a tile animator. Cursor is a word; all others are bytes.
#[derive(Debug, Clone)]
pub struct TileAnimationState {
    pub cursor: String,
    pub frame: String,
    pub timer: String,
    pub direction: String,
    pub enabled: String,
    pub finished: String,
}
impl TileAnimationState {
    pub fn new(name: &str) -> Self {
        Self {
            cursor: format!("wBgAnim{name}Cursor"),
            frame: format!("wBgAnim{name}Frame"),
            timer: format!("wBgAnim{name}Timer"),
            direction: format!("wBgAnim{name}Direction"),
            enabled: format!("wBgAnim{name}Enabled"),
            finished: format!("wBgAnim{name}Finished"),
        }
    }
}

/// `targets` is a ROM word array of map addresses; `frames` is a ROM word array of
/// pointers to frame byte arrays (one tile ID per target). Initialize cursor/frame/
/// timer/direction/finished to zero and enabled to one. Call once per game frame.
/// The first call queues frame zero. Hold each fully queued frame for `delay` calls;
/// queue backpressure pauses playback and resumes the current frame. Carry set means
/// queue full; clear means idle or complete. Changes all registers, preserves IME.
/// Pausing preserves progress; `finished` becomes one after Once's final frame queues.
/// The queue flush is asynchronous: multi-cell frames may become visible in pieces.
///
/// # Panics
/// If `cells` is not 1..=1024, `count` not 1..=256, or `delay` is 0.
#[track_caller]
pub fn animate(
    name: &str,
    targets: &str,
    frames: &str,
    cells: u16,
    count: u16,
    delay: u8,
    playback: TilePlayback,
) -> Routine {
    assert!(cells > 0 && cells <= 1024, "invalid cell count");
    assert!(
        (1..=256).contains(&count) && delay > 0,
        "invalid animation timing"
    );
    let state = TileAnimationState::new(name);
    let routine = format!("UpdateBgAnim{name}");
    let mut b = Block::new();
    b.label(&routine)
        .ld_a_addr_def(&state.enabled)
        .and(R8::A)
        .ret_cond(Condition::Z)
        .ld_a_addr_def(&state.finished)
        .and(R8::A)
        .jr_cond(Condition::NZ, ".idle")
        .ld_a_addr_def(&state.timer)
        .and(R8::A)
        .jr_cond(Condition::Z, ".queue")
        .dec(R8::A)
        .ld_addr_def_a(&state.timer)
        .jr_cond(Condition::NZ, ".idle");
    // Expired hold: choose the next frame. The current frame is fully queued.
    match playback {
        TilePlayback::Loop => {
            b.ld_a_addr_def(&state.frame)
                .cp((count - 1) as u8)
                .jr_cond(Condition::C, ".forward")
                .xor(R8::A)
                .jr(".set_frame");
        }
        TilePlayback::Once => {
            b.ld_a_addr_def(&state.frame).jr(".forward");
        }
        TilePlayback::PingPong if count == 1 => {
            b.xor(R8::A).jr(".set_frame");
        }
        TilePlayback::PingPong => {
            b.ld_a_addr_def(&state.frame)
                .and(R8::A)
                .jr_cond(Condition::Z, ".turn_forward")
                .cp((count - 1) as u8)
                .jr_cond(Condition::Z, ".turn_back")
                .ld_a_addr_def(&state.direction)
                .and(R8::A)
                .ld_a_addr_def(&state.frame)
                .jr_cond(Condition::Z, ".forward")
                .dec(R8::A)
                .jr(".set_frame")
                .label(".turn_forward")
                .xor(R8::A)
                .ld_addr_def_a(&state.direction)
                .ld_a_addr_def(&state.frame)
                .jr(".forward")
                .label(".turn_back")
                .ld_a(1)
                .ld_addr_def_a(&state.direction)
                .ld_a_addr_def(&state.frame)
                .dec(R8::A)
                .jr(".set_frame");
        }
    }
    b.label(".forward")
        .inc(R8::A)
        .label(".set_frame")
        .ld_addr_def_a(&state.frame)
        .label(".queue")
        .ld_a_addr_def(&state.cursor)
        .ld(R8::E, R8::A)
        .ld(R8::A, Mem::addr(Expr::sym(&state.cursor) + 1))
        .ld(R8::D, R8::A)
        .cp((cells >> 8) as u8)
        .jr_cond(Condition::C, ".write")
        .jr_cond(Condition::NZ, ".complete")
        .ld(R8::A, R8::E)
        .cp(cells as u8)
        .jr_cond(Condition::NC, ".complete")
        .label(".write")
        .push(R16Stack::DE)
        .ld(R16::HL, targets)
        .add_hl(R16::DE)
        .add_hl(R16::DE)
        .ld(R8::A, Mem::Hli)
        .ld(R8::C, R8::A)
        .ld(R8::B, R8::AtHl)
        .push(R16Stack::BC)
        .ld_a_addr_def(&state.frame)
        .ld(R8::L, R8::A)
        .ld(R8::H, 0)
        .add_hl(R16::HL)
        .ld(R16::BC, frames)
        .add_hl(R16::BC)
        .ld(R8::A, Mem::Hli)
        .ld(R8::H, R8::AtHl)
        .ld(R8::L, R8::A)
        .add_hl(R16::DE)
        .ld(R8::A, R8::AtHl)
        .pop(R16Stack::HL)
        .call("QueueVramWrite")
        .pop(R16Stack::DE)
        .ret_cond(Condition::C)
        .inc(R16::DE)
        .ld(R8::A, R8::E)
        .ld_addr_def_a(&state.cursor)
        .ld(R8::A, R8::D)
        .ld(Mem::addr(Expr::sym(&state.cursor) + 1), R8::A)
        .jr(".queue")
        .label(".complete");
    if matches!(playback, TilePlayback::Once) {
        b.ld_a_addr_def(&state.frame)
            .cp((count - 1) as u8)
            .jr_cond(Condition::NZ, ".hold")
            .ld_a(1)
            .ld_addr_def_a(&state.finished)
            .jr(".idle")
            .label(".hold");
    }
    b.xor(R8::A)
        .ld_addr_def_a(&state.cursor)
        .ld(Mem::addr(Expr::sym(&state.cursor) + 1), R8::A)
        .ld_a(delay)
        .ld_addr_def_a(&state.timer)
        .label(".idle")
        .xor(R8::A)
        .ret();
    Routine::new(&routine, b)
        .with_dep(vram_queue::enqueue())
        .with_clobbers(Regs::ALL)
}
