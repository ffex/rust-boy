//! Input manager for handling Game Boy controller input.

use crate::asm::{Block, Condition, Instr, LabelAllocator, R8};
use crate::stdlib::inputs::{PadButton, check_key_state};

enum BindingKind {
    Held,
    JustPressed,
    Released,
    Repeat { delay: u8, interval: u8 },
}

struct InputBinding {
    button: PadButton,
    action: Vec<Instr>,
    kind: BindingKind,
}

/// Registers actions for held, newly pressed, released, or repeating buttons.
#[derive(Default)]
pub struct InputManager {
    bindings: Vec<InputBinding>,
}

impl InputManager {
    /// Create a new input manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Run `action` every frame while `button` is held. This preserves the original
    /// behavior of `on_press`; use [`on_just_pressed`](Self::on_just_pressed) for an edge.
    pub fn on_press(&mut self, button: PadButton, action: Vec<Instr>) -> &mut Self {
        self.push(button, action, BindingKind::Held)
    }

    /// Run `action` every frame while `button` is held.
    pub fn on_held(&mut self, button: PadButton, action: Vec<Instr>) -> &mut Self {
        self.push(button, action, BindingKind::Held)
    }

    /// Run `action` only on the frame `button` transitions from up to down.
    pub fn on_just_pressed(&mut self, button: PadButton, action: Vec<Instr>) -> &mut Self {
        self.push(button, action, BindingKind::JustPressed)
    }

    /// Run `action` only on the frame `button` transitions from down to up.
    pub fn on_release(&mut self, button: PadButton, action: Vec<Instr>) -> &mut Self {
        self.push(button, action, BindingKind::Released)
    }

    /// Run `action` immediately on press, then after `delay` held frames and every
    /// `interval` held frames. Both frame counts must be in `1..=255`.
    #[track_caller]
    pub fn on_repeat(
        &mut self,
        button: PadButton,
        delay: u8,
        interval: u8,
        action: Vec<Instr>,
    ) -> &mut Self {
        assert!(delay > 0, "repeat delay must be in 1..=255");
        assert!(interval > 0, "repeat interval must be in 1..=255");
        self.push(button, action, BindingKind::Repeat { delay, interval })
    }

    fn push(&mut self, button: PadButton, action: Vec<Instr>, kind: BindingKind) -> &mut Self {
        self.bindings.push(InputBinding {
            button,
            action,
            kind,
        });
        self
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    pub(crate) fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    pub(crate) fn generate_code(&self, labels: &LabelAllocator, first_id: usize) -> Vec<Instr> {
        let mut out = Vec::new();
        for (index, binding) in self.bindings.iter().enumerate() {
            match binding.kind {
                BindingKind::Held => out.extend(check_key_state(
                    labels,
                    binding.button,
                    "wCurKeys",
                    binding.action.clone(),
                )),
                BindingKind::JustPressed => out.extend(check_key_state(
                    labels,
                    binding.button,
                    "wNewKeys",
                    binding.action.clone(),
                )),
                BindingKind::Released => out.extend(check_key_state(
                    labels,
                    binding.button,
                    "wReleasedKeys",
                    binding.action.clone(),
                )),
                BindingKind::Repeat { delay, interval } => {
                    let id = first_id + index;
                    let new_stem = format!("input_repeat_new_{id}");
                    let held_stem = format!("input_repeat_held_{id}");
                    let action_stem = format!("input_repeat_action_{id}");
                    let done_stem = format!("input_repeat_done_{id}");
                    let [new, held, action, done] = labels.locals([
                        new_stem.as_str(),
                        held_stem.as_str(),
                        action_stem.as_str(),
                        done_stem.as_str(),
                    ]);
                    let state = format!("wInputRepeat_{id}");
                    let mut code = Block::new();
                    code.ld_a_addr_def("wNewKeys").and(binding.button.flag());
                    code.jp_cond(Condition::NZ, &new);
                    code.ld_a_addr_def("wCurKeys").and(binding.button.flag());
                    code.jp_cond(Condition::NZ, &held);
                    code.xor(R8::A).ld_addr_def_a(&state).jp(&done);
                    code.label(&held);
                    code.ld_a_addr_def(&state).dec(R8::A).ld_addr_def_a(&state);
                    code.jp_cond(Condition::NZ, &done);
                    code.ld_a(interval).ld_addr_def_a(&state).jp(&action);
                    code.label(&new).ld_a(delay).ld_addr_def_a(&state);
                    // Emit label-bearing user snippets once for both trigger paths.
                    code.label(&action).emit_all(binding.action.clone());
                    code.label(&done);
                    out.extend(code.into_instrs());
                }
            }
        }
        out
    }

    pub(crate) fn repeat_variable_names(&self, first_id: usize) -> Vec<String> {
        self.bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| matches!(binding.kind, BindingKind::Repeat { .. }))
            .map(|(index, _)| format!("wInputRepeat_{}", first_id + index))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_keep_same_button_actions_and_allocate_repeat_state() {
        let mut inputs = InputManager::new();
        inputs
            .on_held(PadButton::A, Vec::new())
            .on_just_pressed(PadButton::A, Vec::new())
            .on_release(PadButton::A, Vec::new())
            .on_repeat(PadButton::A, 4, 2, Vec::new());
        let code = inputs.generate_code(&LabelAllocator::new(), 3);
        assert_eq!(inputs.binding_count(), 4);
        assert_eq!(inputs.repeat_variable_names(3), ["wInputRepeat_6"]);
        assert!(!code.is_empty());
    }

    #[test]
    fn two_repeats_on_one_button_use_distinct_state_and_labels() {
        let mut inputs = InputManager::new();
        inputs
            .on_repeat(PadButton::A, 3, 2, Vec::new())
            .on_repeat(PadButton::A, 5, 4, Vec::new());
        let code = inputs.generate_code(&LabelAllocator::new(), 7);
        let asm = code.iter().map(Instr::to_string).collect::<Vec<_>>();

        assert_eq!(
            inputs.repeat_variable_names(7),
            ["wInputRepeat_7", "wInputRepeat_8"]
        );
        for (id, label_id) in [(7, 0), (8, 1)] {
            assert!(
                asm.iter()
                    .any(|line| line == &format!(".input_repeat_new_{id}_{label_id}:"))
            );
            assert!(
                asm.iter()
                    .any(|line| line == &format!(".input_repeat_held_{id}_{label_id}:"))
            );
            assert!(
                asm.iter()
                    .any(|line| line == &format!(".input_repeat_done_{id}_{label_id}:"))
            );
            assert!(
                asm.iter()
                    .any(|line| line == &format!("ld [wInputRepeat_{id}], a"))
            );
        }
    }

    #[test]
    #[should_panic(expected = "repeat delay")]
    fn repeat_rejects_zero_delay() {
        InputManager::new().on_repeat(PadButton::A, 0, 1, Vec::new());
    }

    #[test]
    #[should_panic(expected = "repeat interval")]
    fn repeat_rejects_zero_interval() {
        InputManager::new().on_repeat(PadButton::A, 1, 0, Vec::new());
    }
}
