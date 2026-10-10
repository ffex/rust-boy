//! A headless Game Boy (DMG) for tests: it runs a ROM and lets a test read its memory
//! and registers.
//!
//! What it models:
//! - The whole SM83 instruction set, M-cycle by M-cycle: every memory access takes one
//!   M-cycle, and the PPU, the timer and the OAM DMA advance with it, so an access sees
//!   the PPU mode of the cycle it happens in. Checked with Blargg's `cpu_instrs` (all 11),
//!   `instr_timing` and `mem_timing` ROMs (`tests/emulator.rs`, with `GB_TEST_ROMS`).
//! - Interrupts (VBlank, STAT, timer, serial, joypad), dispatched in 5 M-cycles (6 when
//!   they end a `halt`). `ei` takes effect after the next instruction, which a `di`
//!   cancels. `halt` waits for an interrupt; with one already pending it does not stop:
//!   with IME set the interrupt is taken, with IME clear the halt bug reads the next
//!   opcode twice, and right after `ei` the interrupt is taken and returns to the
//!   `halt`, which runs again (Pan Docs, "halt bug"). Checked by `tests/emulator.rs`.
//! - The timer: `DIV`, and `TIMA` counting on the falling edge of its `DIV` bit, which
//!   after an overflow reads 0 for one M-cycle before the reload from `TMA` and the
//!   interrupt (a write to `TIMA` in that M-cycle cancels them).
//! - The PPU's timing, not its picture: 154 lines of 456 dots, mode 2 (80 dots), mode 3
//!   (172 dots, its shortest length), mode 0 for the rest of the line, mode 1 (VBlank)
//!   on lines 144-153, `LY`, `LYC`, `STAT` and its interrupt (on the rising edge of the
//!   OR of its sources). With the LCD off, `LY` is 0 and the CPU can reach VRAM and OAM at
//!   any time; the first line after the LCD is turned on has no OAM scan (mode 0, OAM
//!   free, where mode 2 would be).
//! - What the hardware blocks: with the LCD on, a CPU write to OAM in mode 2 or 3 is
//!   dropped and a read gives `$FF`; the same for VRAM in mode 3. An OAM DMA written in
//!   M-cycle `w` copies byte `i` in M-cycle `w + 2 + i`, and from `w + 2` to `w + 161`
//!   the CPU cannot reach OAM nor the bus the DMA reads (VRAM, or the external bus:
//!   ROM, cartridge RAM, WRAM); a read there gives `$FF`, a write is dropped. Each
//!   blocked access is counted, and every CPU write to OAM is logged ([`OamWrite`]), so a
//!   test can see when a program touches OAM outside VBlank and HBlank ([B12]).
//! - The joypad (`P1`), the serial port (the bytes a program sends are collected, as
//!   Blargg's ROMs print through it), and the cartridge: ROM only (with RAM for types
//!   `$08` and `$09`), MBC1 and MBC5 ROM banking, 32 KiB of cartridge RAM.
//!
//! Known limits (not modelled):
//! - The picture: no pixels, no sprite or background fetch, so mode 3 always lasts its
//!   shortest 172 dots (on hardware sprites and scrolling lengthen it, and mode 0 shrinks).
//! - The first line after the LCD is turned on has the length of any other line.
//! - The DMG `STAT`-write quirk (a write to `STAT` requests a STAT interrupt in modes 0
//!   and 1), and the mode-2 STAT interrupt that line 144 also requests on the DMG.
//! - A wake-up from `halt` with IME clear takes no extra M-cycle.
//! - Audio (the sound registers are plain bytes), `stop` (a 2-byte `nop`), the OAM
//!   corruption bug, MBC1 banking mode 1, the boot ROM (the emulator starts at `$0100`
//!   with the registers the DMG boot ROM leaves).
//! - An illegal opcode panics, so a test never passes by running garbage.
//!
//! [B12]: ../../CONTEXT.md#b12

use std::collections::BTreeMap;

/// Dots (4 MHz clocks) in one M-cycle
const DOTS_PER_MCYCLE: u64 = 4;
/// M-cycles in one line: 456 dots
const LINE_MCYCLES: u32 = 456 / 4;
/// M-cycles of mode 2 (OAM scan) at the start of a visible line: 80 dots
const MODE2_MCYCLES: u32 = 80 / 4;
/// M-cycles of mode 3 (drawing) after mode 2: 172 dots, its shortest length
const MODE3_MCYCLES: u32 = 172 / 4;
/// M-cycles in one frame: 154 lines of 456 dots (70224 dots)
pub const FRAME_MCYCLES: u64 = 154 * 456 / DOTS_PER_MCYCLE;

/// Interrupt flags (`IF`, `IE`)
const INT_VBLANK: u8 = 0x01;
const INT_STAT: u8 = 0x02;
const INT_TIMER: u8 = 0x04;
const INT_SERIAL: u8 = 0x08;
const INT_JOYPAD: u8 = 0x10;

/// CPU flags in `f`
const FLAG_Z: u8 = 0x80;
const FLAG_N: u8 = 0x40;
const FLAG_H: u8 = 0x20;
const FLAG_C: u8 = 0x10;

/// A joypad button
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

impl Button {
    /// Its bit in [`Bus::buttons`]: the directions in the low nibble (as `P1` reads them
    /// with the directions selected), the buttons in the high one
    fn mask(self) -> u8 {
        match self {
            Button::Right => 0x01,
            Button::Left => 0x02,
            Button::Up => 0x04,
            Button::Down => 0x08,
            Button::A => 0x10,
            Button::B => 0x20,
            Button::Select => 0x40,
            Button::Start => 0x80,
        }
    }
}

/// The PPU mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Mode 0: the end of a visible line; VRAM and OAM are free
    HBlank,
    /// Mode 1: lines 144-153; VRAM and OAM are free
    VBlank,
    /// Mode 2: the PPU reads OAM; the CPU cannot
    OamScan,
    /// Mode 3: the PPU reads OAM and VRAM; the CPU can reach neither
    Drawing,
}

impl Mode {
    fn bits(self) -> u8 {
        match self {
            Mode::HBlank => 0,
            Mode::VBlank => 1,
            Mode::OamScan => 2,
            Mode::Drawing => 3,
        }
    }
}

/// A CPU write to OAM, and when it happened
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OamWrite {
    /// The frame it happened in (M-cycles since power-on / [`FRAME_MCYCLES`])
    pub frame: u64,
    /// M-cycles since power-on
    pub cycle: u64,
    /// The address of the instruction that wrote
    pub pc: u16,
    /// The address written, `$FE00`-`$FE9F`
    pub address: u16,
    pub value: u8,
    /// `LY` when it happened
    pub ly: u8,
    /// The PPU mode when it happened (`HBlank` when the LCD is off)
    pub mode: Mode,
    /// Whether the LCD was on
    pub lcd_on: bool,
    /// Whether the hardware dropped it: LCD on and mode 2 or 3, or an OAM DMA running
    pub dropped: bool,
}

/// The CPU registers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    /// The interrupt master enable
    pub ime: bool,
    pub halted: bool,
}

impl Registers {
    pub fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }
    pub fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }
    pub fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }
    pub fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }
}

/// The cartridge's memory bank controller
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mbc {
    None,
    Mbc1,
    Mbc5,
}

/// Everything but the CPU: memory, the PPU's timing, the timer, the joypad, DMA, serial
struct Bus {
    rom: Vec<u8>,
    mbc: Mbc,
    /// The ROM bank at `$4000`-`$7FFF`
    rom_bank: usize,
    /// MBC1: the upper two bits of the ROM bank
    mbc1_upper: usize,
    cart_ram: Vec<u8>,
    cart_ram_bank: usize,
    cart_ram_enabled: bool,
    vram: [u8; 0x2000],
    wram: [u8; 0x2000],
    oam: [u8; 0xA0],
    hram: [u8; 0x7F],
    /// `$FF00`-`$FF7F`, for the registers without their own field
    io: [u8; 0x80],
    ie: u8,
    /// `IF`, the low 5 bits
    interrupt_flag: u8,

    // PPU
    lcdc: u8,
    /// `STAT` bits 3-6, the interrupt sources
    stat_select: u8,
    ly: u8,
    lyc: u8,
    /// M-cycles into the current line
    line_cycle: u32,
    /// The first line after the LCD is turned on, which has no OAM scan: mode 0 where
    /// mode 2 would be, OAM free
    first_line: bool,
    /// The STAT interrupt line, to request the interrupt on its rising edge
    stat_line: bool,

    // Timer
    /// The 16-bit counter whose upper byte is `DIV`
    div_counter: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    /// `TIMA` overflowed in the last M-cycle: it reads 0 now, and is reloaded from `TMA`
    /// (with the interrupt) in this one
    tima_reload: bool,

    // Joypad
    /// The pressed buttons ([`Button::mask`])
    buttons: u8,
    /// `P1` bits 4-5, which group is selected
    p1_select: u8,

    // OAM DMA
    /// The source of the DMA running
    dma_source: u16,
    /// The source of a requested DMA, until it starts
    dma_next_source: u16,
    /// While a DMA runs: the next byte to copy (160 in its last busy M-cycle)
    dma_index: Option<u16>,
    /// M-cycles before a requested DMA starts
    dma_delay: u8,

    // Serial
    serial_out: Vec<u8>,

    // Bookkeeping
    /// M-cycles since power-on
    cycles: u64,
    /// The address of the instruction running, for [`OamWrite::pc`]
    current_pc: u16,
    oam_writes: Vec<OamWrite>,
    blocked_vram_writes: u64,
    blocked_vram_reads: u64,
    blocked_oam_reads: u64,
    dma_conflicts: u64,
}

impl Bus {
    fn new(rom: Vec<u8>) -> Bus {
        assert!(
            rom.len() >= 0x150,
            "a ROM has at least its header, $150 bytes"
        );
        let mbc = match rom[0x147] {
            0x00 | 0x08 | 0x09 => Mbc::None,
            0x01..=0x03 => Mbc::Mbc1,
            0x19..=0x1E => Mbc::Mbc5,
            other => panic!("cartridge type ${:02X} is not modelled", other),
        };
        // ROM + RAM without an MBC: the RAM needs no enable
        let ram_without_mbc = matches!(rom[0x147], 0x08 | 0x09);
        let mut io = [0u8; 0x80];
        // What the DMG boot ROM leaves in the registers this module keeps as plain bytes
        io[0x01] = 0x00; // SB
        io[0x02] = 0x7E; // SC
        io[0x10] = 0x80; // NR10
        io[0x11] = 0xBF;
        io[0x12] = 0xF3;
        io[0x14] = 0xBF;
        io[0x16] = 0x3F;
        io[0x19] = 0xBF;
        io[0x1A] = 0x7F;
        io[0x1B] = 0xFF;
        io[0x1C] = 0x9F;
        io[0x1E] = 0xBF;
        io[0x20] = 0xFF;
        io[0x23] = 0xBF;
        io[0x24] = 0x77;
        io[0x25] = 0xF3;
        io[0x26] = 0xF1;
        io[0x47] = 0xFC; // BGP
        io[0x48] = 0xFF; // OBP0
        io[0x49] = 0xFF; // OBP1
        Bus {
            rom,
            mbc,
            rom_bank: 1,
            mbc1_upper: 0,
            cart_ram: vec![0; 0x8000],
            cart_ram_bank: 0,
            cart_ram_enabled: ram_without_mbc,
            vram: [0; 0x2000],
            wram: [0; 0x2000],
            oam: [0; 0xA0],
            hram: [0; 0x7F],
            io,
            ie: 0,
            interrupt_flag: INT_VBLANK,
            lcdc: 0x91,
            stat_select: 0,
            ly: 0,
            lyc: 0,
            line_cycle: 0,
            first_line: false,
            stat_line: false,
            div_counter: 0xABCC,
            tima: 0,
            tma: 0,
            tac: 0xF8,
            tima_reload: false,
            buttons: 0,
            p1_select: 0x30,
            dma_source: 0,
            dma_next_source: 0,
            dma_index: None,
            dma_delay: 0,
            serial_out: Vec::new(),
            cycles: 0,
            current_pc: 0x0100,
            oam_writes: Vec::new(),
            blocked_vram_writes: 0,
            blocked_vram_reads: 0,
            blocked_oam_reads: 0,
            dma_conflicts: 0,
        }
    }

    fn lcd_on(&self) -> bool {
        self.lcdc & 0x80 != 0
    }

    fn mode(&self) -> Mode {
        if !self.lcd_on() {
            Mode::HBlank
        } else if self.ly >= 144 {
            Mode::VBlank
        } else if self.line_cycle < MODE2_MCYCLES {
            if self.first_line {
                Mode::HBlank
            } else {
                Mode::OamScan
            }
        } else if self.line_cycle < MODE2_MCYCLES + MODE3_MCYCLES {
            Mode::Drawing
        } else {
            Mode::HBlank
        }
    }

    fn oam_blocked(&self) -> bool {
        self.dma_index.is_some() || matches!(self.mode(), Mode::OamScan | Mode::Drawing)
    }

    /// Whether a running OAM DMA keeps the CPU from `address`: OAM, and the bus the DMA
    /// reads from (VRAM, or the external bus: ROM, cartridge RAM, WRAM); HRAM and the I/O
    /// registers stay free
    fn dma_blocks(&self, address: u16) -> bool {
        if self.dma_index.is_none() {
            return false;
        }
        let from_vram = (0x8000..=0x9FFF).contains(&self.dma_source);
        match address {
            0xFE00..=0xFEFF => true,
            0xFF00..=0xFFFF => false,
            0x8000..=0x9FFF => from_vram,
            _ => !from_vram,
        }
    }

    fn vram_blocked(&self) -> bool {
        self.mode() == Mode::Drawing
    }

    /// One M-cycle of everything but the CPU
    fn tick(&mut self) {
        self.cycles += 1;
        self.tick_timer();
        self.tick_dma();
        self.tick_ppu();
    }

    fn timer_bit(&self) -> u16 {
        match self.tac & 0x03 {
            0 => 1 << 9,
            1 => 1 << 3,
            2 => 1 << 5,
            _ => 1 << 7,
        }
    }

    /// Whether the timer's input is high: its bit of the counter, if the timer is on
    fn timer_input(&self) -> bool {
        self.tac & 0x04 != 0 && self.div_counter & self.timer_bit() != 0
    }

    fn tick_timer(&mut self) {
        if self.tima_reload {
            self.tima_reload = false;
            self.tima = self.tma;
            self.interrupt_flag |= INT_TIMER;
        }
        let before = self.timer_input();
        self.div_counter = self.div_counter.wrapping_add(DOTS_PER_MCYCLE as u16);
        if before && !self.timer_input() {
            self.increment_tima();
        }
    }

    fn increment_tima(&mut self) {
        let (tima, overflow) = self.tima.overflowing_add(1);
        // On overflow TIMA reads 0 for one M-cycle, then the reload and the interrupt
        self.tima = tima;
        self.tima_reload = overflow;
    }

    /// A DMA requested in M-cycle `w` (the write to `DMA`) copies byte `i` in M-cycle
    /// `w + 2 + i` and keeps the CPU off its buses from `w + 2` to `w + 161`
    fn tick_dma(&mut self) {
        if self.dma_delay > 0 {
            self.dma_delay -= 1;
            if self.dma_delay == 0 {
                // It starts (a DMA already running stops here)
                self.dma_source = self.dma_next_source;
                self.dma_index = Some(0);
            }
        }
        if let Some(index) = self.dma_index {
            if index == 0xA0 {
                self.dma_index = None;
            } else {
                let byte = self.peek(self.dma_source.wrapping_add(index));
                self.oam[usize::from(index)] = byte;
                self.dma_index = Some(index + 1);
            }
        }
    }

    fn tick_ppu(&mut self) {
        if !self.lcd_on() {
            return;
        }
        self.line_cycle += 1;
        if self.line_cycle == LINE_MCYCLES {
            self.line_cycle = 0;
            self.first_line = false;
            self.ly = if self.ly == 153 { 0 } else { self.ly + 1 };
            if self.ly == 144 {
                self.interrupt_flag |= INT_VBLANK;
            }
        }
        self.update_stat_line();
    }

    fn update_stat_line(&mut self) {
        let mode = self.mode();
        let line = self.lcd_on()
            && ((self.stat_select & 0x08 != 0 && mode == Mode::HBlank)
                || (self.stat_select & 0x10 != 0 && mode == Mode::VBlank)
                || (self.stat_select & 0x20 != 0 && mode == Mode::OamScan)
                || (self.stat_select & 0x40 != 0 && self.ly == self.lyc));
        if line && !self.stat_line {
            self.interrupt_flag |= INT_STAT;
        }
        self.stat_line = line;
    }

    /// The byte at `address`, as the CPU reads it now (blocked accesses give `$FF`)
    fn cpu_read(&mut self, address: u16) -> u8 {
        if self.dma_blocks(address) {
            self.dma_conflicts += 1;
            return 0xFF;
        }
        match address {
            0x8000..=0x9FFF if self.vram_blocked() => {
                self.blocked_vram_reads += 1;
                0xFF
            }
            0xFE00..=0xFE9F if self.oam_blocked() => {
                self.blocked_oam_reads += 1;
                0xFF
            }
            _ => self.peek(address),
        }
    }

    /// The byte at `address`, with nothing blocked and no side effect: what a test reads
    fn peek(&self, address: u16) -> u8 {
        match address {
            // Bank 0 (MBC1 banking mode 1, which maps another bank here, is not modelled)
            0x0000..=0x3FFF => self.rom_byte(usize::from(address)),
            0x4000..=0x7FFF => {
                self.rom_byte(self.effective_rom_bank() * 0x4000 + usize::from(address - 0x4000))
            }
            0x8000..=0x9FFF => self.vram[usize::from(address - 0x8000)],
            0xA000..=0xBFFF => {
                if self.cart_ram_enabled {
                    self.cart_ram[self.cart_ram_bank * 0x2000 + usize::from(address - 0xA000)]
                } else {
                    0xFF
                }
            }
            0xC000..=0xDFFF => self.wram[usize::from(address - 0xC000)],
            0xE000..=0xFDFF => self.wram[usize::from(address - 0xE000)],
            0xFE00..=0xFE9F => self.oam[usize::from(address - 0xFE00)],
            0xFEA0..=0xFEFF => 0x00,
            0xFF00..=0xFF7F => self.read_io(address),
            0xFF80..=0xFFFE => self.hram[usize::from(address - 0xFF80)],
            0xFFFF => self.ie,
        }
    }

    fn rom_byte(&self, index: usize) -> u8 {
        if self.rom.is_empty() {
            0xFF
        } else {
            self.rom[index % self.rom.len()]
        }
    }

    fn effective_rom_bank(&self) -> usize {
        match self.mbc {
            Mbc::None => 1,
            Mbc::Mbc1 => {
                let low = if self.rom_bank & 0x1F == 0 {
                    1
                } else {
                    self.rom_bank & 0x1F
                };
                (self.mbc1_upper << 5) | low
            }
            Mbc::Mbc5 => self.rom_bank,
        }
    }

    fn read_io(&self, address: u16) -> u8 {
        match address {
            0xFF00 => {
                let mut low = 0x0F;
                if self.p1_select & 0x10 == 0 {
                    low &= !self.buttons & 0x0F;
                }
                if self.p1_select & 0x20 == 0 {
                    low &= !(self.buttons >> 4) & 0x0F;
                }
                0xC0 | self.p1_select | low
            }
            0xFF02 => self.io[0x02] | 0x7E,
            0xFF04 => (self.div_counter >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8,
            0xFF0F => self.interrupt_flag | 0xE0,
            0xFF40 => self.lcdc,
            0xFF41 => {
                let coincidence = if self.ly == self.lyc { 0x04 } else { 0 };
                0x80 | self.stat_select | coincidence | self.mode().bits()
            }
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF46 => (self.dma_next_source >> 8) as u8,
            _ => self.io[usize::from(address - 0xFF00)],
        }
    }

    /// Write `value` at `address`, as the CPU does now (blocked accesses are dropped)
    fn cpu_write(&mut self, address: u16, value: u8) {
        if (0xFE00..=0xFE9F).contains(&address) {
            let dropped = self.oam_blocked();
            self.oam_writes.push(OamWrite {
                frame: self.cycles / FRAME_MCYCLES,
                cycle: self.cycles,
                pc: self.current_pc,
                address,
                value,
                ly: self.ly,
                mode: self.mode(),
                lcd_on: self.lcd_on(),
                dropped,
            });
            if dropped {
                return;
            }
        }
        if self.dma_blocks(address) {
            self.dma_conflicts += 1;
            return;
        }
        match address {
            0x0000..=0x7FFF => self.write_mbc(address, value),
            0x8000..=0x9FFF => {
                if self.vram_blocked() {
                    self.blocked_vram_writes += 1;
                } else {
                    self.vram[usize::from(address - 0x8000)] = value;
                }
            }
            0xA000..=0xBFFF => {
                if self.cart_ram_enabled {
                    self.cart_ram[self.cart_ram_bank * 0x2000 + usize::from(address - 0xA000)] =
                        value;
                }
            }
            0xC000..=0xDFFF => self.wram[usize::from(address - 0xC000)] = value,
            0xE000..=0xFDFF => self.wram[usize::from(address - 0xE000)] = value,
            0xFE00..=0xFE9F => self.oam[usize::from(address - 0xFE00)] = value,
            0xFEA0..=0xFEFF => {}
            0xFF00..=0xFF7F => self.write_io(address, value),
            0xFF80..=0xFFFE => self.hram[usize::from(address - 0xFF80)] = value,
            0xFFFF => self.ie = value,
        }
    }

    fn write_mbc(&mut self, address: u16, value: u8) {
        match (self.mbc, address) {
            (Mbc::None, _) => {}
            (_, 0x0000..=0x1FFF) => self.cart_ram_enabled = value & 0x0F == 0x0A,
            (Mbc::Mbc1, 0x2000..=0x3FFF) => self.rom_bank = usize::from(value & 0x1F),
            (Mbc::Mbc1, 0x4000..=0x5FFF) => self.mbc1_upper = usize::from(value & 0x03),
            (Mbc::Mbc1, _) => {} // banking mode: only mode 0 is modelled
            (Mbc::Mbc5, 0x2000..=0x2FFF) => {
                self.rom_bank = (self.rom_bank & 0x100) | usize::from(value);
            }
            (Mbc::Mbc5, 0x3000..=0x3FFF) => {
                self.rom_bank = (self.rom_bank & 0xFF) | (usize::from(value & 0x01) << 8);
            }
            (Mbc::Mbc5, 0x4000..=0x5FFF) => self.cart_ram_bank = usize::from(value & 0x03),
            (Mbc::Mbc5, _) => {}
        }
    }

    fn write_io(&mut self, address: u16, value: u8) {
        match address {
            0xFF00 => self.p1_select = value & 0x30,
            0xFF02 => {
                self.io[0x02] = value & 0x81;
                if value & 0x81 == 0x81 {
                    // A transfer on the internal clock, with nothing connected: done at once
                    self.serial_out.push(self.io[0x01]);
                    self.io[0x01] = 0xFF;
                    self.io[0x02] &= 0x7F;
                    self.interrupt_flag |= INT_SERIAL;
                }
            }
            0xFF04 => {
                let before = self.timer_input();
                self.div_counter = 0;
                if before {
                    self.increment_tima();
                }
            }
            0xFF05 => {
                // A write in the M-cycle after an overflow cancels the reload
                self.tima = value;
                self.tima_reload = false;
            }
            0xFF06 => self.tma = value,
            0xFF07 => {
                let before = self.timer_input();
                self.tac = value & 0x07;
                if before && !self.timer_input() {
                    self.increment_tima();
                }
            }
            0xFF0F => self.interrupt_flag = value & 0x1F,
            0xFF40 => {
                let was_on = self.lcd_on();
                self.lcdc = value;
                if was_on && !self.lcd_on() {
                    self.ly = 0;
                    self.line_cycle = 0;
                }
                if !was_on && self.lcd_on() {
                    self.first_line = true;
                }
                self.update_stat_line();
            }
            0xFF41 => {
                self.stat_select = value & 0x78;
                self.update_stat_line();
            }
            0xFF44 => {} // LY is read-only
            0xFF45 => {
                self.lyc = value;
                self.update_stat_line();
            }
            0xFF46 => {
                self.dma_next_source = u16::from(value) << 8;
                self.dma_delay = 2;
            }
            _ => self.io[usize::from(address - 0xFF00)] = value,
        }
    }

    fn press(&mut self, mask: u8) {
        if self.buttons & mask != mask {
            self.interrupt_flag |= INT_JOYPAD;
        }
        self.buttons |= mask;
    }
}

/// A Game Boy running a ROM
pub struct GameBoy {
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    sp: u16,
    pc: u16,
    ime: bool,
    /// `ei` was run: IME is set after the next instruction
    ime_pending: bool,
    /// The instruction running is the one right after `ei`: IME is already set (so a
    /// `di` here cancels it), but no interrupt was taken before it, and a `halt` here
    /// sees IME as 0
    after_ei: bool,
    halted: bool,
    /// The halt bug: the next opcode is read twice
    halt_bug: bool,
    bus: Bus,
    /// The ROM's symbols (from the `.sym` file), for [`GameBoy::symbol`]
    symbols: BTreeMap<String, u16>,
}

impl GameBoy {
    /// A Game Boy that starts `rom` at `$0100`, with the registers the DMG boot ROM leaves
    pub fn new(rom: Vec<u8>) -> GameBoy {
        GameBoy {
            a: 0x01,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false,
            ime_pending: false,
            after_ei: false,
            halted: false,
            halt_bug: false,
            bus: Bus::new(rom),
            symbols: BTreeMap::new(),
        }
    }

    /// The same, with the symbols of the ROM, by name (`Scope.local` for a local label)
    pub fn with_symbols(rom: Vec<u8>, symbols: BTreeMap<String, u16>) -> GameBoy {
        let mut gb = GameBoy::new(rom);
        gb.symbols = symbols;
        gb
    }

    // ----- What a test uses -----

    /// Run `frames` frames: 70224 dots each, whether the LCD is on or not
    pub fn run_frames(&mut self, frames: u64) {
        let end = self.bus.cycles + frames * FRAME_MCYCLES;
        self.run_until_cycle(end);
    }

    /// Run until `cycle` M-cycles since power-on (the instruction running then ends)
    pub fn run_until_cycle(&mut self, cycle: u64) {
        while self.bus.cycles < cycle {
            self.step();
        }
    }

    /// Run until the CPU is about to run the instruction at `address`, at most
    /// `max_frames` frames; whether it got there
    pub fn run_until_pc(&mut self, address: u16, max_frames: u64) -> bool {
        let end = self.bus.cycles + max_frames * FRAME_MCYCLES;
        while self.bus.cycles < end {
            if self.pc == address && !self.halted {
                return true;
            }
            self.step();
        }
        false
    }

    /// Run a script: each step holds `buttons` (and releases every other one) for
    /// `frames` frames; every button is released at the end
    pub fn run_script(&mut self, script: &[(u64, &[Button])]) {
        for (frames, buttons) in script {
            self.release_all();
            for button in *buttons {
                self.press(*button);
            }
            self.run_frames(*frames);
        }
        self.release_all();
    }

    pub fn press(&mut self, button: Button) {
        self.bus.press(button.mask());
    }

    pub fn release(&mut self, button: Button) {
        self.bus.buttons &= !button.mask();
    }

    pub fn release_all(&mut self) {
        self.bus.buttons = 0;
    }

    /// The byte at `address`, as the memory holds it (nothing blocked, no side effect)
    pub fn read(&self, address: u16) -> u8 {
        self.bus.peek(address)
    }

    /// The little-endian word at `address`
    pub fn read16(&self, address: u16) -> u16 {
        u16::from_le_bytes([self.read(address), self.read(address.wrapping_add(1))])
    }

    /// The address of `name` in the ROM's symbols; panics if there is none
    pub fn symbol(&self, name: &str) -> u16 {
        *self.symbols.get(name).unwrap_or_else(|| {
            panic!(
                "no symbol {:?}; the ROM defines: {:?}",
                name,
                self.symbols.keys().collect::<Vec<_>>()
            )
        })
    }

    /// The byte at symbol `name`
    pub fn read_symbol(&self, name: &str) -> u8 {
        self.read(self.symbol(name))
    }

    /// The 4 bytes of OAM entry `index`: Y, X, tile, attributes
    pub fn oam_entry(&self, index: usize) -> [u8; 4] {
        let base = index * 4;
        let oam = &self.bus.oam;
        [oam[base], oam[base + 1], oam[base + 2], oam[base + 3]]
    }

    pub fn oam(&self) -> &[u8; 0xA0] {
        &self.bus.oam
    }

    pub fn vram(&self) -> &[u8; 0x2000] {
        &self.bus.vram
    }

    pub fn registers(&self) -> Registers {
        Registers {
            a: self.a,
            f: self.f,
            b: self.b,
            c: self.c,
            d: self.d,
            e: self.e,
            h: self.h,
            l: self.l,
            sp: self.sp,
            pc: self.pc,
            ime: self.ime,
            halted: self.halted,
        }
    }

    /// M-cycles since power-on
    pub fn cycles(&self) -> u64 {
        self.bus.cycles
    }

    /// Frames since power-on
    pub fn frame(&self) -> u64 {
        self.bus.cycles / FRAME_MCYCLES
    }

    /// `LY` and the PPU mode now
    pub fn ly_mode(&self) -> (u8, Mode) {
        (self.bus.ly, self.bus.mode())
    }

    /// Every CPU write to OAM so far, with when it happened
    pub fn oam_writes(&self) -> &[OamWrite] {
        &self.bus.oam_writes
    }

    /// The CPU writes to OAM the hardware dropped (mode 2 or 3, or during a DMA)
    pub fn dropped_oam_writes(&self) -> Vec<&OamWrite> {
        self.bus.oam_writes.iter().filter(|w| w.dropped).collect()
    }

    /// CPU accesses to VRAM in mode 3 and reads of OAM in mode 2 or 3, which the hardware
    /// blocks: (VRAM writes, VRAM reads, OAM reads)
    pub fn blocked_accesses(&self) -> (u64, u64, u64) {
        (
            self.bus.blocked_vram_writes,
            self.bus.blocked_vram_reads,
            self.bus.blocked_oam_reads,
        )
    }

    /// CPU accesses outside HRAM and the I/O registers during an OAM DMA
    pub fn dma_conflicts(&self) -> u64 {
        self.bus.dma_conflicts
    }

    /// The bytes the program sent through the serial port
    pub fn serial_output(&self) -> &[u8] {
        &self.bus.serial_out
    }

    // ----- The CPU -----

    fn tick(&mut self) {
        self.bus.tick();
    }

    fn read8(&mut self, address: u16) -> u8 {
        self.bus.tick();
        self.bus.cpu_read(address)
    }

    fn write8(&mut self, address: u16, value: u8) {
        self.bus.tick();
        self.bus.cpu_write(address, value);
    }

    fn fetch8(&mut self) -> u8 {
        let value = self.read8(self.pc);
        if self.halt_bug {
            self.halt_bug = false;
        } else {
            self.pc = self.pc.wrapping_add(1);
        }
        value
    }

    fn fetch16(&mut self) -> u16 {
        let low = self.fetch8();
        let high = self.fetch8();
        u16::from_le_bytes([low, high])
    }

    fn push16(&mut self, value: u16) {
        let [high, low] = value.to_be_bytes();
        self.sp = self.sp.wrapping_sub(1);
        self.write8(self.sp, high);
        self.sp = self.sp.wrapping_sub(1);
        self.write8(self.sp, low);
    }

    fn pop16(&mut self) -> u16 {
        let low = self.read8(self.sp);
        self.sp = self.sp.wrapping_add(1);
        let high = self.read8(self.sp);
        self.sp = self.sp.wrapping_add(1);
        u16::from_le_bytes([low, high])
    }

    fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }
    fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }
    fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }
    fn set_bc(&mut self, value: u16) {
        [self.b, self.c] = value.to_be_bytes();
    }
    fn set_de(&mut self, value: u16) {
        [self.d, self.e] = value.to_be_bytes();
    }
    fn set_hl(&mut self, value: u16) {
        [self.h, self.l] = value.to_be_bytes();
    }

    /// The register pair of bits 4-5 of an opcode: `bc`, `de`, `hl`, `sp`
    fn rp(&self, index: u8) -> u16 {
        match index {
            0 => self.bc(),
            1 => self.de(),
            2 => self.hl(),
            _ => self.sp,
        }
    }

    fn set_rp(&mut self, index: u8, value: u16) {
        match index {
            0 => self.set_bc(value),
            1 => self.set_de(value),
            2 => self.set_hl(value),
            _ => self.sp = value,
        }
    }

    /// The 8-bit operand of an opcode's 3 bits: `b c d e h l [hl] a`
    fn r8(&mut self, index: u8) -> u8 {
        match index {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => self.read8(self.hl()),
            _ => self.a,
        }
    }

    fn set_r8(&mut self, index: u8, value: u8) {
        match index {
            0 => self.b = value,
            1 => self.c = value,
            2 => self.d = value,
            3 => self.e = value,
            4 => self.h = value,
            5 => self.l = value,
            6 => self.write8(self.hl(), value),
            _ => self.a = value,
        }
    }

    fn flag(&self, flag: u8) -> bool {
        self.f & flag != 0
    }

    fn set_flags(&mut self, z: bool, n: bool, h: bool, c: bool) {
        self.f = (if z { FLAG_Z } else { 0 })
            | (if n { FLAG_N } else { 0 })
            | (if h { FLAG_H } else { 0 })
            | (if c { FLAG_C } else { 0 });
    }

    /// The condition of bits 3-4 of an opcode: `nz z nc c`
    fn condition(&self, index: u8) -> bool {
        match index {
            0 => !self.flag(FLAG_Z),
            1 => self.flag(FLAG_Z),
            2 => !self.flag(FLAG_C),
            _ => self.flag(FLAG_C),
        }
    }

    /// The 8-bit ALU operation of bits 3-5 of an opcode on `a`
    fn alu(&mut self, operation: u8, value: u8) {
        let a = self.a;
        let carry = u8::from(self.flag(FLAG_C));
        match operation {
            0 | 1 => {
                // add, adc
                let carry = if operation == 1 { carry } else { 0 };
                let result = u16::from(a) + u16::from(value) + u16::from(carry);
                let half = (a & 0x0F) + (value & 0x0F) + carry > 0x0F;
                self.a = result as u8;
                self.set_flags(self.a == 0, false, half, result > 0xFF);
            }
            2 | 3 | 7 => {
                // sub, sbc, cp
                let carry = if operation == 3 { carry } else { 0 };
                let result = i16::from(a) - i16::from(value) - i16::from(carry);
                let half = i16::from(a & 0x0F) - i16::from(value & 0x0F) - i16::from(carry) < 0;
                let byte = result as u8;
                self.set_flags(byte == 0, true, half, result < 0);
                if operation != 7 {
                    self.a = byte;
                }
            }
            4 => {
                self.a &= value;
                self.set_flags(self.a == 0, false, true, false);
            }
            5 => {
                self.a ^= value;
                self.set_flags(self.a == 0, false, false, false);
            }
            _ => {
                self.a |= value;
                self.set_flags(self.a == 0, false, false, false);
            }
        }
    }

    fn inc8(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        let carry = self.flag(FLAG_C);
        self.set_flags(result == 0, false, value & 0x0F == 0x0F, carry);
        result
    }

    fn dec8(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        let carry = self.flag(FLAG_C);
        self.set_flags(result == 0, true, value & 0x0F == 0, carry);
        result
    }

    /// `sp` plus a signed byte, with the flags of `add sp, e` / `ld hl, sp + e`
    fn sp_plus_e(&mut self, e: u8) -> u16 {
        let sp = self.sp;
        let result = sp.wrapping_add(e as i8 as u16);
        let half = (sp & 0x0F) + (u16::from(e) & 0x0F) > 0x0F;
        let carry = (sp & 0xFF) + u16::from(e) > 0xFF;
        self.set_flags(false, false, half, carry);
        result
    }

    /// The highest-priority interrupt that is requested and enabled
    fn pending_interrupt(&self) -> Option<u8> {
        let pending = self.bus.ie & self.bus.interrupt_flag & 0x1F;
        (pending != 0).then(|| pending.trailing_zeros() as u8)
    }

    /// Run one instruction, or one M-cycle of `halt`, or the dispatch of an interrupt
    pub fn step(&mut self) {
        let mut woke = false;
        if self.halted {
            if self.pending_interrupt().is_some() {
                self.halted = false;
                woke = true;
            } else {
                self.tick();
                return;
            }
        }
        if self.ime {
            if let Some(bit) = self.pending_interrupt() {
                if woke {
                    // Leaving `halt` for an interrupt takes one more M-cycle
                    self.tick();
                }
                self.ime = false;
                self.tick();
                self.tick();
                // `ei` then `halt` with an interrupt pending: the halt bug left `pc` on the
                // byte after the `halt` without moving past it, so the handler returns to
                // the `halt`, which runs again
                let pc = if self.halt_bug {
                    self.halt_bug = false;
                    self.pc.wrapping_sub(1)
                } else {
                    self.pc
                };
                self.push16(pc);
                self.bus.interrupt_flag &= !(1 << bit);
                self.pc = 0x0040 + 8 * u16::from(bit);
                self.tick();
                return;
            }
        }
        // `ei` takes effect here, after the check above: no interrupt is taken before
        // the instruction that follows it, and a `di` in that instruction cancels it
        self.after_ei = self.ime_pending;
        if self.ime_pending {
            self.ime_pending = false;
            self.ime = true;
        }
        self.bus.current_pc = self.pc;
        let opcode = self.fetch8();
        self.execute(opcode);
        self.after_ei = false;
    }

    fn execute(&mut self, opcode: u8) {
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;
        let p = y >> 1;
        match opcode {
            0x00 => {}
            0x10 => {
                // stop: a 2-byte nop here
                self.fetch8();
            }
            0x76 => {
                // halt. With an interrupt pending it does not stop: with IME set it is taken
                // at once; with IME clear, or right after `ei`, the halt bug (the next
                // opcode is read twice; after `ei` the interrupt is taken and returns to
                // the `halt`, see `step`)
                if self.pending_interrupt().is_some() {
                    if !self.ime || self.after_ei {
                        self.halt_bug = true;
                    }
                } else {
                    self.halted = true;
                }
            }
            0x40..=0x7F => {
                let value = self.r8(z);
                self.set_r8(y, value);
            }
            0x80..=0xBF => {
                let value = self.r8(z);
                self.alu(y, value);
            }
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
                let value = self.fetch8();
                self.alu(y, value);
            }
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                let value = self.fetch8();
                self.set_r8(y, value);
            }
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                let value = self.r8(y);
                let result = self.inc8(value);
                self.set_r8(y, result);
            }
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                let value = self.r8(y);
                let result = self.dec8(value);
                self.set_r8(y, result);
            }
            0x01 | 0x11 | 0x21 | 0x31 => {
                let value = self.fetch16();
                self.set_rp(p, value);
            }
            0x03 | 0x13 | 0x23 | 0x33 => {
                let value = self.rp(p).wrapping_add(1);
                self.set_rp(p, value);
                self.tick();
            }
            0x0B | 0x1B | 0x2B | 0x3B => {
                let value = self.rp(p).wrapping_sub(1);
                self.set_rp(p, value);
                self.tick();
            }
            0x09 | 0x19 | 0x29 | 0x39 => {
                let hl = self.hl();
                let value = self.rp(p);
                let (result, carry) = hl.overflowing_add(value);
                let half = (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF;
                let z = self.flag(FLAG_Z);
                self.set_flags(z, false, half, carry);
                self.set_hl(result);
                self.tick();
            }
            0x02 => self.write8(self.bc(), self.a),
            0x12 => self.write8(self.de(), self.a),
            0x22 => {
                let hl = self.hl();
                self.write8(hl, self.a);
                self.set_hl(hl.wrapping_add(1));
            }
            0x32 => {
                let hl = self.hl();
                self.write8(hl, self.a);
                self.set_hl(hl.wrapping_sub(1));
            }
            0x0A => self.a = self.read8(self.bc()),
            0x1A => self.a = self.read8(self.de()),
            0x2A => {
                let hl = self.hl();
                self.a = self.read8(hl);
                self.set_hl(hl.wrapping_add(1));
            }
            0x3A => {
                let hl = self.hl();
                self.a = self.read8(hl);
                self.set_hl(hl.wrapping_sub(1));
            }
            0x07 => {
                // rlca
                let carry = self.a & 0x80 != 0;
                self.a = self.a.rotate_left(1);
                self.set_flags(false, false, false, carry);
            }
            0x0F => {
                // rrca
                let carry = self.a & 0x01 != 0;
                self.a = self.a.rotate_right(1);
                self.set_flags(false, false, false, carry);
            }
            0x17 => {
                // rla
                let carry = self.a & 0x80 != 0;
                self.a = (self.a << 1) | u8::from(self.flag(FLAG_C));
                self.set_flags(false, false, false, carry);
            }
            0x1F => {
                // rra
                let carry = self.a & 0x01 != 0;
                self.a = (self.a >> 1) | (u8::from(self.flag(FLAG_C)) << 7);
                self.set_flags(false, false, false, carry);
            }
            0x27 => {
                // daa
                let mut a = self.a;
                let mut carry = self.flag(FLAG_C);
                if self.flag(FLAG_N) {
                    if carry {
                        a = a.wrapping_sub(0x60);
                    }
                    if self.flag(FLAG_H) {
                        a = a.wrapping_sub(0x06);
                    }
                } else {
                    if carry || a > 0x99 {
                        a = a.wrapping_add(0x60);
                        carry = true;
                    }
                    if self.flag(FLAG_H) || a & 0x0F > 0x09 {
                        a = a.wrapping_add(0x06);
                    }
                }
                self.a = a;
                let n = self.flag(FLAG_N);
                self.set_flags(a == 0, n, false, carry);
            }
            0x2F => {
                // cpl
                self.a = !self.a;
                self.f |= FLAG_N | FLAG_H;
            }
            0x37 => {
                // scf
                self.f = (self.f & FLAG_Z) | FLAG_C;
            }
            0x3F => {
                // ccf
                self.f = (self.f & (FLAG_Z | FLAG_C)) ^ FLAG_C;
            }
            0x08 => {
                // ld [n16], sp
                let address = self.fetch16();
                let [high, low] = self.sp.to_be_bytes();
                self.write8(address, low);
                self.write8(address.wrapping_add(1), high);
            }
            0x18 => {
                let offset = self.fetch8() as i8;
                self.pc = self.pc.wrapping_add(offset as u16);
                self.tick();
            }
            0x20 | 0x28 | 0x30 | 0x38 => {
                let offset = self.fetch8() as i8;
                if self.condition(y - 4) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    self.tick();
                }
            }
            0xC3 => {
                self.pc = self.fetch16();
                self.tick();
            }
            0xC2 | 0xCA | 0xD2 | 0xDA => {
                let address = self.fetch16();
                if self.condition(y) {
                    self.pc = address;
                    self.tick();
                }
            }
            0xE9 => self.pc = self.hl(),
            0xCD => {
                let address = self.fetch16();
                self.tick();
                let pc = self.pc;
                self.push16(pc);
                self.pc = address;
            }
            0xC4 | 0xCC | 0xD4 | 0xDC => {
                let address = self.fetch16();
                if self.condition(y) {
                    self.tick();
                    let pc = self.pc;
                    self.push16(pc);
                    self.pc = address;
                }
            }
            0xC9 => {
                self.pc = self.pop16();
                self.tick();
            }
            0xD9 => {
                // reti
                self.pc = self.pop16();
                self.tick();
                self.ime = true;
            }
            0xC0 | 0xC8 | 0xD0 | 0xD8 => {
                self.tick();
                if self.condition(y) {
                    self.pc = self.pop16();
                    self.tick();
                }
            }
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                self.tick();
                let pc = self.pc;
                self.push16(pc);
                self.pc = u16::from(y) * 8;
            }
            0xC1 | 0xD1 | 0xE1 => {
                let value = self.pop16();
                self.set_rp(p, value);
            }
            0xF1 => {
                let [a, f] = self.pop16().to_be_bytes();
                self.a = a;
                self.f = f & 0xF0;
            }
            0xC5 | 0xD5 | 0xE5 => {
                self.tick();
                let value = self.rp(p);
                self.push16(value);
            }
            0xF5 => {
                self.tick();
                let value = u16::from_be_bytes([self.a, self.f]);
                self.push16(value);
            }
            0xE0 => {
                let offset = self.fetch8();
                self.write8(0xFF00 | u16::from(offset), self.a);
            }
            0xF0 => {
                let offset = self.fetch8();
                self.a = self.read8(0xFF00 | u16::from(offset));
            }
            0xE2 => self.write8(0xFF00 | u16::from(self.c), self.a),
            0xF2 => self.a = self.read8(0xFF00 | u16::from(self.c)),
            0xEA => {
                let address = self.fetch16();
                self.write8(address, self.a);
            }
            0xFA => {
                let address = self.fetch16();
                self.a = self.read8(address);
            }
            0xE8 => {
                let e = self.fetch8();
                self.sp = self.sp_plus_e(e);
                self.tick();
                self.tick();
            }
            0xF8 => {
                let e = self.fetch8();
                let value = self.sp_plus_e(e);
                self.set_hl(value);
                self.tick();
            }
            0xF9 => {
                self.sp = self.hl();
                self.tick();
            }
            0xF3 => {
                self.ime = false;
                self.ime_pending = false;
            }
            0xFB => self.ime_pending = true,
            0xCB => {
                let opcode = self.fetch8();
                self.execute_cb(opcode);
            }
            _ => panic!(
                "illegal opcode ${:02X} at ${:04X} (the CPU would lock up)",
                opcode, self.bus.current_pc
            ),
        }
    }

    fn execute_cb(&mut self, opcode: u8) {
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;
        let value = self.r8(z);
        match opcode >> 6 {
            0 => {
                let carry_in = u8::from(self.flag(FLAG_C));
                let (result, carry) = match y {
                    0 => (value.rotate_left(1), value & 0x80 != 0),
                    1 => (value.rotate_right(1), value & 0x01 != 0),
                    2 => ((value << 1) | carry_in, value & 0x80 != 0),
                    3 => ((value >> 1) | (carry_in << 7), value & 0x01 != 0),
                    4 => (value << 1, value & 0x80 != 0),
                    5 => ((value >> 1) | (value & 0x80), value & 0x01 != 0),
                    6 => (value.rotate_left(4), false),
                    _ => (value >> 1, value & 0x01 != 0),
                };
                self.set_flags(result == 0, false, false, carry);
                self.set_r8(z, result);
            }
            1 => {
                // bit
                let carry = self.flag(FLAG_C);
                self.set_flags(value & (1 << y) == 0, false, true, carry);
            }
            2 => self.set_r8(z, value & !(1 << y)),
            _ => self.set_r8(z, value | (1 << y)),
        }
    }
}
