//! A bounded FIFO of VRAM byte writes, produced by game code and consumed in VBlank.
//!
//! [`enqueue`] takes `a` (value) and `hl` (VRAM address), returning carry clear on success
//! or set if full or outside VRAM. There is one producer: initialization or main-loop
//! code, **never an interrupt handler**. [`flush`] runs with interrupts off, normally in
//! the VBlank handler. Neither routine changes IME. The producer publishes its write
//! cursor only after writing an entire entry, so a VBlank interrupt at any instruction
//! observes only complete entries. The consumer never resets the producer's cursor.
//!
//! [`CAPACITY`] pending writes fit in [`BUFFER_BYTES`] bytes plus two cursors. An extra
//! ring slot distinguishes empty from full. The consumer writes only on lines 144–152,
//! leaving one line of headroom for its last write; a late handler defers the remainder.
//! Arbitrary user handler work can still exceed the VBlank budget.
//! In `RustBoy`, use its queue helpers to install the consumer and reserve storage;
//! registering these routines alone does not enable the engine's queue integration.

use crate::asm::{Block, Condition, Instr, Mem, R8, R16, R16Stack, Section};
use crate::hw;
use crate::stdlib::routine::{Regs, Routine};

/// Maximum number of pending byte writes. Overflow rejects the new write (carry set).
pub const CAPACITY: u8 = 12;
/// WRAM storage for the ring, including the extra slot that distinguishes full/empty.
pub const BUFFER_BYTES: u16 = (CAPACITY as u16 + 1) * 3;
/// Label of the ring storage, containing address low, address high, then value per slot.
pub const BUFFER: &str = "wVramQueue";
/// Consumer cursor: byte offset of the next entry to flush.
pub const READ: &str = "wVramQueueRead";
/// Producer cursor: byte offset of the next free entry.
pub const WRITE: &str = "wVramQueueWrite";

/// Clear the cursors at startup, before enabling interrupts. Does not clear the unused
/// entry bytes. Changes `a` and the flags.
pub fn initialize() -> Vec<Instr> {
    let mut code = Block::new();
    code.xor(R8::A).ld_addr_def_a(READ).ld_addr_def_a(WRITE);
    code.into_instrs()
}

/// Reserve the ring and its cursors for a standalone `stdlib` program. The engine
/// allocates equivalent buffers at fixed addresses through its memory allocator.
pub fn section() -> Vec<Instr> {
    let mut code = Block::new();
    code.section(Section::wram0("VRAM queue"))
        .label(BUFFER)
        .ds(&BUFFER_BYTES.to_string())
        .label(READ)
        .ds("1")
        .label(WRITE)
        .ds("1");
    code.into_instrs()
}

/// Queue one byte: `hl` = address, `a` = value. Returns carry clear if accepted, set if
/// full or if the address is outside VRAM. Changes all registers and flags. Neither
/// VRAM nor the cursors change on rejection. Single producer only (see module docs).
pub fn enqueue() -> Routine {
    let mut code = Block::new();
    code.label("QueueVramWrite")
        .ld(R8::D, R8::A)
        .ld(R8::A, R8::H)
        .cp((hw::VRAM.value >> 8) as u8)
        .ret_cond(Condition::C)
        .cp((hw::VRAM_END >> 8) as u8)
        .jr_cond(Condition::NC, ".reject")
        .ld(R8::B, R8::H)
        .ld(R8::C, R8::L)
        .ld_a_addr_def(WRITE)
        .ld(R8::E, R8::A)
        .add(3)
        .cp(BUFFER_BYTES as u8)
        .jr_cond(Condition::C, ".next")
        .xor(R8::A)
        .label(".next")
        .ld(R8::H, R8::A)
        .ld_a_addr_def(READ)
        .cp(R8::H)
        .jr_cond(Condition::Z, ".reject")
        .ld(R8::L, R8::E)
        .ld(R8::E, R8::H)
        .ld(R8::H, 0)
        .push(R16Stack::BC)
        .ld(R16::BC, BUFFER)
        .add_hl(R16::BC)
        .pop(R16Stack::BC)
        .ld(R8::A, R8::C)
        .ld(Mem::Hli, R8::A)
        .ld(R8::A, R8::B)
        .ld(Mem::Hli, R8::A)
        .ld(R8::AtHl, R8::D)
        // Commit last: the consumer sees none or all of this entry.
        .label(".publish")
        .ld(R8::A, R8::E)
        .ld_addr_def_a(WRITE)
        .and(R8::A)
        .ret()
        .label(".reject")
        .scf()
        .ret();
    Routine::new("QueueVramWrite", code)
        .with_reads(Regs::A | Regs::HL)
        .with_returns(Regs::F)
        .with_clobbers(Regs::A | Regs::BC | Regs::DE | Regs::HL)
}

/// Flush committed entries in FIFO order, with interrupts off. All registers and flags
/// may change. Stop before consuming an entry if outside lines 144–152, so a delayed
/// VBlank handler defers work instead of losing a VRAM write. The final line of VBlank
/// is reserved as timing headroom, not used to begin another entry.
pub fn flush() -> Routine {
    let mut code = Block::new();
    code.label("FlushVramQueue")
        .label(".next")
        .ld_a_addr_def(hw::LY)
        .cp(hw::SCRN_Y.value)
        .ret_cond(Condition::C)
        .cp(hw::SCRN_Y.value + hw::VBLANK_LINES as u8 - 1)
        .ret_cond(Condition::NC)
        .ld_a_addr_def(READ)
        .ld(R8::E, R8::A)
        .ld_a_addr_def(WRITE)
        .cp(R8::E)
        .ret_cond(Condition::Z)
        .ld(R8::L, R8::E)
        .ld(R8::H, 0)
        .ld(R16::BC, BUFFER)
        .add_hl(R16::BC)
        .ld(R8::A, Mem::Hli)
        .ld(R8::E, R8::A)
        .ld(R8::A, Mem::Hli)
        .ld(R8::D, R8::A)
        .ld(R8::A, R8::AtHl)
        .label(".write")
        .ld(Mem::De, R8::A)
        .ld_a_addr_def(READ)
        .add(3)
        .cp(BUFFER_BYTES as u8)
        .jr_cond(Condition::C, ".advance")
        .xor(R8::A)
        .label(".advance")
        .ld_addr_def_a(READ)
        .jr(".next");
    Routine::new("FlushVramQueue", code).with_clobbers(Regs::ALL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::Expr;
    use crate::asm::test_cpu::TestCpu;

    fn write(cpu: &mut TestCpu, address: u16, value: u8) -> bool {
        let mut inputs = Block::new();
        inputs.ld(R16::HL, Expr::hex(address)).ld(R8::A, value);
        cpu.run(&inputs.into_instrs());
        cpu.run(enqueue().body());
        !cpu.carry
    }

    #[test]
    fn fifo_full_wrap_and_rejection() {
        let mut cpu = TestCpu::default();
        cpu.run(&initialize());
        cpu.mem.insert(hw::LY.name.to_string(), hw::SCRN_Y.value);
        for batch in 0..30u8 {
            for i in 0..CAPACITY {
                assert!(write(&mut cpu, hw::SCRN0.value + u16::from(i), batch + i));
            }
            let cursors = (cpu.mem[READ], cpu.mem[WRITE]);
            assert!(!write(&mut cpu, hw::SCRN1.value, 255));
            for invalid in [0, hw::VRAM.value - 1, hw::VRAM_END, u16::MAX] {
                assert!(!write(&mut cpu, invalid, 255));
            }
            assert_eq!((cpu.mem[READ], cpu.mem[WRITE]), cursors);
            cpu.run(flush().body());
            assert_eq!(cpu.mem[READ], cpu.mem[WRITE]);
            for i in 0..CAPACITY {
                assert_eq!(
                    cpu.mem[&format!("${:04X}", hw::SCRN0.value + u16::from(i))],
                    batch + i
                );
            }
            assert!(cpu.mem.get(&format!("${:04X}", hw::SCRN1.value)).is_none());
        }
        // Repeated writes to one address remain in insertion order: the last wins.
        assert!(write(&mut cpu, hw::SCRN1.value, 7));
        assert!(write(&mut cpu, hw::SCRN1.value, 9));
        cpu.run(flush().body());
        assert_eq!(cpu.mem[&format!("${:04X}", hw::SCRN1.value)], 9);
        for address in [hw::VRAM.value, hw::VRAM_END - 1] {
            assert!(write(&mut cpu, address, 99));
        }
        cpu.run(flush().body());
        for address in [hw::VRAM.value, hw::VRAM_END - 1] {
            assert_eq!(cpu.mem[&format!("${address:04X}")], 99);
        }
    }

    #[test]
    fn flush_defers_outside_its_safe_window() {
        let mut cpu = TestCpu::default();
        cpu.run(&initialize());
        assert!(write(&mut cpu, hw::SCRN0.value, 42));
        for ly in [0, 52, 143, 153] {
            cpu.mem.insert(hw::LY.name.to_string(), ly);
            cpu.run(flush().body());
            assert_ne!(cpu.mem[READ], cpu.mem[WRITE]);
            assert!(cpu.mem.get(&format!("${:04X}", hw::SCRN0.value)).is_none());
        }
        cpu.mem.insert(hw::LY.name.to_string(), 152);
        cpu.run(flush().body());
        assert_eq!(cpu.mem[READ], cpu.mem[WRITE]);
        assert_eq!(cpu.mem[&format!("${:04X}", hw::SCRN0.value)], 42);
    }
}
