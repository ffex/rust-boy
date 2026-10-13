//! Tilemap animations with queue-aware timing.
use super::{AnimationType, Chunk, RustBoy, TilemapArea, Var};
use crate::asm::{Block, Instr};
use crate::stdlib::graphics::tile_animation::{self, TileAnimationState, TilePlayback};

/// Fixed map cells and tile-ID frames. Frame graphics must already be in VRAM.
#[derive(Debug, Clone)]
pub struct BackgroundAnimationConfig {
    area: TilemapArea,
    cells: Vec<(u8, u8)>,
    frames: Vec<Vec<u8>>,
    delay: u8,
    playback: AnimationType,
}
impl BackgroundAnimationConfig {
    /// Each frame supplies one tile per cell, in the same order.
    ///
    /// The cell targets and the frames are ROM tables in the single ROM0 bank
    /// (2 bytes per cell, 2 per frame, plus one byte per cell per frame): tables that do not
    /// fit with the rest of the program make rgbasm fail ("Section grew too big").
    ///
    /// # Panics
    /// If `cells` is empty or has a duplicate or a cell outside the 32×32 map, or `frames`
    /// is empty, has more than 256 frames, or a frame whose length is not the cell count.
    #[track_caller]
    pub fn new(area: TilemapArea, cells: &[(u8, u8)], frames: &[Vec<u8>]) -> Self {
        assert!(!cells.is_empty(), "animation needs map cells");
        let mut seen = std::collections::BTreeSet::new();
        for &(x, y) in cells {
            assert!(
                x < crate::hw::SCRN_VX_B.value
                    && y < crate::hw::SCRN_VY_B.value
                    && seen.insert((x, y)),
                "invalid/duplicate animation cell"
            );
        }
        assert!(
            (1..=256).contains(&frames.len()),
            "animation needs 1..=256 frames"
        );
        assert!(
            frames.iter().all(|frame| frame.len() == cells.len()),
            "frame/cell count mismatch"
        );
        Self {
            area,
            cells: cells.to_vec(),
            frames: frames.to_vec(),
            delay: 1,
            playback: AnimationType::Loop,
        }
    }
    /// Calls per fully queued frame, 1..=255 (default 1). Backpressure extends it.
    ///
    /// # Panics
    /// If `delay` is 0.
    #[track_caller]
    pub fn delay(mut self, delay: u8) -> Self {
        assert!(delay > 0, "animation delay cannot be zero");
        self.delay = delay;
        self
    }
    /// Loop (the default), PingPong or Once.
    pub fn playback(mut self, playback: AnimationType) -> Self {
        self.playback = playback;
        self
    }
}
/// A persistent animator. Call `step` exactly once per game frame from init/main.
#[derive(Debug, Clone)]
pub struct BackgroundAnimation {
    routine: String,
    cursor: Var,
    frame: Var,
    timer: Var,
    direction: Var,
    enabled: Var,
    finished: Var,
}
impl BackgroundAnimation {
    /// Queue/resume the current frame. Carry means queue full. Changes all registers.
    /// Fully queued frames need a subsequent VBlank before they are visible.
    pub fn step(&self) -> Vec<Instr> {
        let mut b = Block::new();
        b.call(&self.routine);
        b.into_instrs()
    }
    /// Pause without changing the current frame, hold timer or partial queue progress.
    pub fn pause(&self) -> Vec<Instr> {
        self.enabled.set(0)
    }
    /// Resume a paused animation where it stopped.
    pub fn resume(&self) -> Vec<Instr> {
        self.enabled.set(1)
    }
    /// Replay from frame zero. Pending queue entries are retained; they flush first.
    pub fn restart(&self) -> Vec<Instr> {
        let mut b = Block::new();
        for var in [
            &self.cursor,
            &self.frame,
            &self.timer,
            &self.direction,
            &self.finished,
        ] {
            b.emit_all(var.set(0));
        }
        b.emit_all(self.enabled.set(1));
        b.into_instrs()
    }
    /// Loads A with 1 after Once's final frame is queued, else 0. Not a flush fence.
    pub fn finished(&self) -> Vec<Instr> {
        self.finished.get()
    }
}
impl RustBoy {
    /// Animate one or more cells on either background/window map. Registers the
    /// VBlank queue consumer automatically. Each animator reserves seven WRAM bytes.
    /// All tile IDs refer to preloaded graphics. Steps cannot run in handlers.
    ///
    /// # Panics
    /// If `name` cannot be a label, or another background animation or a variable already
    /// uses its routine or state names.
    #[track_caller]
    pub fn create_background_animation(
        &mut self,
        name: &str,
        config: BackgroundAnimationConfig,
    ) -> BackgroundAnimation {
        super::sprites::check_label("background animation", name);
        let routine = format!("UpdateBgAnim{name}");
        assert!(
            !self.function_exists(&routine),
            "duplicate background animation {name}"
        );
        let s = TileAnimationState::new(name);
        for reserved in [
            &s.cursor,
            &s.frame,
            &s.timer,
            &s.direction,
            &s.enabled,
            &s.finished,
        ] {
            assert!(
                !self.vars.names().any(|(existing, _)| existing == reserved),
                "duplicate animation storage {reserved}"
            );
        }
        let cursor = self.vars.create_u16(&s.cursor, 0);
        let frame = self.vars.create_u8(&s.frame, 0);
        let timer = self.vars.create_u8(&s.timer, 0);
        let direction = self.vars.create_u8(&s.direction, 0);
        let enabled = self.vars.create_u8(&s.enabled, 1);
        let finished = self.vars.create_u8(&s.finished, 0);
        let targets = format!("BgAnim{name}Targets");
        let frames = format!("BgAnim{name}Frames");
        self.raw(|asm| {
            asm.chunk(Chunk::Tiles).label(&targets).dw(&config
                .cells
                .iter()
                .map(|&(x, y)| {
                    format!(
                        "${:04X}",
                        config.area.address()
                            + u16::from(y) * u16::from(crate::hw::SCRN_VX_B.value)
                            + u16::from(x)
                    )
                })
                .collect::<Vec<_>>()
                .join(", "));
            asm.label(&frames).dw(&(0..config.frames.len())
                .map(|i| format!("BgAnim{name}Frame{i}"))
                .collect::<Vec<_>>()
                .join(", "));
            for (i, tiles) in config.frames.iter().enumerate() {
                asm.label(&format!("BgAnim{name}Frame{i}")).db(&tiles
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(", "));
            }
        });
        let playback = match config.playback {
            AnimationType::Loop => TilePlayback::Loop,
            AnimationType::PingPong => TilePlayback::PingPong,
            AnimationType::Once => TilePlayback::Once,
        };
        self.queue_vram_write_from_registers();
        self.call_routine(&tile_animation::animate(
            name,
            &targets,
            &frames,
            config.cells.len() as u16,
            config.frames.len() as u16,
            config.delay,
            playback,
        ));
        BackgroundAnimation {
            routine,
            cursor,
            frame,
            timer,
            direction,
            enabled,
            finished,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Error, panic_message};
    #[test]
    fn invalid_config_and_duplicate_names_are_rejected() {
        for (cells, frames) in [
            (vec![], vec![vec![]]),
            (vec![(32, 0)], vec![vec![0]]),
            (vec![(0, 0), (0, 0)], vec![vec![0, 1]]),
            (vec![(0, 0)], vec![]),
            (vec![(0, 0)], vec![vec![]]),
            (vec![(0, 0)], vec![vec![0]; 257]),
        ] {
            assert!(
                !panic_message(|| BackgroundAnimationConfig::new(
                    TilemapArea::Map9800,
                    &cells,
                    &frames
                ))
                .is_empty()
            );
        }
        let config = BackgroundAnimationConfig::new(TilemapArea::Map9800, &[(0, 0)], &[vec![0]]);
        assert!(!panic_message(|| config.clone().delay(0)).is_empty());
        let mut game = RustBoy::new();
        let animator = game.create_background_animation("One", config.clone());
        game.add_to_main_loop(animator.step());
        let before = game.build().unwrap();
        assert_eq!(before, game.build().unwrap());
        assert!(!panic_message(|| game.create_background_animation("One", config)).is_empty());
        assert_eq!(before, game.build().unwrap());
        game.on_vblank(animator.step());
        assert!(matches!(game.build(), Err(Error::MainLoopOnly { .. })));
    }
}
