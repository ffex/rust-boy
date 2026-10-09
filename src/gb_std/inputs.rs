use crate::gb_asm::{Block, Condition, Instr, LabelAllocator, Mem, R8};
use crate::hw;

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
    /// Returns the flag constant name (e.g., "PADF_DOWN")
    pub fn name(self) -> &'static str {
        match self {
            PadButton::Down => "PADF_DOWN",
            PadButton::Up => "PADF_UP",
            PadButton::Left => "PADF_LEFT",
            PadButton::Right => "PADF_RIGHT",
            PadButton::Start => "PADF_START",
            PadButton::Select => "PADF_SELECT",
            PadButton::B => "PADF_B",
            PadButton::A => "PADF_A",
        }
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
/// and tracks which keys are currently pressed and newly pressed.
///
/// @requires wCurKeys: 1 byte variable to store currently pressed keys
/// @requires wNewKeys: 1 byte variable to store newly pressed keys
/// @requires P1F_GET_BTN, P1F_GET_DPAD, P1F_GET_NONE constants
/// @requires rP1: Joypad register
///
/// # Key States
/// - wCurKeys: Bitmap of currently pressed keys (1 = pressed, 0 = not pressed)
/// - wNewKeys: Bitmap of keys that just transitioned to pressed this frame
pub fn update_keys() -> Vec<Instr> {
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
    asm.xor(R8::B);
    asm.and(R8::B);
    asm.ld_addr_def_a("wNewKeys");
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
    asm.and(button.name());
    asm.jp_cond(Condition::Z, &end);
    asm.emit_all(pressed_func);
    asm.label(&end);
    asm.into_instrs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::label_check::assert_code_labels_ok;
    use crate::gb_asm::test_cpu::TestCpu;
    use crate::gb_std::flow::{Emittable, If};

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
        cpu.consts.insert("PADF_LEFT".to_string(), 0x20);
        cpu.consts.insert("PADF_A".to_string(), 0x01);
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
