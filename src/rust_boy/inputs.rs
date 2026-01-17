//! Input manager for handling Game Boy controller input
//!
//! Provides a high-level API for binding button presses to actions.

use crate::gb_asm::Instr;
use crate::gb_std::inputs::{PadButton, check_key};

/// A registered input binding
struct InputBinding {
    button: PadButton,
    action: Vec<Instr>,
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
/// inputs.on_press(PadButton::Left, gb.sprites.move_left_limit(paddle, 1, 15));
/// inputs.on_press(PadButton::Right, gb.sprites.move_right_limit(paddle, 1, 105));
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
        self.bindings.push(InputBinding { button, action });
        self
    }

    /// Check if any bindings have been registered
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Generate the input handling code
    ///
    /// This does NOT include the UpdateKeys call - that is handled by RustBoy
    /// to ensure the function is properly registered as used.
    pub(crate) fn generate_code(&self) -> Vec<Instr> {
        let mut instrs = Vec::new();

        for binding in &self.bindings {
            instrs.extend(check_key(binding.button, binding.action.clone()));
        }

        instrs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::Asm;

    #[test]
    fn test_new_input_manager() {
        let inputs = InputManager::new();
        assert!(inputs.is_empty());
    }

    #[test]
    fn test_on_press() {
        let mut inputs = InputManager::new();
        let mut asm = Asm::new();
        asm.ret();

        inputs.on_press(PadButton::Left, asm.get_main_instrs());
        assert!(!inputs.is_empty());
    }

    #[test]
    fn test_generate_code() {
        let mut inputs = InputManager::new();
        let mut asm = Asm::new();
        asm.ret();

        inputs.on_press(PadButton::A, asm.get_main_instrs());
        let code = inputs.generate_code();

        // Should contain check_key generated code
        assert!(!code.is_empty());
    }
}
