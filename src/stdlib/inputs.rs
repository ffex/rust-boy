use crate::asm::{Block, Condition, Expr, Instr, JumpTarget, LabelAllocator, Mem, R8};
use crate::hw;
use crate::stdlib::routine::{Regs, Routine};

/// Enum for joypad buttons that can return constant names and values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadButton {
    Down,
    Up,
    Left,
    Right,
    Start,
    Select,
    B,
    A,
}

impl PadButton {
    /// The `hardware.inc` flag of this button in `wCurKeys` / `wNewKeys` (`hw::PADF_DOWN`, …)
    pub fn flag(self) -> hw::Symbol<u8> {
        match self {
            PadButton::Down => hw::PADF_DOWN,
            PadButton::Up => hw::PADF_UP,
            PadButton::Left => hw::PADF_LEFT,
            PadButton::Right => hw::PADF_RIGHT,
            PadButton::Start => hw::PADF_START,
            PadButton::Select => hw::PADF_SELECT,
            PadButton::B => hw::PADF_B,
            PadButton::A => hw::PADF_A,
        }
    }
    /// The name of its flag constant (e.g., "PADF_DOWN"): `self.flag().name`
    pub fn name(self) -> &'static str {
        self.flag().name
    }
    /// Stem of the local labels [`check_key`] emits for this button, which a
    /// [`LabelAllocator`] numbers: "check_left" gives `.check_left_3`
    pub fn label(self) -> &'static str {
        match self {
            PadButton::Down => "check_down",
            PadButton::Up => "check_up",
            PadButton::Left => "check_left",
            PadButton::Right => "check_right",
            PadButton::Start => "check_start",
            PadButton::Select => "check_select",
            PadButton::B => "check_b",
            PadButton::A => "check_a",
        }
    }
}

/// Polls the Game Boy controller and updates key state variables.
/// This function reads both button and D-pad inputs, combines them,
/// and tracks which keys are currently pressed, newly pressed, and newly released.
///
/// @requires wCurKeys: 1 byte variable to store currently pressed keys
/// @requires wNewKeys: 1 byte variable to store newly pressed keys
/// @requires wReleasedKeys: 1 byte variable to store newly released keys
/// @requires P1F_GET_BTN, P1F_GET_DPAD, P1F_GET_NONE constants
/// @requires rP1: Joypad register
///
/// # Key States
/// - wCurKeys: Bitmap of currently pressed keys (1 = pressed, 0 = not pressed)
/// - wNewKeys: Bitmap of keys that just transitioned to pressed this frame
/// - wReleasedKeys: Bitmap of keys that just transitioned to released this frame
///
/// # Calling convention
/// - Reads and returns nothing in registers: the keys go to `wCurKeys`, `wNewKeys`, and
///   `wReleasedKeys`, its variables ([`Routine::variables`]).
/// - Clobbers: `a`, `b`, `c` and the flags.
pub fn update_keys() -> Routine {
    let mut asm = Block::new();

    asm.label("UpdateKeys");
    asm.ld(R8::A, hw::P1F_GET_BTN);
    asm.call(".onenibble");
    asm.ld(R8::B, R8::A);

    asm.ld(R8::A, hw::P1F_GET_DPAD);
    asm.call(".onenibble");
    asm.swap(R8::A);
    asm.xor(R8::B);
    asm.ld(R8::B, R8::A);

    asm.ld(R8::A, hw::P1F_GET_NONE);
    asm.ldh(Mem::addr(hw::P1), R8::A);

    asm.ld_a_addr_def("wCurKeys");
    asm.ld(R8::C, R8::A);
    asm.xor(R8::B);
    asm.and(R8::B);
    asm.ld_addr_def_a("wNewKeys");
    asm.ld(R8::A, R8::B);
    asm.cpl();
    asm.and(R8::C);
    asm.ld_addr_def_a("wReleasedKeys");
    asm.ld(R8::A, R8::B);
    asm.ld_addr_def_a("wCurKeys");
    asm.ret();

    asm.label(".onenibble");
    asm.ldh(Mem::addr(hw::P1), R8::A);
    asm.call(".knowret");
    asm.ldh(R8::A, Mem::addr(hw::P1));
    asm.ldh(R8::A, Mem::addr(hw::P1));
    asm.ldh(R8::A, Mem::addr(hw::P1));
    asm.or(0xF0);

    asm.label(".knowret");
    asm.ret();

    Routine::new("UpdateKeys", asm)
        .with_variable("wCurKeys")
        .with_variable("wNewKeys")
        .with_variable("wReleasedKeys")
        .with_clobbers(Regs::A | Regs::B | Regs::C | Regs::F)
}

/// Run `action` when `button` is set in the named key-state bitmap.
pub(crate) fn check_key_state(
    labels: &LabelAllocator,
    button: PadButton,
    state: &'static str,
    action: Vec<Instr>,
) -> Vec<Instr> {
    let stem = button.label().to_owned();
    let end_stem = format!("{stem}_end");
    let [start, end] = labels.locals([stem.as_str(), end_stem.as_str()]);
    let mut asm = Block::new();
    asm.label(&start);
    asm.ld_a_addr_def(state);
    asm.and(button.flag());
    asm.jp_cond(Condition::Z, &end);
    asm.emit_all(action);
    asm.label(&end);
    asm.into_instrs()
}
/// Run `pressed_func` while `button` is held (its bit is set in `wCurKeys`)
///
/// The labels are local, with one number from `labels` (`.check_left_3`, `.check_left_end_3`),
/// so the same check can be emitted any number of times, and inside an `If` body. Like
/// any code with local labels, it must come after a global label, and `pressed_func`
/// must not define a global label (it would start a new label scope). Uses A and the flags.
//TODO check if it is ok, or we have to implement a big scope "check all keys"
// and one is pressed we jump at the end of the block
pub fn check_key(
    labels: &LabelAllocator,
    button: PadButton,
    pressed_func: Vec<Instr>,
) -> Vec<Instr> {
    let end_stem = format!("{}_end", button.label());
    let [start, end] = labels.locals([button.label(), end_stem.as_str()]);
    let mut asm = Block::new();
    asm.label(&start);
    asm.ld_a_addr_def("wCurKeys");
    asm.and(button.flag());
    asm.jp_cond(Condition::Z, &end);
    asm.emit_all(pressed_func);
    asm.label(&end);
    asm.into_instrs()
}

/// The variable of [`check_soft_reset`]: 0 until it sees the four reset buttons not all held
pub const SOFT_RESET_ARMED: &str = "wSoftResetArmed";

/// The four buttons of a soft reset, A + B + Select + Start, as `wCurKeys` bits
pub fn soft_reset_keys() -> Expr {
    Expr::from(hw::PADF_A) | hw::PADF_B | hw::PADF_SELECT | hw::PADF_START
}

/// The `CheckSoftReset` routine: reset the game when A, B, Select and Start are held
/// together
///
/// "Reset" means what the console does after its boot ROM: a jump to the cartridge's entry
/// point, `$0100` ([`hw::ROM_HEADER`]), so the program's start-up code runs again. It must
/// therefore start like a reset needs: interrupts off (`di`) and the stack pointer set
/// (`ld sp, $FFFE`), as `RustBoy`'s does; then it sets everything again (VRAM, OAM, every
/// variable to its initial value). What it does not set keeps its value: the registers the
/// start-up code does not write (`rSCX`, `rSCY`, ...), and the RAM no variable holds.
///
/// It reads `wCurKeys`, so it must run after [`update_keys`] in the same frame. It resets
/// once per press: the reset needs the four buttons not all held in an earlier frame
/// ([`SOFT_RESET_ARMED`], which the start-up code sets to 0), so holding them through the
/// reset does not reset the game again on every frame; it resets again once one of them
/// has been released and the four are held again.
///
/// - Reads and returns nothing; clobbers `a` and the flags (when it returns).
/// - Variables: `wCurKeys` (read), [`SOFT_RESET_ARMED`].
pub fn check_soft_reset() -> Routine {
    let mut asm = Block::new();
    asm.comment("Reset when A, B, Select and Start are held, after they were not all held");
    asm.label("CheckSoftReset");
    asm.ld_a_addr_def("wCurKeys");
    asm.and(soft_reset_keys());
    asm.cp(soft_reset_keys());
    asm.jr_cond(Condition::Z, ".held");
    // Not all four: the next time they are, reset
    asm.ld_a(1);
    asm.ld_addr_def_a(SOFT_RESET_ARMED);
    asm.ret();
    asm.label(".held");
    asm.ld_a_addr_def(SOFT_RESET_ARMED);
    asm.and(R8::A);
    // Held since the last reset (or power-on): wait for a release
    asm.ret_cond(Condition::Z);
    asm.emit(Instr::Jp {
        target: JumpTarget::Addr(hw::ROM_HEADER),
    });
    Routine::new("CheckSoftReset", asm)
        .with_variable("wCurKeys")
        .with_variable(SOFT_RESET_ARMED)
        .with_clobbers(Regs::A | Regs::F)
}

/// Code that selects both groups of the joypad in `rP1` (buttons and directions: their
/// select bits cleared, `P1F_GET_BTN & P1F_GET_DPAD`), so that a press requests the joypad
/// interrupt. [`update_keys`] leaves no group selected (`P1F_GET_NONE`). Uses `a`.
pub fn select_all_keys() -> Vec<Instr> {
    let mut asm = Block::new();
    asm.ld(R8::A, Expr::from(hw::P1F_GET_BTN) & hw::P1F_GET_DPAD);
    asm.ldh(Mem::addr(hw::P1), R8::A);
    asm.into_instrs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::label_check::assert_code_labels_ok;
    use crate::asm::test_cpu::TestCpu;
    use crate::stdlib::flow::{Emittable, If};

    /// Code that sets `var` to 1, to see which bodies ran
    fn mark(var: &str) -> Vec<Instr> {
        let mut asm = Block::new();
        asm.ld_a(1).ld_addr_def_a(var);
        asm.into_instrs()
    }

    /// Runs `code` with `keys` held; returns which of `vars` were marked
    fn run(code: &[Instr], keys: u8, vars: &[&str]) -> Vec<bool> {
        let mut cpu = TestCpu::default();
        cpu.mem.insert("wCurKeys".to_string(), keys);
        for var in vars {
            cpu.mem.insert(var.to_string(), 0);
        }
        // The key flags, with their `hardware.inc` values (Left is $20, A is $01)
        for flag in [hw::PADF_LEFT, hw::PADF_A] {
            cpu.consts.insert(flag.name.to_string(), flag.value);
        }
        cpu.run(code);
        vars.iter().map(|var| cpu.mem[*var] == 1).collect()
    }

    #[test]
    fn test_check_key_twice_on_one_button() {
        // B7: both checks emitted the global labels CheckLeft and CheckLeftEnd
        let labels = LabelAllocator::new();
        let code = [
            check_key(&labels, PadButton::Left, mark("wFirst")),
            check_key(&labels, PadButton::Left, mark("wSecond")),
            check_key(&labels, PadButton::A, mark("wThird")),
        ]
        .concat();
        assert_code_labels_ok(&code);

        let vars = ["wFirst", "wSecond", "wThird"];
        assert_eq!(run(&code, 0x00, &vars), [false, false, false]);
        assert_eq!(run(&code, 0x20, &vars), [true, true, false]);
        assert_eq!(run(&code, 0x01, &vars), [false, false, true]);
        assert_eq!(run(&code, 0x21, &vars), [true, true, true]);
    }

    #[test]
    fn test_check_key_inside_an_if() {
        // B7: the global label CheckLeft inside the If body started a new label scope,
        // so the If's jump to .end_if_0 could not be resolved
        let labels = LabelAllocator::new();
        let load_keys = || {
            let mut asm = Block::new();
            asm.ld_a_addr_def("wCurKeys");
            asm.into_instrs()
        };
        let body = check_key(&labels, PadButton::Left, mark("wLeft"));
        let code = If::ne(load_keys(), load_keys(), body)
            .or_else(check_key(&labels, PadButton::Left, mark("wElse")))
            .emit(&labels);
        assert_code_labels_ok(&code);

        // The keys always equal themselves: the else branch runs, and checks Left
        let vars = ["wLeft", "wElse"];
        assert_eq!(run(&code, 0x20, &vars), [false, true]);
        assert_eq!(run(&code, 0x01, &vars), [false, false]);
    }
}
