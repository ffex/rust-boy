use crate::gb_asm::{Asm, Condition, Instr, Operand, Register};

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
    pub fn label(self) -> &'static str {
        match self {
            PadButton::Down => "CheckDown",
            PadButton::Up => "CheckUp",
            PadButton::Left => "CheckLeft",
            PadButton::Right => "CheckRight",
            PadButton::Start => "CheckStart",
            PadButton::Select => "CheckSelect",
            PadButton::B => "CheckB",
            PadButton::A => "CheckA",
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
/// - wCurKeys: Bitmap of currently pressed keys (0 = pressed, 1 = not pressed)
/// - wNewKeys: Bitmap of keys that just transitioned to pressed this frame
pub fn update_keys() -> Vec<Instr> {
    let mut asm = Asm::new();

    asm.comment("Poll half the controller (buttons)");
    asm.label("UpdateKeys");
    asm.ld(
        Operand::Reg(Register::A),
        Operand::Label("P1F_GET_BTN".to_string()),
    );
    asm.call(".onenibble");
    asm.ld(Operand::Reg(Register::B), Operand::Reg(Register::A));
    asm.comment("B7-4 = 1; B3-0 = unpressed buttons");

    asm.comment("Poll the other half (D-pad)");
    asm.ld(
        Operand::Reg(Register::A),
        Operand::Label("P1F_GET_DPAD".to_string()),
    );
    asm.call(".onenibble");
    asm.swap(Operand::Reg(Register::A));
    asm.comment("A7-4 = unpressed directions; A3-0 = 1");
    asm.xor(Operand::Reg(Register::A), Operand::Reg(Register::B));
    asm.comment("A = pressed buttons + directions");
    asm.ld(Operand::Reg(Register::B), Operand::Reg(Register::A));
    asm.comment("B = pressed buttons + directions");

    asm.comment("Release the controller");
    asm.ld(
        Operand::Reg(Register::A),
        Operand::Label("P1F_GET_NONE".to_string()),
    );
    asm.ldh(Operand::Label("rP1".to_string()), Operand::Reg(Register::A));

    asm.comment("Combine with previous wCurKeys to make wNewKeys");
    asm.ld(
        Operand::Reg(Register::A),
        Operand::Label("wCurKeys".to_string()),
    );
    asm.xor(Operand::Reg(Register::A), Operand::Reg(Register::B));
    asm.comment("A = keys that changed state");
    asm.and(Operand::Reg(Register::B));
    asm.comment("A = keys that changed to pressed");
    asm.ld(
        Operand::Label("wNewKeys".to_string()),
        Operand::Reg(Register::A),
    );
    asm.ld(Operand::Reg(Register::A), Operand::Reg(Register::B));
    asm.ld(
        Operand::Label("wCurKeys".to_string()),
        Operand::Reg(Register::A),
    );
    asm.ret();

    asm.comment("Helper function to read one nibble from joypad");
    asm.label(".onenibble");
    asm.ldh(Operand::Label("rP1".to_string()), Operand::Reg(Register::A));
    asm.comment("Switch the key matrix");
    asm.call(".knowret");
    asm.comment("Burn 10 cycles calling a known ret");
    asm.ldh(Operand::Reg(Register::A), Operand::Label("rP1".to_string()));
    asm.comment("Ignore value while waiting for key matrix to settle");
    asm.ldh(Operand::Reg(Register::A), Operand::Label("rP1".to_string()));
    asm.ldh(Operand::Reg(Register::A), Operand::Label("rP1".to_string()));
    asm.comment("This read counts");
    asm.or(Operand::Reg(Register::A), Operand::Imm(0xF0));
    asm.comment("A7-4 = 1; A3-0 = unpressed keys");

    asm.label(".knowret");
    asm.ret();

    asm.get_main_instrs()
}
//TODO check if it is ok, or we have to implement a big scope "check all keys"
// and one is pressed we jump at the end of the block
pub fn check_key(button: PadButton, pressed_func: Vec<Instr>) -> Vec<Instr> {
    let mut asm = Asm::new();
    asm.label(button.label());
    asm.ld(
        Operand::Label("wCurKeys".to_string()),
        Operand::Reg(Register::A),
    );
    asm.and(Operand::Label(button.name().to_string()));
    asm.jp_cond(Condition::Z, &format!("{}End", button.label()));
    asm.emit_all(pressed_func);
    asm.label(&format!("{}End", button.label()));
    asm.get_main_instrs()
}
