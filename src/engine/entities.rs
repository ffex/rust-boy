//! Runtime entities backed by reusable OAM slots.

use crate::asm::Instr;

use super::sprite_pool::SpritePool;

/// A pool of single-sprite runtime entities.
///
/// Entity indices are runtime values held in `A`; a pool allocates the lowest free
/// index. Each entity is one hardware sprite. Use the sprite tile index and OAM flags
/// when writing it. Calls belong in init or the main loop, not an interrupt handler.
#[derive(Debug, Clone)]
pub struct EntityPool(SpritePool);

impl EntityPool {
    pub(crate) fn new(pool: SpritePool) -> Self {
        Self(pool)
    }

    /// The pool's name, used in generated assembly labels.
    pub fn name(&self) -> &str {
        self.0.name()
    }

    /// The maximum number of entities that can be alive at once.
    pub fn capacity(&self) -> u8 {
        self.0.capacity()
    }

    /// Spawn one entity in a hidden state. On success A is its relative entity index,
    /// carry is clear, and HL points to its cleared shadow-OAM entry. When full, A is
    /// 255 and carry is set. Changes AF/BC/HL, preserves DE and IME.
    pub fn spawn(&self) -> Vec<Instr> {
        self.0.allocate()
    }

    /// Despawn the entity at relative index A, clearing its OAM entry and freeing its
    /// slot. A double despawn or invalid index sets carry without changing memory.
    /// Changes AF/HL; preserves BC/DE and IME.
    pub fn despawn(&self) -> Vec<Instr> {
        self.0.release()
    }

    /// Set/show entity A at screen position B,C with tile D and OAM flags E.
    /// Carry reports an invalid, unallocated index or invalid position/tile.
    /// Changes AF/HL; preserves BC/DE and IME.
    pub fn set_sprite(&self) -> Vec<Instr> {
        self.0.write()
    }

    /// Hide entity A while keeping it alive; `set_sprite` shows it again. Carry reports an
    /// invalid or unallocated index. Changes AF/HL; preserves BC/DE and IME.
    pub fn hide(&self) -> Vec<Instr> {
        self.0.hide()
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::RustBoy;

    #[test]
    fn entity_pool_exposes_spawn_and_despawn_routines() {
        let mut gb = RustBoy::new();
        let entities = gb.create_entity_pool("Actors", 3);
        assert_eq!(entities.name(), "Actors");
        assert_eq!(entities.capacity(), 3);

        gb.init(entities.spawn());
        gb.add_to_main_loop(entities.set_sprite());
        gb.add_to_main_loop(entities.hide());
        gb.add_to_main_loop(entities.despawn());
        let asm = gb.build().unwrap();
        assert!(asm.contains("call PoolActorsAllocate"));
        assert!(asm.contains("call PoolActorsWrite"));
        assert!(asm.contains("call PoolActorsHide"));
        assert!(asm.contains("call PoolActorsRelease"));
        assert!(asm.contains("PoolActorsAllocate:"));
        assert!(asm.contains("PoolActorsWrite:"));
        assert!(asm.contains("PoolActorsHide:"));
        assert!(asm.contains("PoolActorsRelease:"));
    }
}
