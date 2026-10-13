//! Interrupts: the five sources of the Game Boy, their vectors and handlers, the wait for
//! the next frame, and the code that configures, enables and disables them.
//!
//! An interrupt ([`Interrupt`]) is requested by the hardware (VBlank, the `rSTAT`
//! conditions, a timer overflow, the end of a serial transfer, a joypad line going low) by
//! setting its bit in `rIF`. When its bit is also set in `rIE` and interrupts are on
//! (`ei`), the CPU clears the `rIF` bit, turns interrupts off, pushes `pc` and jumps to the
//! interrupt's vector ([`Interrupt::vector`], `$40` for VBlank). The pieces here:
//! - [`vector`]: the 8-byte vector, a `SECTION` at its address with a `jp` to the handler;
//! - [`handler`]: a handler, the code of the interrupt between the `push`es and `pop`s of
//!   the registers it changes, then `reti` (which turns interrupts on again). The registers
//!   it saves follow the calling-convention model of [`Routine`]: what the code may write
//!   ([`Regs::written_by`]), or a routine's [`Routine::changes`]; so the code it interrupts
//!   finds every register as it left it;
//! - [`wait_frame`]: the `WaitFrame` routine, which sleeps (`halt`) until the VBlank
//!   handler has run ([`signal_frame`]), without missing a VBlank that comes while it
//!   checks;
//! - the code that enables or disables interrupts in `rIE` ([`enable`], [`disable`],
//!   [`set_enabled`]), drops the pending ones ([`clear_pending`]), configures the `rSTAT`
//!   sources and `rLYC` ([`Stat`], [`configure_stat`]) and the timer ([`Timer`],
//!   [`start_timer`], [`stop_timer`]).
//!
//! The order of a start-up that turns them on: `di` first, then the sources (`rSTAT`,
//! `rLYC`, the timer) and `rIE`, then [`clear_pending`] (what the hardware requested
//! before, such as the VBlank the start-up waited for, is dropped), then `ei`. `RustBoy`
//! does this in its start-up code and uses these pieces for its handlers (see
//! `RustBoy::on_interrupt`).
//!
//! # Example
//! ```
//! use rust_boy::asm::{Block, R8};
//! use rust_boy::stdlib::interrupts::{self, Interrupt, Timer, TimerClock};
//! use rust_boy::stdlib::routine::Regs;
//!
//! // A timer handler that counts in wTicks
//! let mut count = Block::new();
//! count.ld_a_addr_def("wTicks").inc(R8::A).ld_addr_def_a("wTicks");
//! let saves = Regs::written_by(&count).unwrap_or(Regs::ALL);
//! assert_eq!(saves, Regs::AF);
//! let handler = interrupts::handler(Interrupt::Timer, count, saves);
//! let text: Vec<String> = handler.body().iter().map(|i| i.to_string()).collect();
//! assert_eq!(
//!     text,
//!     ["TimerHandler:", "push af", "ld a, [wTicks]", "inc a", "ld [wTicks], a", "pop af", "reti"]
//! );
//!
//! // Its vector, and the start-up code that runs it 64 times a second
//! let mut startup = Block::new();
//! startup
//!     .emit_all(interrupts::start_timer(Timer::new(TimerClock::Hz4096, 0)))
//!     .emit_all(interrupts::set_enabled(&[Interrupt::VBlank, Interrupt::Timer]))
//!     .emit_all(interrupts::clear_pending())
//!     .ei();
//! let vector = interrupts::vector(Interrupt::Timer, Some("TimerHandler"));
//! assert_eq!(vector[0].to_string(), "SECTION \"Timer interrupt\", ROM0[$50]");
//! ```

use crate::asm::{Block, Condition, Expr, Instr, Mem, R8, R16Stack, Section};
use crate::hw;

use super::routine::{Regs, Routine};

/// An interrupt source of the Game Boy, in the order of their priority (VBlank first) and
/// of their bits in `rIE` / `rIF`
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Interrupt {
    /// The LCD entered VBlank (line 144): once a frame, while the LCD is on
    VBlank,
    /// One of the `rSTAT` conditions that [`Stat`] selects: `LY` = `LYC`, or a PPU mode
    Stat,
    /// The timer overflowed (`rTIMA` went past 255; see [`Timer`])
    Timer,
    /// A serial transfer ended
    Serial,
    /// A joypad line went from high to low: a button pressed in a group that `rP1`
    /// selects, or a group selected while one of its buttons is held
    Joypad,
}

impl Interrupt {
    /// Every interrupt, in priority order
    pub const ALL: [Interrupt; 5] = [
        Interrupt::VBlank,
        Interrupt::Stat,
        Interrupt::Timer,
        Interrupt::Serial,
        Interrupt::Joypad,
    ];

    /// Its bit in `rIE` and `rIF`: `IEF_VBLANK`, `IEF_STAT`, `IEF_TIMER`, `IEF_SERIAL`,
    /// `IEF_HILO`
    pub fn flag(self) -> hw::Symbol<u8> {
        match self {
            Interrupt::VBlank => hw::IEF_VBLANK,
            Interrupt::Stat => hw::IEF_STAT,
            Interrupt::Timer => hw::IEF_TIMER,
            Interrupt::Serial => hw::IEF_SERIAL,
            Interrupt::Joypad => hw::IEF_HILO,
        }
    }

    /// The address the CPU jumps to for it: `$40`, `$48`, `$50`, `$58`, `$60`
    pub fn vector(self) -> u16 {
        match self {
            Interrupt::VBlank => hw::VECTOR_VBLANK,
            Interrupt::Stat => hw::VECTOR_STAT,
            Interrupt::Timer => hw::VECTOR_TIMER,
            Interrupt::Serial => hw::VECTOR_SERIAL,
            Interrupt::Joypad => hw::VECTOR_JOYPAD,
        }
    }

    /// The label of its handler ([`handler`]): `VBlankHandler`, `StatHandler`,
    /// `TimerHandler`, `SerialHandler`, `JoypadHandler`
    pub fn handler_label(self) -> &'static str {
        match self {
            Interrupt::VBlank => "VBlankHandler",
            Interrupt::Stat => "StatHandler",
            Interrupt::Timer => "TimerHandler",
            Interrupt::Serial => "SerialHandler",
            Interrupt::Joypad => "JoypadHandler",
        }
    }

    /// The name of the section of its vector ([`vector`]): `VBlank interrupt`, ...
    pub fn section_name(self) -> &'static str {
        match self {
            Interrupt::VBlank => "VBlank interrupt",
            Interrupt::Stat => "STAT interrupt",
            Interrupt::Timer => "Timer interrupt",
            Interrupt::Serial => "Serial interrupt",
            Interrupt::Joypad => "Joypad interrupt",
        }
    }
}

/// The vector of `interrupt`: a `ROM0` section at its address ([`Interrupt::vector`]) with
/// `jp handler`, or with `reti` when there is no handler (the interrupt then only wakes a
/// `halt`)
pub fn vector(interrupt: Interrupt, handler: Option<&str>) -> Vec<Instr> {
    let mut asm = Block::new();
    asm.section(Section::rom0(interrupt.section_name()).at(interrupt.vector()));
    match handler {
        Some(label) => asm.jp(label),
        None => asm.reti(),
    };
    asm.into_instrs()
}

/// The register pairs `push` and `pop` save to keep every register of `regs`, in the order
/// they are pushed: `af` (for `a` or the flags), `bc`, `de`, `hl`
pub fn saved_pairs(regs: Regs) -> Vec<R16Stack> {
    [
        (Regs::AF, R16Stack::AF),
        (Regs::BC, R16Stack::BC),
        (Regs::DE, R16Stack::DE),
        (Regs::HL, R16Stack::HL),
    ]
    .into_iter()
    .filter(|(pair, _)| regs.intersects(*pair))
    .map(|(_, pair)| pair)
    .collect()
}

/// The handler of `interrupt`, the routine its vector jumps to: its label
/// ([`Interrupt::handler_label`]), a `push` of each pair that holds a register of `saves`,
/// `body`, the `pop`s in reverse order, then `reti`
///
/// `saves` must hold every register `body` may change, so that the code the interrupt
/// stops finds its registers as it left them: [`Regs::written_by`] of the body when it can
/// tell (`Regs::ALL` when it cannot: a `call`, raw code), or the [`Routine::changes`] of a
/// routine the body calls. The handler is a [`Routine`] that changes no register
/// (`clobbers` is [`Regs::NONE`]). It runs with interrupts off (the CPU turns them off to
/// dispatch it), and `reti` turns them on again.
///
/// # Panics
/// If `body` returns (`ret`, `ret cc` or `reti`): it runs inline, between the saves and
/// the restores.
#[track_caller]
pub fn handler(interrupt: Interrupt, body: impl Into<Vec<Instr>>, saves: Regs) -> Routine {
    let body = body.into();
    if let Some(ret) = body
        .iter()
        .find(|instr| matches!(instr, Instr::Ret | Instr::RetCond { .. } | Instr::Reti))
    {
        panic!(
            "the {} handler's code returns ({}): it runs between the saves and the restores of \
             the registers, which end with reti",
            interrupt.handler_label(),
            ret
        );
    }
    let pairs = saved_pairs(saves);
    let mut asm = Block::new();
    asm.label(interrupt.handler_label());
    for pair in &pairs {
        asm.push(*pair);
    }
    asm.emit_all(body);
    for pair in pairs.iter().rev() {
        asm.pop(*pair);
    }
    asm.reti();
    Routine::new(interrupt.handler_label(), asm).with_clobbers(Regs::NONE)
}

/// The byte the VBlank handler sets ([`signal_frame`]) and [`wait_frame`] waits for: a
/// `u8` variable of the `WaitFrame` routine
pub const VBLANK_FLAG: &str = "wVBlankFlag";

/// The code a VBlank handler runs to tell [`wait_frame`] that a frame started: it sets
/// [`VBLANK_FLAG`] (uses `a`)
pub fn signal_frame() -> Vec<Instr> {
    let mut asm = Block::new();
    asm.ld_a(1);
    asm.ld_addr_def_a(VBLANK_FLAG);
    asm.into_instrs()
}

/// The `WaitFrame` routine: wait for the next VBlank, asleep (`halt`)
///
/// It makes sure the VBlank interrupt is enabled (it sets `IEF_VBLANK` in `rIE`, keeping
/// the other bits), clears [`VBLANK_FLAG`], then sleeps until the VBlank handler has set it
/// again ([`signal_frame`]). So it returns after the handler of the first VBlank that
/// starts once it has cleared the flag: about one frame later at most (a VBlank that starts
/// during its first instructions is not that one, as with the `rLY` polling it replaces).
/// The other interrupts wake the `halt` too: their handlers run, and it goes back to sleep.
///
/// It checks the flag with interrupts off and sleeps with them off, so a VBlank that comes
/// between the check and the `halt` is not missed: with interrupts off, `halt` ends as soon
/// as an enabled interrupt is pending (at once if one already is) without taking it, then
/// `ei` lets the CPU take it before the flag is checked again. (When one is already pending,
/// the DMG's halt bug runs the `ei` after the `halt` twice, which changes nothing.) A
/// check-then-`halt` loop with interrupts on would sleep through that frame.
///
/// - Needs: a VBlank handler that sets the flag, and the LCD on; otherwise it never
///   returns. It enables the VBlank interrupt itself, so code that wrote `rIE` without
///   `IEF_VBLANK` does not stop the main loop.
/// - Returns with interrupts on (`ei`), whatever their state at the call.
/// - Reads and returns nothing; clobbers `a` and the flags.
pub fn wait_frame() -> Routine {
    let mut asm = Block::new();
    asm.comment("Wait for the next VBlank, asleep: the VBlank handler sets wVBlankFlag");
    asm.label("WaitFrame");
    // The frame clock: whatever wrote rIE, VBlank stays enabled
    asm.ldh(R8::A, Mem::addr(hw::IE));
    asm.or(hw::IEF_VBLANK);
    asm.ldh(Mem::addr(hw::IE), R8::A);
    asm.xor(R8::A);
    asm.ld_addr_def_a(VBLANK_FLAG);
    asm.label(".wait");
    asm.di();
    asm.ld_a_addr_def(VBLANK_FLAG);
    asm.and(R8::A);
    asm.jr_cond(Condition::NZ, ".done");
    // Interrupts off: halt ends when one is pending, without taking it
    asm.halt();
    // Take it (after the jr), then check again
    asm.ei();
    asm.jr(".wait");
    asm.label(".done");
    asm.ei();
    asm.ret();
    Routine::new("WaitFrame", asm)
        .with_variable(VBLANK_FLAG)
        .with_clobbers(Regs::A | Regs::F)
}

/// The `rIE` / `rIF` bits of `interrupts`, as an expression: `IEF_VBLANK | IEF_TIMER`
/// (each once, in priority order; `0` for none)
pub fn mask(interrupts: &[Interrupt]) -> Expr {
    Interrupt::ALL
        .into_iter()
        .filter(|interrupt| interrupts.contains(interrupt))
        .map(|interrupt| Expr::from(interrupt.flag()))
        .reduce(|mask, flag| mask | flag)
        .unwrap_or_else(|| Expr::from(0u8))
}

/// Code that enables exactly `interrupts` in `rIE` (and disables the others): `ld a, mask`,
/// `ldh [rIE], a`. Uses `a`.
pub fn set_enabled(interrupts: &[Interrupt]) -> Vec<Instr> {
    let mut asm = Block::new();
    asm.ld(R8::A, mask(interrupts));
    asm.ldh(Mem::addr(hw::IE), R8::A);
    asm.into_instrs()
}

/// Code that enables `interrupts` in `rIE`, keeping the others as they are. Uses `a` and
/// the flags.
pub fn enable(interrupts: &[Interrupt]) -> Vec<Instr> {
    let mut asm = Block::new();
    asm.ldh(R8::A, Mem::addr(hw::IE));
    asm.or(mask(interrupts));
    asm.ldh(Mem::addr(hw::IE), R8::A);
    asm.into_instrs()
}

/// Code that disables `interrupts` in `rIE`, keeping the others as they are. Uses `a` and
/// the flags. (A request that is already pending stays in `rIF`, and is taken when the
/// interrupt is enabled again, unless [`clear_pending`] drops it.)
pub fn disable(interrupts: &[Interrupt]) -> Vec<Instr> {
    let mut asm = Block::new();
    asm.ldh(R8::A, Mem::addr(hw::IE));
    asm.and(Expr::low(!mask(interrupts)));
    asm.ldh(Mem::addr(hw::IE), R8::A);
    asm.into_instrs()
}

/// Code that drops every pending interrupt request (`rIF` = 0), to run before `ei`: the
/// hardware requests interrupts whether they are enabled or not (the VBlank of every
/// frame, a timer overflow), and a request left there would be taken at once. Uses `a` and
/// the flags.
pub fn clear_pending() -> Vec<Instr> {
    let mut asm = Block::new();
    asm.xor(R8::A);
    asm.ldh(Mem::addr(hw::IF), R8::A);
    asm.into_instrs()
}

/// The conditions that request the STAT interrupt ([`Interrupt::Stat`]): `LY` = `LYC` on
/// a line, and the PPU modes, any number of them (bits 3-6 of `rSTAT`)
///
/// The interrupt is requested when the OR of the selected conditions goes from false to
/// true: two conditions that follow each other without a gap (mode 0 of a line, then the
/// LYC line's start) request it once.
///
/// # Example
/// ```
/// use rust_boy::stdlib::interrupts::{Stat, configure_stat};
///
/// let stat = Stat::new().line(100).hblank();
/// let text: Vec<String> = configure_stat(stat).iter().map(|i| i.to_string()).collect();
/// assert_eq!(
///     text,
///     ["ld a, 100", "ldh [rLYC], a", "ld a, STATF_LYC | STATF_MODE00", "ldh [rSTAT], a"]
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stat {
    lyc: Option<u8>,
    hblank: bool,
    vblank: bool,
    oam_scan: bool,
}

impl Stat {
    /// No condition
    pub fn new() -> Stat {
        Stat::default()
    }

    /// With `LY` = `LYC` on line `line` (`STATF_LYC`): at the start of that line
    ///
    /// # Panics
    /// If `line` is above [`hw::LY_LAST_LINE`]: `LY` goes from 0 to that line.
    #[track_caller]
    pub fn line(mut self, line: u8) -> Stat {
        assert!(
            line <= hw::LY_LAST_LINE,
            "LYC {} is never reached: LY goes from 0 to {}",
            line,
            hw::LY_LAST_LINE
        );
        self.lyc = Some(line);
        self
    }

    /// With mode 0, HBlank (`STATF_MODE00`): at the end of each visible line
    pub fn hblank(mut self) -> Stat {
        self.hblank = true;
        self
    }

    /// With mode 1, VBlank (`STATF_MODE01`): at the start of VBlank, with the VBlank
    /// interrupt
    pub fn vblank(mut self) -> Stat {
        self.vblank = true;
        self
    }

    /// With mode 2, the OAM scan (`STATF_MODE10`): at the start of each visible line
    pub fn oam_scan(mut self) -> Stat {
        self.oam_scan = true;
        self
    }

    /// The line of the `LY` = `LYC` condition, if it is selected
    pub fn lyc(&self) -> Option<u8> {
        self.lyc
    }

    /// The `rSTAT` bits of the selected conditions, as an expression (`0` for none)
    pub fn sources(&self) -> Expr {
        [
            (self.lyc.is_some(), hw::STATF_LYC),
            (self.oam_scan, hw::STATF_MODE10),
            (self.vblank, hw::STATF_MODE01),
            (self.hblank, hw::STATF_MODE00),
        ]
        .into_iter()
        .filter(|(selected, _)| *selected)
        .map(|(_, flag)| Expr::from(flag))
        .reduce(|sources, flag| sources | flag)
        .unwrap_or_else(|| Expr::from(0u8))
    }
}

/// Code that selects the conditions of `stat`: `rLYC` (if it has a line), then `rSTAT`.
/// Uses `a`.
///
/// On the DMG, a write to `rSTAT` in mode 0 or 1 (HBlank, VBlank), or while `LY` = `LYC`,
/// requests a STAT interrupt of its own (a hardware quirk): with the STAT interrupt
/// enabled, its handler runs once more. Write it with the LCD off, or drop the request
/// ([`clear_pending`]) before interrupts are on, as `RustBoy`'s start-up code does.
pub fn configure_stat(stat: Stat) -> Vec<Instr> {
    let mut asm = Block::new();
    if let Some(line) = stat.lyc {
        asm.ld_a(line);
        asm.ldh(Mem::addr(hw::LYC), R8::A);
    }
    asm.ld(R8::A, stat.sources());
    asm.ldh(Mem::addr(hw::STAT), R8::A);
    asm.into_instrs()
}

/// The clock of the timer: how often `rTIMA` counts up (`rTAC` bits 0-1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimerClock {
    /// 4096 Hz: one step every 256 M-cycles (`TACF_4KHZ`)
    Hz4096,
    /// 16384 Hz: one step every 64 M-cycles (`TACF_16KHZ`)
    Hz16384,
    /// 65536 Hz: one step every 16 M-cycles (`TACF_65KHZ`)
    Hz65536,
    /// 262144 Hz: one step every 4 M-cycles (`TACF_262KHZ`)
    Hz262144,
}

impl TimerClock {
    /// Its `rTAC` bits: `TACF_4KHZ`, `TACF_16KHZ`, `TACF_65KHZ`, `TACF_262KHZ`
    pub fn flag(self) -> hw::Symbol<u8> {
        match self {
            TimerClock::Hz4096 => hw::TACF_4KHZ,
            TimerClock::Hz16384 => hw::TACF_16KHZ,
            TimerClock::Hz65536 => hw::TACF_65KHZ,
            TimerClock::Hz262144 => hw::TACF_262KHZ,
        }
    }

    /// The M-cycles (1 MiHz) between two steps of `rTIMA`
    pub fn mcycles_per_step(self) -> u32 {
        match self {
            TimerClock::Hz4096 => 256,
            TimerClock::Hz16384 => 64,
            TimerClock::Hz65536 => 16,
            TimerClock::Hz262144 => 4,
        }
    }
}

/// The timer: `rTIMA` counts up at `clock` from `modulo` (`rTMA`), and each time it
/// overflows it starts from `modulo` again and requests the timer interrupt
/// ([`Interrupt::Timer`]), every [`period_mcycles`](Self::period_mcycles)
///
/// # Example
/// ```
/// use rust_boy::stdlib::interrupts::{Timer, TimerClock};
///
/// // 64 steps of 256 M-cycles: 64 interrupts a second (a second is 1048576 M-cycles)
/// let timer = Timer::new(TimerClock::Hz4096, 192);
/// assert_eq!(timer.period_mcycles(), 16384);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timer {
    /// How often `rTIMA` counts up
    pub clock: TimerClock,
    /// Where `rTIMA` starts from (`rTMA`): `256 - modulo` steps between two interrupts
    pub modulo: u8,
}

impl Timer {
    /// The timer counting at `clock` from `modulo`
    pub fn new(clock: TimerClock, modulo: u8) -> Timer {
        Timer { clock, modulo }
    }

    /// The M-cycles between two of its interrupts: `(256 - modulo)` steps of the clock
    pub fn period_mcycles(&self) -> u32 {
        (256 - u32::from(self.modulo)) * self.clock.mcycles_per_step()
    }
}

/// Code that starts `timer`: `rTMA` and `rTIMA` = its modulo (so the first interrupt comes
/// after a whole period), then `rTAC` = `TACF_START` and its clock. Uses `a`.
pub fn start_timer(timer: Timer) -> Vec<Instr> {
    let mut asm = Block::new();
    asm.ld_a(timer.modulo);
    asm.ldh(Mem::addr(hw::TMA), R8::A);
    asm.ldh(Mem::addr(hw::TIMA), R8::A);
    asm.ld(R8::A, Expr::from(hw::TACF_START) | timer.clock.flag());
    asm.ldh(Mem::addr(hw::TAC), R8::A);
    asm.into_instrs()
}

/// Code that stops the timer (`rTAC` = `TACF_STOP`). Uses `a`.
pub fn stop_timer() -> Vec<Instr> {
    let mut asm = Block::new();
    asm.ld(R8::A, hw::TACF_STOP);
    asm.ldh(Mem::addr(hw::TAC), R8::A);
    asm.into_instrs()
}

/// Tests of the interrupt pieces: their text, and what they save. What they do on the
/// hardware is checked on the test emulator (`tests/emulator.rs`).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::Block;

    fn text(code: &[Instr]) -> Vec<String> {
        code.iter().map(|instr| instr.to_string()).collect()
    }

    #[test]
    fn test_interrupts_in_priority_order() {
        // The bits of rIE / rIF and the vectors go up with the priority
        let flags: Vec<u8> = Interrupt::ALL.iter().map(|i| i.flag().value).collect();
        assert_eq!(flags, [0x01, 0x02, 0x04, 0x08, 0x10]);
        let vectors: Vec<u16> = Interrupt::ALL.iter().map(|i| i.vector()).collect();
        assert_eq!(vectors, [0x40, 0x48, 0x50, 0x58, 0x60]);
        let mut sorted = Interrupt::ALL;
        sorted.sort();
        assert_eq!(sorted, Interrupt::ALL, "Ord is the priority order");
    }

    #[test]
    fn test_vector_with_and_without_a_handler() {
        assert_eq!(
            text(&vector(Interrupt::VBlank, Some("VBlankHandler"))),
            [
                "SECTION \"VBlank interrupt\", ROM0[$40]",
                "jp VBlankHandler"
            ]
        );
        assert_eq!(
            text(&vector(Interrupt::Joypad, None)),
            ["SECTION \"Joypad interrupt\", ROM0[$60]", "reti"]
        );
    }

    #[test]
    fn test_a_handler_saves_the_pairs_of_the_registers_it_is_given() {
        let mut body = Block::new();
        body.ld_a(1).ld_e(2);
        // a and e: af and de, pushed in that order and popped in reverse
        let handler = handler(Interrupt::Stat, body, Regs::A | Regs::E);
        assert_eq!(
            text(handler.body()),
            [
                "StatHandler:",
                "push af",
                "push de",
                "ld a, 1",
                "ld e, 2",
                "pop de",
                "pop af",
                "reti"
            ]
        );
        assert_eq!(handler.name(), "StatHandler");
        assert_eq!(handler.changes(), Regs::NONE, "a handler changes nothing");

        assert_eq!(saved_pairs(Regs::NONE), []);
        assert_eq!(saved_pairs(Regs::F), [R16Stack::AF]);
        assert_eq!(saved_pairs(Regs::L | Regs::C), [R16Stack::BC, R16Stack::HL]);
        assert_eq!(
            saved_pairs(Regs::ALL),
            [R16Stack::AF, R16Stack::BC, R16Stack::DE, R16Stack::HL]
        );
    }

    #[test]
    fn test_a_handler_body_cannot_return() {
        for code in [
            |asm: &mut Block| {
                asm.ret();
            },
            |asm: &mut Block| {
                asm.reti();
            },
            |asm: &mut Block| {
                asm.ret_cond(Condition::Z);
            },
        ] {
            let mut body = Block::new();
            body.ld_a(1);
            code(&mut body);
            let message = crate::engine::panic_message(|| handler(Interrupt::Timer, body, Regs::A));
            assert!(
                message.contains("TimerHandler handler's code returns"),
                "{}",
                message
            );
        }
    }

    #[test]
    fn test_wait_frame_checks_and_sleeps_with_interrupts_off() {
        let routine = wait_frame();
        assert_eq!(routine.variables(), [VBLANK_FLAG]);
        assert_eq!(routine.changes(), Regs::A | Regs::F);
        let body = text(routine.body());
        let code: Vec<&str> = body
            .iter()
            .map(String::as_str)
            .filter(|line| !line.starts_with(';'))
            .collect();
        assert_eq!(
            code,
            [
                "WaitFrame:",
                "ldh a, [rIE]",
                "or a, IEF_VBLANK",
                "ldh [rIE], a",
                "xor a, a",
                "ld [wVBlankFlag], a",
                ".wait:",
                "di",
                "ld a, [wVBlankFlag]",
                "and a, a",
                "jr nz, .done",
                "halt",
                "ei",
                "jr .wait",
                ".done:",
                "ei",
                "ret"
            ]
        );
        assert_eq!(text(&signal_frame()), ["ld a, 1", "ld [wVBlankFlag], a"]);
    }

    #[test]
    fn test_enable_and_disable() {
        assert_eq!(mask(&[]).to_string(), "0");
        // Each once, in priority order, whatever the order given
        assert_eq!(
            mask(&[Interrupt::Timer, Interrupt::VBlank, Interrupt::Timer]).to_string(),
            "IEF_VBLANK | IEF_TIMER"
        );
        assert_eq!(
            text(&set_enabled(&[Interrupt::VBlank, Interrupt::Joypad])),
            ["ld a, IEF_VBLANK | IEF_HILO", "ldh [rIE], a"]
        );
        assert_eq!(
            text(&enable(&[Interrupt::Stat])),
            ["ldh a, [rIE]", "or a, IEF_STAT", "ldh [rIE], a"]
        );
        assert_eq!(
            text(&disable(&[Interrupt::Stat, Interrupt::Timer])),
            [
                "ldh a, [rIE]",
                "and a, LOW(~(IEF_STAT | IEF_TIMER))",
                "ldh [rIE], a"
            ]
        );
        assert_eq!(text(&clear_pending()), ["xor a, a", "ldh [rIF], a"]);
    }

    #[test]
    fn test_stat_sources() {
        assert_eq!(Stat::new().sources().to_string(), "0");
        assert_eq!(Stat::new().lyc(), None);
        let all = Stat::new().oam_scan().hblank().vblank().line(0);
        assert_eq!(
            all.sources().to_string(),
            "STATF_LYC | STATF_MODE10 | STATF_MODE01 | STATF_MODE00"
        );
        assert_eq!(all.lyc(), Some(0));
        // No line: rSTAT only
        assert_eq!(
            text(&configure_stat(Stat::new().hblank())),
            ["ld a, STATF_MODE00", "ldh [rSTAT], a"]
        );
        assert_eq!(Stat::new().line(153).lyc(), Some(153));
        let message = crate::engine::panic_message(|| Stat::new().line(154));
        assert!(message.contains("LYC 154 is never reached"), "{}", message);
    }

    #[test]
    fn test_timer() {
        let clocks = [
            (TimerClock::Hz4096, "TACF_4KHZ", 256),
            (TimerClock::Hz16384, "TACF_16KHZ", 64),
            (TimerClock::Hz65536, "TACF_65KHZ", 16),
            (TimerClock::Hz262144, "TACF_262KHZ", 4),
        ];
        for (clock, flag, step) in clocks {
            assert_eq!(clock.flag().name, flag);
            assert_eq!(clock.mcycles_per_step(), step);
            // A step every `step` M-cycles: 4194304 Hz / 4 / step
            assert_eq!(1_048_576 / step, clock_hz(clock));
        }
        assert_eq!(Timer::new(TimerClock::Hz4096, 0).period_mcycles(), 65536);
        assert_eq!(Timer::new(TimerClock::Hz262144, 255).period_mcycles(), 4);
        assert_eq!(
            text(&start_timer(Timer::new(TimerClock::Hz65536, 0xC0))),
            [
                "ld a, 192",
                "ldh [rTMA], a",
                "ldh [rTIMA], a",
                "ld a, TACF_START | TACF_65KHZ",
                "ldh [rTAC], a"
            ]
        );
        assert_eq!(text(&stop_timer()), ["ld a, TACF_STOP", "ldh [rTAC], a"]);
    }

    /// The frequency a clock's name gives
    fn clock_hz(clock: TimerClock) -> u32 {
        match clock {
            TimerClock::Hz4096 => 4096,
            TimerClock::Hz16384 => 16384,
            TimerClock::Hz65536 => 65536,
            TimerClock::Hz262144 => 262144,
        }
    }

    /// Every piece, assembled (`-Weverything -Werror`) and linked by RGBDS (with
    /// `RGBDS_LINK_CHECK`), in a program that uses them all
    #[test]
    fn test_the_pieces_assemble() {
        use crate::asm::Asm;
        let mut asm = Asm::new();
        asm.include_hardware();
        for interrupt in Interrupt::ALL {
            let handler = (interrupt != Interrupt::Serial).then_some(interrupt.handler_label());
            asm.emit_all(vector(interrupt, handler));
        }
        asm.section(Section::rom0("Header").at(hw::ROM_HEADER))
            .jp("EntryPoint")
            .ds_fill(&format!("${:X} - @", hw::ROM_HEADER_END), "0");
        asm.label("EntryPoint").di();
        asm.emit_all(configure_stat(Stat::new().line(10).hblank()));
        asm.emit_all(start_timer(Timer::new(TimerClock::Hz16384, 7)));
        asm.emit_all(set_enabled(&Interrupt::ALL));
        asm.emit_all(disable(&[Interrupt::Serial]));
        asm.emit_all(enable(&[Interrupt::Timer]));
        asm.emit_all(clear_pending());
        asm.ei();
        asm.label("Main").call("WaitFrame");
        asm.emit_all(stop_timer());
        asm.jp("Main");
        asm.emit_all(wait_frame());
        let mut vblank = Block::new();
        vblank.emit_all(signal_frame()).ld_bc(1);
        asm.emit_all(handler(Interrupt::VBlank, vblank, Regs::ALL));
        for interrupt in [Interrupt::Stat, Interrupt::Timer, Interrupt::Joypad] {
            let mut nop = Block::new();
            nop.nop();
            asm.emit_all(handler(interrupt, nop, Regs::NONE));
        }
        asm.section(Section::wram0("Variables"))
            .label(VBLANK_FLAG)
            .ds("1");
        let program = asm.to_asm();
        crate::asm::label_check::assert_links(&program);
        // Every warning an error (a truncated `and a, LOW(~..)` too)
        crate::asm::label_check::rgbds_rom(&program);
    }
}
