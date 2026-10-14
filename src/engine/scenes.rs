//! A small scene dispatcher built from regular [`Routine`](crate::stdlib::routine::Routine)s.
//!
//! Register one routine per scene, call the dispatcher from the main loop, and use
//! [`SceneManager::switch_to`] wherever the game changes state. Scene routines and their
//! dependencies are registered automatically when the dispatcher is passed to
//! [`RustBoy::call_routine`](super::rustboy::RustBoy::call_routine).

use std::collections::BTreeMap;

use crate::asm::{Block, Condition, Instr};
use crate::stdlib::routine::Routine;

/// Maps byte-sized scene IDs to update routines and builds a dispatcher routine.
#[derive(Clone, Debug)]
pub struct SceneManager {
    dispatcher: String,
    state: String,
    scenes: BTreeMap<u8, Routine>,
}

impl SceneManager {
    /// Create a scene manager with a dispatcher label and WRAM state variable.
    ///
    /// The state variable is declared by the dispatcher as a zero-initialized byte unless
    /// the program defines it first. Scene IDs are bytes; zero selects the initial scene.
    /// The dispatcher name and state variable must not collide with other program symbols.
    pub fn new(dispatcher: &str, state_variable: &str) -> Self {
        Self {
            dispatcher: dispatcher.to_string(),
            state: state_variable.to_string(),
            scenes: BTreeMap::new(),
        }
    }

    /// Register a scene update routine for `id`.
    ///
    /// # Panics
    /// If `id` is already registered.
    pub fn add_scene(&mut self, id: u8, update: Routine) -> &mut Self {
        assert!(
            !self.scenes.contains_key(&id),
            "scene id {id} is already registered"
        );
        self.scenes.insert(id, update);
        self
    }

    /// Generate instructions that select `id` for the next dispatcher call.
    /// The state takes effect the next time the dispatcher runs. Register the dispatcher
    /// with `RustBoy::call_routine` first so its state variable is declared.
    pub fn switch_to(&self, id: u8) -> Vec<Instr> {
        let mut code = Block::new();
        code.ld_a(id).ld_addr_def_a(&self.state);
        code.into_instrs()
    }

    /// Build a routine that calls the update routine selected by the state byte.
    /// Unknown IDs do nothing. Scene routines are dependencies and are emitted once.
    pub fn dispatcher(&self) -> Routine {
        let mut code = Block::new();
        code.label(&self.dispatcher);
        for (id, scene) in &self.scenes {
            let next = format!(".scene_next_{id}");
            code.ld_a_addr_def(&self.state)
                .cp_imm(*id)
                .jp_cond(Condition::NZ, &next)
                .call(scene.name())
                .ret()
                .label(&next);
        }
        code.ret();

        let mut dispatcher =
            Routine::new(&self.dispatcher, code.into_instrs()).with_variable(&self.state);
        for scene in self.scenes.values() {
            dispatcher = dispatcher.with_dep(scene.clone());
        }
        dispatcher
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::Asm;

    fn update() -> Routine {
        let mut code = Block::new();
        code.label("UpdateTitle").ret();
        Routine::new("UpdateTitle", code.into_instrs())
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn duplicate_scene_ids_are_rejected() {
        let mut scenes = SceneManager::new("UpdateScene", "wCurrentScene");
        scenes.add_scene(0, update()).add_scene(0, update());
    }

    #[test]
    fn switch_and_dispatch_instructions_are_stable() {
        let mut scenes = SceneManager::new("UpdateScene", "wCurrentScene");
        scenes.add_scene(2, update());
        let mut expected = Block::new();
        expected.ld_a(9).ld_addr_def_a("wCurrentScene");
        assert_eq!(scenes.switch_to(9), expected.into_instrs());
        let mut expected = Block::new();
        expected
            .label("UpdateScene")
            .ld_a_addr_def("wCurrentScene")
            .cp_imm(2)
            .jp_cond(Condition::NZ, ".scene_next_2")
            .call("UpdateTitle")
            .ret()
            .label(".scene_next_2")
            .ret();
        let mut actual = Asm::new();
        actual.emit_all(scenes.dispatcher());
        let mut wanted = Asm::new();
        wanted.emit_all(expected.into_instrs());
        assert_eq!(actual.to_asm(), wanted.to_asm());
    }
}
