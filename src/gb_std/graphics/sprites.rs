use crate::gb_asm::{Asm, Condition, Instr, LabelAllocator, Operand, Register};

pub fn clear_objects_screen() -> Vec<Instr> {
    let mut asm = Asm::new();
    // Clear OAM loop
    asm.label("ClearOam")
        .ld_hli_label("a")
        .dec_label("b")
        .jp_cond(Condition::NZ, "ClearOam");
    asm.get_main_instrs()
}

pub fn initialize_objects_screen() -> Vec<Instr> {
    let mut asm = Asm::new();
    // Initialize OAM
    asm.ld_a(0).ld_b(160).ld_hl_label("_OAMRAM");
    asm.get_main_instrs()
}

/// Which way a limited move changes a sprite coordinate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveDir {
    /// Left or up: the coordinate decreases, and the limit is the smallest value it may take
    Decrease,
    /// Right or down: the coordinate increases, and the limit is the largest value it may take
    Increase,
}

/// Move a sprite coordinate by `distance` without ever going past `limit`
///
/// `coord` is the address of the coordinate, an OAM Y or X byte (e.g. `_OAMRAM+1`).
/// The limit is included: it is the smallest value the coordinate may take when it
/// decreases (left, up) and the largest when it increases (right, down).
/// - A step that would go past the limit stops exactly on it, whatever the distance, so
///   the sprite reaches the same edge at any speed and never wraps around 0 / 255.
/// - A coordinate already beyond the limit does not move: the move never takes it
///   further, and does not pull it back either.
///
/// `followers` are the other sprites of a composite: each `(address, offset)` is set to
/// the new value of `coord` plus `offset`, so the whole composite moves as one block or
/// not at all. `coord` must then be the one that reaches the limit first (the smallest
/// when decreasing, the largest when increasing).
///
/// Uses A and the flags, and two local labels from `labels`, `.{stem}_N_store` and
/// `.{stem}_N_end` (B7): the move can be emitted any number of times, and inside an `If`.
pub(crate) fn move_coord_limit(
    labels: &LabelAllocator,
    stem: &str,
    coord: &str,
    followers: &[(String, i16)],
    dir: MoveDir,
    distance: u8,
    limit: u8,
) -> Vec<Instr> {
    let label = labels.local(stem);
    let store = format!("{}_store", label);
    let end = format!("{}_end", label);
    let mut asm = Asm::new();

    // Work on the offset from the limit, A = coord - limit, so the limit is at 0 and
    // the carry flag of each step tells on which side of it the coordinate is
    asm.ld_a_addr_def(coord)
        .sub(Operand::Reg(Register::A), Operand::Imm(limit));
    match dir {
        MoveDir::Decrease => {
            // Carry: coord < limit, already past the limit
            asm.jp_cond(Condition::C, &end)
                // Carry: the step goes past the limit
                .sub(Operand::Reg(Register::A), Operand::Imm(distance));
        }
        MoveDir::Increase => {
            // No carry: coord >= limit, on the limit or past it
            asm.jp_cond(Condition::NC, &end)
                // A is coord - limit + 256 here; carry: the step reaches the limit or goes past it
                .add(Operand::Reg(Register::A), Operand::Imm(distance));
        }
    }
    asm.jp_cond(Condition::NC, &store)
        .ld_a(0) // stop exactly on the limit
        .label(&store)
        .add(Operand::Reg(Register::A), Operand::Imm(limit))
        .ld_addr_def_a(coord);

    // A holds the new coord; put each follower at its offset from it
    let mut a_offset = 0i16;
    for (follower, offset) in followers {
        let step = offset - a_offset;
        if step > 0 {
            asm.add(Operand::Reg(Register::A), Operand::Imm(step as u8));
        } else if step < 0 {
            asm.sub(
                Operand::Reg(Register::A),
                Operand::Imm(step.unsigned_abs() as u8),
            );
        }
        asm.ld_addr_def_a(follower);
        a_offset = *offset;
    }

    asm.label(&end);
    asm.get_main_instrs()
}

pub struct SpriteManager {
    sprites: Vec<Sprite>,
    current_sprite_index: u8,
}
impl Default for SpriteManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SpriteManager {
    pub fn new() -> Self {
        SpriteManager {
            sprites: Vec::new(),
            current_sprite_index: 0,
        }
    }
    pub fn add_sprite(&mut self, x: u8, y: u8, tile: u8, flags: u8) {
        let sprite = Sprite::new(self.current_sprite_index, x, y, tile, flags);
        self.sprites.push(sprite);
        self.current_sprite_index += 1;
    }
    pub fn draw(&self) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_hl_label("_OAMRAM");
        for sprite in &self.sprites {
            asm.emit_all(sprite.draw());
        }
        asm.get_main_instrs()
    }
    pub fn get_sprite(&self, id: u8) -> Option<&Sprite> {
        self.sprites.iter().find(|s| s.id == id)
    }
    pub fn get_sprite_mut(&mut self, id: u8) -> Option<&mut Sprite> {
        self.sprites.iter_mut().find(|s| s.id == id)
    }
}
pub struct Sprite {
    pub id: u8,
    pub x: u8,
    pub y: u8,
    pub tile: u8,
    pub flags: u8,
}

impl Sprite {
    pub fn new(id: u8, x: u8, y: u8, tile: u8, flags: u8) -> Self {
        Sprite {
            id,
            x,
            y,
            tile,
            flags,
        }
    }
    pub fn draw(&self) -> Vec<Instr> {
        let mut asm = Asm::new();

        asm.ld_a(self.y + 16)
            .ld_hli_label("a")
            .ld_a(self.x + 8)
            .ld_hli_label("a")
            .ld_a(self.tile)
            .ld_hli_label("a")
            .ld_a(self.flags)
            .ld_hli_label("a");
        asm.get_main_instrs()
    }

    /// Move the sprite left by `distance` pixels, with no limit
    ///
    /// The plain moves emit no labels, so they can be used any number of times and
    /// inside an `If` (they used to emit the global labels `Left:` / `LeftEnd:`, B7).
    pub fn move_left(&mut self, distance: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1))
            .sub(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 1));
        asm.get_main_instrs()
    }

    /// Move the sprite right by `distance` pixels, with no limit; see [`Sprite::move_left`]
    pub fn move_right(&mut self, distance: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1))
            .add(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 1));
        asm.get_main_instrs()
    }

    /// Move the sprite up by `distance` pixels, with no limit; see [`Sprite::move_left`]
    pub fn move_up(&mut self, distance: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4))
            .sub(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4));
        asm.get_main_instrs()
    }

    /// Move the sprite down by `distance` pixels, with no limit; see [`Sprite::move_left`]
    pub fn move_down(&mut self, distance: u8) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4))
            .add(Operand::Reg(Register::A), Operand::Imm(distance))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4));
        asm.get_main_instrs()
    }

    /// Move the sprite left by `distance` pixels, but never left of `limit` (an OAM
    /// X, screen x + 8): a step that would go past the limit stops exactly on it, and a
    /// sprite already past the limit does not move
    ///
    /// Its two local labels come from `labels` (`.sprite0_left_limit_3_store`, `…_end`).
    pub fn move_left_limit(
        &mut self,
        labels: &LabelAllocator,
        distance: u8,
        limit: u8,
    ) -> Vec<Instr> {
        self.move_limit(labels, "left", 1, MoveDir::Decrease, distance, limit)
    }

    /// Move the sprite right by `distance` pixels, but never right of `limit` (an OAM
    /// X, screen x + 8); see [`Sprite::move_left_limit`]
    pub fn move_right_limit(
        &mut self,
        labels: &LabelAllocator,
        distance: u8,
        limit: u8,
    ) -> Vec<Instr> {
        self.move_limit(labels, "right", 1, MoveDir::Increase, distance, limit)
    }

    /// Move the sprite up by `distance` pixels, but never above `limit` (an OAM
    /// Y, screen y + 16); see [`Sprite::move_left_limit`]
    pub fn move_up_limit(
        &mut self,
        labels: &LabelAllocator,
        distance: u8,
        limit: u8,
    ) -> Vec<Instr> {
        self.move_limit(labels, "up", 0, MoveDir::Decrease, distance, limit)
    }

    /// Move the sprite down by `distance` pixels, but never below `limit` (an OAM
    /// Y, screen y + 16); see [`Sprite::move_left_limit`]
    pub fn move_down_limit(
        &mut self,
        labels: &LabelAllocator,
        distance: u8,
        limit: u8,
    ) -> Vec<Instr> {
        self.move_limit(labels, "down", 0, MoveDir::Increase, distance, limit)
    }

    /// Limited move of byte `byte` of the sprite's OAM entry (Y = 0, X = 1)
    fn move_limit(
        &self,
        labels: &LabelAllocator,
        name: &str,
        byte: u8,
        dir: MoveDir,
        distance: u8,
        limit: u8,
    ) -> Vec<Instr> {
        move_coord_limit(
            labels,
            &format!("sprite{}_{}_limit", self.id, name),
            &format!("_OAMRAM+{}", self.id * 4 + byte),
            &[],
            dir,
            distance,
            limit,
        )
    }

    pub fn move_x_var(&mut self, var_name: &str) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(var_name)
            .ld(Operand::Reg(Register::B), Operand::Reg(Register::A))
            .ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1))
            .add(Operand::Reg(Register::A), Operand::Reg(Register::B))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4 + 1));

        asm.get_main_instrs()
    }

    pub fn move_y_var(&mut self, var_name: &str) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(var_name)
            .ld(Operand::Reg(Register::B), Operand::Reg(Register::A))
            .ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4))
            .add(Operand::Reg(Register::A), Operand::Reg(Register::B))
            .ld_addr_def_a(&format!("_OAMRAM+{}", self.id * 4));

        asm.get_main_instrs()
    }

    pub fn get_pivot(&self, x_offset: i16, y_offset: i16) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4))
            .sub(
                Operand::Reg(Register::A),
                Operand::Imm(u8::try_from(16i16 + y_offset).unwrap_or(0)),
            )
            .ld(Operand::Reg(Register::C), Operand::Reg(Register::A))
            .ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1))
            .sub(
                Operand::Reg(Register::A),
                Operand::Imm(u8::try_from(8i16 + x_offset).unwrap_or(0)),
            )
            .ld(Operand::Reg(Register::B), Operand::Reg(Register::A));

        asm.get_main_instrs()
    }

    /// Get sprite Y position into register A (for use with If statements)
    pub fn get_y(&self) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4));
        asm.get_main_instrs()
    }
    /// Get sprite X position into register A (for use with If statements)
    pub fn get_x(&self) -> Vec<Instr> {
        let mut asm = Asm::new();
        asm.ld_a_addr_def(&format!("_OAMRAM+{}", self.id * 4 + 1));
        asm.get_main_instrs()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::gb_asm::label_check::assert_code_labels_ok;
    use crate::gb_asm::test_cpu::TestCpu;
    use crate::gb_std::flow::{Emittable, If};

    /// What a limited move must do to a coordinate at `pos`: move it by `distance`, but
    /// stop exactly on `limit` instead of passing it; a coordinate already beyond the
    /// limit stays where it is.
    pub(crate) fn expected_move(dir: MoveDir, pos: u8, distance: u8, limit: u8) -> u8 {
        match dir {
            MoveDir::Decrease if pos < limit => pos,
            MoveDir::Decrease => pos.saturating_sub(distance).max(limit),
            MoveDir::Increase if pos > limit => pos,
            MoveDir::Increase => pos.saturating_add(distance).min(limit),
        }
    }

    /// Distances and limits the limited moves are tested with (every start position is)
    pub(crate) const DISTANCES: [u8; 7] = [0, 1, 2, 3, 8, 100, 255];
    pub(crate) const LIMITS: [u8; 10] = [0, 1, 8, 15, 16, 104, 149, 160, 254, 255];

    type LimitMove = fn(&mut Sprite, &LabelAllocator, u8, u8) -> Vec<Instr>;

    #[test]
    fn test_limit_moves_stop_exactly_on_the_limit() {
        let labels = LabelAllocator::new();
        // (name, method, byte of the OAM entry it moves (Y = 0, X = 1), direction)
        let moves: [(&str, LimitMove, u8, MoveDir); 4] = [
            ("left", Sprite::move_left_limit, 1, MoveDir::Decrease),
            ("right", Sprite::move_right_limit, 1, MoveDir::Increase),
            ("up", Sprite::move_up_limit, 0, MoveDir::Decrease),
            ("down", Sprite::move_down_limit, 0, MoveDir::Increase),
        ];
        // Sprite 1: its Y and X are at _OAMRAM+4 and _OAMRAM+5
        let mut sprite = Sprite::new(1, 0, 0, 0, 0);

        for (name, method, byte, dir) in moves {
            let moved = format!("_OAMRAM+{}", 4 + byte);
            let other = format!("_OAMRAM+{}", 4 + (1 - byte));
            for distance in DISTANCES {
                for limit in LIMITS {
                    let code = method(&mut sprite, &labels, distance, limit);
                    for pos in 0..=255 {
                        let mut cpu = TestCpu::default();
                        cpu.mem.insert(moved.clone(), pos);
                        cpu.mem.insert(other.clone(), 42);
                        cpu.run(&code);
                        assert_eq!(
                            cpu.mem[&moved],
                            expected_move(dir, pos, distance, limit),
                            "move_{}_limit({}, {}) from {}",
                            name,
                            distance,
                            limit,
                            pos
                        );
                        assert_eq!(
                            cpu.mem[&other], 42,
                            "move_{}_limit moved the other axis",
                            name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_moves_can_be_emitted_many_times() {
        // B7: every copy emitted the same global labels (LeftLimit, LeftLimitStore,
        // LeftLimitEnd, Left, LeftEnd, ...), whatever the sprite
        let labels = LabelAllocator::new();
        let mut paddle = Sprite::new(0, 0, 0, 0, 0);
        let mut ball = Sprite::new(1, 0, 0, 0, 0);
        let mut code = Vec::new();
        for _ in 0..3 {
            code.extend(paddle.move_left_limit(&labels, 1, 16));
            code.extend(ball.move_down_limit(&labels, 2, 144));
            code.extend(ball.move_right(1));
        }
        assert_code_labels_ok(&code);

        // Each copy jumps within itself: all three run
        let mut cpu = TestCpu::default();
        cpu.mem.insert("_OAMRAM+1".to_string(), 18); // paddle X: 17, 16, then on the limit
        cpu.mem.insert("_OAMRAM+4".to_string(), 100); // ball Y: 102, 104, 106
        cpu.mem.insert("_OAMRAM+5".to_string(), 50); // ball X: 51, 52, 53
        cpu.run(&code);
        assert_eq!(cpu.mem["_OAMRAM+1"], 16);
        assert_eq!(cpu.mem["_OAMRAM+4"], 106);
        assert_eq!(cpu.mem["_OAMRAM+5"], 53);
    }

    #[test]
    fn test_moves_inside_an_if() {
        // B7: a global label inside the If body started a new label scope, so the If
        // could not find its own .end_if_N / .else_N
        let labels = LabelAllocator::new();
        let mut sprite = Sprite::new(0, 0, 0, 0, 0);
        let mut if_counter = 0;
        let mut code = Vec::new();
        for _ in 0..2 {
            let then_move = sprite.move_up_limit(&labels, 1, 16);
            let else_move = sprite.move_left(1);
            let mut if_stmt = If::lt(sprite.get_y(), sprite.get_x(), then_move).or_else(else_move);
            code.extend(if_stmt.emit(&mut if_counter));
        }
        assert_code_labels_ok(&code);
    }
}
