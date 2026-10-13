//! Input manager for handling Game Boy controller input
//!
//! Provides a high-level API for binding button presses to actions.

use crate::asm::{Block, Instr, LabelAllocator};
use crate::stdlib::inputs::PadButton;

/// A registered input binding
struct InputBinding {
    button: PadButton,
    action: Vec<Instr>,
    state: KeyState,
}

#[derive(Clone, Copy)]
enum KeyState {
    Held,
    JustPressed,
}

/// Manages input handling for the game
///
/// The InputManager provides a clean API for registering button actions.
/// When built, it automatically:
/// 1. Calls UpdateKeys to poll the controller
/// 2. Checks each registered button
/// 3. Executes the associated action when pressed
///
/// # Example
/// ```ignore
/// let mut inputs = InputManager::new();
/// inputs.on_press(PadButton::Left, gb.sprites.move_left_limit(paddle, 1, 16));
/// inputs.on_press(PadButton::Right, gb.sprites.move_right_limit(paddle, 1, 104));
/// gb.add_inputs(inputs);
/// ```
#[derive(Default)]
pub struct InputManager {
    bindings: Vec<InputBinding>,
}

impl InputManager {
    /// Create a new InputManager
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an action to execute when a button is pressed
    ///
    /// # Arguments
    /// * `button` - The button to check
    /// * `action` - Instructions to execute when the button is pressed
    pub fn on_press(&mut self, button: PadButton, action: Vec<Instr>) -> &mut Self {
        self.on_held(button, action)
    }

    /// Register an action to execute every frame while the button is held.
    ///
    /// This is the behavior `on_press` has always provided; that method remains as a
    /// compatibility alias.
    pub fn on_held(&mut self, button: PadButton, action: Vec<Instr>) -> &mut Self {
        self.bindings.push(InputBinding {
            button,
            action,
            state: KeyState::Held,
        });
        self
    }

    /// Register an action to execute only on the frame the button becomes pressed.
    ///
    /// This reads `wNewKeys`, which `UpdateKeys` updates once each main-loop frame.
    pub fn on_just_pressed(&mut self, button: PadButton, action: Vec<Instr>) -> &mut Self {
        self.bindings.push(InputBinding {
            button,
            action,
            state: KeyState::JustPressed,
        });
        self
    }

    /// Check if any bindings have been registered
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Generate the input handling code, with local labels numbered by `labels`
    ///
    /// This does NOT include the UpdateKeys call - that is handled by RustBoy
    /// to ensure the function is properly registered as used.
    pub(crate) fn generate_code(&self, labels: &LabelAllocator) -> Vec<Instr> {
        let mut instrs = Vec::new();

        for binding in &self.bindings {
            let variable = match binding.state {
                KeyState::Held => "wCurKeys",
                KeyState::JustPressed => "wNewKeys",
            };
            instrs.extend(check_key_in(
                labels,
                variable,
                binding.button,
                binding.action.clone(),
            ));
        }

        instrs
    }
}

fn check_key_in(
    labels: &LabelAllocator,
    variable: &str,
    button: PadButton,
    action: Vec<Instr>,
) -> Vec<Instr> {
    let end_stem = format!("{}_end", button.label());
    let [start, end] = labels.locals([button.label(), end_stem.as_str()]);
    let mut asm = Block::new();
    asm.label(&start);
    asm.ld_a_addr_def(variable);
    asm.and(button.flag());
    asm.jp_cond(crate::asm::Condition::Z, &end);
    asm.emit_all(action);
    asm.label(&end);
    asm.into_instrs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::Block;

    #[test]
    fn test_new_input_manager() {
        let inputs = InputManager::new();
        assert!(inputs.is_empty());
    }

    #[test]
    fn test_on_press() {
        let mut inputs = InputManager::new();
        let mut asm = Block::new();
        asm.ret();

        inputs.on_press(PadButton::Left, asm.into_instrs());
        assert!(!inputs.is_empty());
    }

    #[test]
    fn test_generate_code() {
        let mut inputs = InputManager::new();
        let mut asm = Block::new();
        asm.ret();

        inputs.on_press(PadButton::A, asm.into_instrs());
        let code = inputs.generate_code(&LabelAllocator::new());

        // Should contain check_key generated code
        assert!(!code.is_empty());
    }

    #[test]
    fn held_and_just_pressed_bindings_read_their_own_state() {
        let mut inputs = InputManager::new();
        inputs
            .on_held(PadButton::A, Vec::new())
            .on_just_pressed(PadButton::A, Vec::new());

        let code = format!("{:?}", inputs.generate_code(&LabelAllocator::new()));
        assert!(code.contains("wCurKeys"), "{code}");
        assert!(code.contains("wNewKeys"), "{code}");
    }
}
