mod tilemap;
mod tiles;

use rust_boy::{
    gb_asm::{Asm, Operand, Register},
    gb_std::{
        flow::{Emittable, If},
        graphics::{
            sprites::{clear_objects_screen, initialize_objects_screen, SpriteManager},
            utility::{
                add_tilemap, add_tiles, cp_in_memory, get_tile_by_pixel, is_specific_tile, memcopy,
                turn_off_screen, turn_on_screen, wait_not_vblank, wait_vblank,
            },
        },
        inputs::{check_key, update_keys},
        utility::header_section,
        variables::VariableSection,
    },
};

fn main() {
    let mut asm = Asm::new();
    asm.include_hardware();
    asm.def("BRICK_LEFT", 0x05);
    asm.def("BRICK_RIGHT", 0x06);
    asm.def("BLANK_TILE", 0x08);
    asm.def("DIGIT_OFFSET", 0x1A);
    asm.def("SCORE_TENS", 0x9870);
    asm.def("SCORE_ONES", 0x9871);
    asm.emit_all(header_section());
    asm.label("EntryPoint");

    asm.call("WaitVBlank");
    asm.emit_all(turn_off_screen());

    // Tile management
    asm.chunk(rust_boy::gb_asm::Chunk::Tiles);
    asm.emit_all(add_tiles("Tiles", tiles::TILES));
    asm.emit_all(add_tiles("Ball", tiles::BALL));
    asm.emit_all(add_tiles("Paddle", tiles::PADDLE));

    asm.chunk(rust_boy::gb_asm::Chunk::Main);
    asm.emit_all(cp_in_memory("Tiles", "$9000"));
    asm.emit_all(cp_in_memory("Ball", "$8010"));
    asm.emit_all(cp_in_memory("Paddle", "$8000"));
    asm.emit_all(cp_in_memory("Tilemap", "$9800"));

    asm.emit_all(initialize_objects_screen());
    asm.emit_all(clear_objects_screen());

    // Sprite management
    let mut sprite_manager = SpriteManager::new();
    sprite_manager.add_sprite(16, 128, 0, 0); // Paddle (id 0)
    sprite_manager.add_sprite(32, 100, 1, 0); // Ball (id 1)
    asm.ld_a(1);
    asm.ld_addr_def_a("wBallMomentumX");
    asm.ld_a_label("-1");
    asm.ld_addr_def_a("wBallMomentumY");
    asm.emit_all(sprite_manager.draw());

    asm.emit_all(turn_on_screen());
    asm.ld_a(0b11100100);
    asm.ld_addr_def_a("rBGP");
    asm.ld_a(0b11100100);
    asm.ld_addr_def_a("rOBP0");

    asm.ld_a(0);
    asm.ld_addr_def_a("wFrameCounter");
    asm.ld_addr_def_a("wNewKeys");
    asm.ld_addr_def_a("wCurKeys");
    asm.ld_addr_def_a("wScore");

    // MAIN LOOP START
    asm.label("Main");
    asm.call("WaitNotVBlank");
    asm.call("WaitVBlank");

    // Ball movement
    asm.emit_all(
        sprite_manager
            .get_sprite_mut(1)
            .unwrap()
            .move_x_var("wBallMomentumX"),
    );
    asm.emit_all(
        sprite_manager
            .get_sprite_mut(1)
            .unwrap()
            .move_y_var("wBallMomentumY"),
    );

    // Bounce on top
    asm.label("BounceOnTop");
    asm.emit_all(sprite_manager.get_sprite(1).unwrap().get_pivot(0, 1));
    asm.call("GetTileByPixel");
    asm.ld_a_addr_reg(rust_boy::gb_asm::Register::HL);
    asm.call("IsWallTile");
    asm.jp_cond(rust_boy::gb_asm::Condition::NZ, "BounceOnTopEnd");
    asm.ld_a(1);
    asm.ld_addr_def_a("wBallMomentumY");
    asm.label("BounceOnTopEnd");

    // Bounce on right
    asm.label("BounceOnRight");
    asm.emit_all(sprite_manager.get_sprite(1).unwrap().get_pivot(-1, 0));
    asm.call("GetTileByPixel");
    asm.ld_a_addr_reg(rust_boy::gb_asm::Register::HL);
    asm.call("IsWallTile");
    asm.jp_cond(rust_boy::gb_asm::Condition::NZ, "BounceOnRightEnd");
    asm.ld_a_label("-1");
    asm.ld_addr_def_a("wBallMomentumX");
    asm.label("BounceOnRightEnd");

    // Bounce on left
    asm.label("BounceOnLeft");
    asm.emit_all(sprite_manager.get_sprite(1).unwrap().get_pivot(1, 0));
    asm.call("GetTileByPixel");
    asm.ld_a_addr_reg(rust_boy::gb_asm::Register::HL);
    asm.call("IsWallTile");
    asm.jp_cond(rust_boy::gb_asm::Condition::NZ, "BounceOnLeftEnd");
    asm.ld_a_label("1");
    asm.ld_addr_def_a("wBallMomentumX");
    asm.label("BounceOnLeftEnd");

    // Bounce on bottom
    asm.label("BounceOnBottom");
    asm.emit_all(sprite_manager.get_sprite(1).unwrap().get_pivot(0, -1));
    asm.call("GetTileByPixel");
    asm.ld_a_addr_reg(rust_boy::gb_asm::Register::HL);
    asm.call("IsWallTile");
    asm.jp_cond(rust_boy::gb_asm::Condition::NZ, "BounceOnBottomEnd");
    asm.ld_a_label("-1");
    asm.ld_addr_def_a("wBallMomentumY");
    asm.label("BounceOnBottomEnd");

    // Paddle bounce using the new simplified If API!
    asm.comment("Paddle bounce check");
    {
        let paddle = sprite_manager.get_sprite(0).unwrap();
        let ball = sprite_manager.get_sprite(1).unwrap();

        // Helper: get ball Y + 5 (for collision offset)
        let ball_y_plus_5 = {
            let mut a = Asm::new();
            a.emit_all(ball.get_y());
            a.add(Operand::Reg(Register::A), Operand::Imm(5));
            a.get_main_instrs()
        };

        // Helper: get paddle X - 8 (left edge)
        let paddle_x_minus_8 = {
            let mut a = Asm::new();
            a.emit_all(paddle.get_x());
            a.sub(Operand::Reg(Register::A), Operand::Imm(8));
            a.get_main_instrs()
        };

        // Helper: get paddle X + 16 (right edge)
        let paddle_x_plus_16 = {
            let mut a = Asm::new();
            a.emit_all(paddle.get_x());
            a.add(Operand::Reg(Register::A), Operand::Imm(8 + 16));
            a.get_main_instrs()
        };

        // Bounce body: set Y momentum to -1
        let bounce = {
            let mut a = Asm::new();
            a.ld_a_label("-1");
            a.ld_addr_def_a("wBallMomentumY");
            a.get_main_instrs()
        };

        // Nested if structure using the new clean API
        // Inner-most: paddle_x + 16 >= ball_x (ball within right bound)
        let inner_if = If::ge(paddle_x_plus_16, ball.get_x(), bounce);

        // Middle: paddle_x - 8 < ball_x (ball past left edge)
        let middle_if = If::lt(paddle_x_minus_8, ball.get_x(), inner_if);

        // Outer: ball_y + 5 == paddle_y (Y alignment)
        let mut paddle_bounce = If::eq(ball_y_plus_5, paddle.get_y(), middle_if);

        // Emit with counter management
        let mut counter = 0;
        asm.emit_all(paddle_bounce.emit(&mut counter));
    }
    asm.comment("PaddleBounceDone");

    asm.call("UpdateKeys");

    // Input handling
    let left_pressed = sprite_manager
        .get_sprite_mut(0)
        .unwrap()
        .move_left_limit(1, 15);
    let right_pressed = sprite_manager
        .get_sprite_mut(0)
        .unwrap()
        .move_right_limit(1, 105);
    asm.emit_all(check_key(
        rust_boy::gb_std::inputs::PadButton::Left,
        left_pressed,
    ));
    asm.emit_all(check_key(
        rust_boy::gb_std::inputs::PadButton::Right,
        right_pressed,
    ));

    asm.jp("Main");

    // Variables management
    asm.chunk(rust_boy::gb_asm::Chunk::Data);
    let mut counter_sec = VariableSection::new("Counter", "WRAM0");
    let mut input_vars_sec = VariableSection::new("Input Variables", "WRAM0");
    let mut ball_data_sec = VariableSection::new("Ball Data", "WRAM0");
    let mut score_sec = VariableSection::new("Score", "WRAM0");

    counter_sec.add_data("wFrameCounter", "db");
    input_vars_sec.add_data("wCurKeys", "db");
    input_vars_sec.add_data("wNewKeys", "db");
    ball_data_sec.add_data("wBallMomentumX", "db");
    ball_data_sec.add_data("wBallMomentumY", "db");
    score_sec.add_data("wScore", "db");

    asm.emit_all(counter_sec.generate());
    asm.emit_all(input_vars_sec.generate());
    asm.emit_all(ball_data_sec.generate());
    asm.emit_all(score_sec.generate());

    // Function Management
    asm.chunk(rust_boy::gb_asm::Chunk::Functions);
    asm.emit_all(memcopy());
    asm.emit_all(update_keys());
    asm.emit_all(wait_vblank());
    asm.emit_all(wait_not_vblank());
    asm.emit_all(get_tile_by_pixel());
    asm.emit_all(is_specific_tile(
        "IsWallTile",
        &["$00", "$01", "$02", "$04", "$05", "$06", "$07"],
    ));

    // Tilemap Management
    asm.chunk(rust_boy::gb_asm::Chunk::Tilemap);
    asm.emit_all(add_tilemap("Tilemap", tilemap::TILEMAP));

    println!("{}", asm.to_asm());
}
