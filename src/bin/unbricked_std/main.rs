mod tilemap;
mod tiles;

use rust_boy::{
    gb_asm::{Asm, Block, Expr, R8, Section},
    gb_std::{
        flow::If,
        graphics::{
            sprites::{SpriteManager, clear_objects_screen, initialize_objects_screen},
            tile_ref::TileRef,
            utility::{
                add_tilemap, add_tiles, cp_in_memory, get_tile_by_pixel, is_specific_tile, memcopy,
                turn_off_screen, turn_on_screen, wait_not_vblank, wait_vblank,
            },
        },
        inputs::{check_key, update_keys},
        utility::header_section,
        variables::VariableSection,
    },
    hw,
};

fn main() {
    let mut asm = Asm::new();
    asm.include_hardware();
    asm.def("BRICK_LEFT", 0x05);
    asm.def("BRICK_RIGHT", 0x06);
    asm.def("BLANK_TILE", 0x08);
    asm.def("DIGIT_OFFSET", 0x1A);
    // The score digits, on the map at row 3, columns 16 and 17
    asm.def("SCORE_TENS", TileRef::from_xy(16, 3).tilemap_addr);
    asm.def("SCORE_ONES", TileRef::from_xy(17, 3).tilemap_addr);
    asm.emit_all(header_section());
    asm.label("EntryPoint");

    asm.call("WaitVBlank");
    asm.emit_all(turn_off_screen());

    // Tile management: the tile data goes after the functions, below
    let mut tile_data = Block::new();
    tile_data.emit_all(add_tiles("Tiles", tiles::TILES));
    tile_data.emit_all(add_tiles("Ball", tiles::BALL));
    tile_data.emit_all(add_tiles("Paddle", tiles::PADDLE));

    // Background tiles at $9000, the paddle (sprite tile 0) and the ball (tile 1) at $8000
    let sprite_tile = |index: u16| Expr::hex(hw::VRAM8000.value + index * hw::TILE_SIZE);
    asm.emit_all(cp_in_memory("Tiles", Expr::hex(hw::VRAM9000.value)));
    asm.emit_all(cp_in_memory("Ball", sprite_tile(1)));
    asm.emit_all(cp_in_memory("Paddle", sprite_tile(0)));
    asm.emit_all(cp_in_memory("Tilemap", Expr::hex(hw::SCRN0.value)));

    asm.emit_all(initialize_objects_screen());
    asm.emit_all(clear_objects_screen(asm.labels()));

    // Sprite management
    let mut sprite_manager = SpriteManager::new();
    sprite_manager.add_sprite(16, 128, 0, 0); // Paddle (id 0)
    sprite_manager.add_sprite(32, 100, 1, 0); // Ball (id 1)
    asm.ld_a(1);
    asm.ld_addr_def_a("wBallMomentumX");
    asm.ld(R8::A, -1);
    asm.ld_addr_def_a("wBallMomentumY");
    asm.emit_all(sprite_manager.draw());

    asm.emit_all(turn_on_screen());
    asm.ld_a(0b11100100);
    asm.ld_addr_def_a(hw::BGP);
    asm.ld_a(0b11100100);
    asm.ld_addr_def_a(hw::OBP0);

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

    // Bounce on top. GetTileByPixel returns the tile index in a, which IsWallTile tests
    asm.label("BounceOnTop");
    asm.emit_all(sprite_manager.get_sprite(1).unwrap().get_pivot(0, 1));
    asm.call("GetTileByPixel");
    asm.call("IsWallTile");
    asm.jp_cond(rust_boy::gb_asm::Condition::NZ, "BounceOnTopEnd");
    asm.ld_a(1);
    asm.ld_addr_def_a("wBallMomentumY");
    asm.label("BounceOnTopEnd");

    // Bounce on right
    asm.label("BounceOnRight");
    asm.emit_all(sprite_manager.get_sprite(1).unwrap().get_pivot(-1, 0));
    asm.call("GetTileByPixel");
    asm.call("IsWallTile");
    asm.jp_cond(rust_boy::gb_asm::Condition::NZ, "BounceOnRightEnd");
    asm.ld(R8::A, -1);
    asm.ld_addr_def_a("wBallMomentumX");
    asm.label("BounceOnRightEnd");

    // Bounce on left
    asm.label("BounceOnLeft");
    asm.emit_all(sprite_manager.get_sprite(1).unwrap().get_pivot(1, 0));
    asm.call("GetTileByPixel");
    asm.call("IsWallTile");
    asm.jp_cond(rust_boy::gb_asm::Condition::NZ, "BounceOnLeftEnd");
    asm.ld_a(1);
    asm.ld_addr_def_a("wBallMomentumX");
    asm.label("BounceOnLeftEnd");

    // Bounce on bottom
    asm.label("BounceOnBottom");
    asm.emit_all(sprite_manager.get_sprite(1).unwrap().get_pivot(0, -1));
    asm.call("GetTileByPixel");
    asm.call("IsWallTile");
    asm.jp_cond(rust_boy::gb_asm::Condition::NZ, "BounceOnBottomEnd");
    asm.ld(R8::A, -1);
    asm.ld_addr_def_a("wBallMomentumY");
    asm.label("BounceOnBottomEnd");

    // Paddle bounce using the new simplified If API!
    asm.comment("Paddle bounce check");
    {
        let paddle = sprite_manager.get_sprite(0).unwrap();
        let ball = sprite_manager.get_sprite(1).unwrap();

        // Helper: get ball Y + 5 (for collision offset)
        let ball_y_plus_5 = {
            let mut a = Block::new();
            a.emit_all(ball.get_y());
            a.add(5);
            a.into_instrs()
        };

        // Helper: get paddle X - 8 (left edge)
        let paddle_x_minus_8 = {
            let mut a = Block::new();
            a.emit_all(paddle.get_x());
            a.sub(8);
            a.into_instrs()
        };

        // Helper: get paddle X + 16 (right edge)
        let paddle_x_plus_16 = {
            let mut a = Block::new();
            a.emit_all(paddle.get_x());
            a.add(16);
            a.into_instrs()
        };

        // Bounce body: set Y momentum to -1
        let bounce = {
            let mut a = Block::new();
            a.ld(R8::A, -1);
            a.ld_addr_def_a("wBallMomentumY");
            a.into_instrs()
        };

        // Nested if structure using the new clean API
        // Inner-most: paddle_x + 16 >= ball_x (ball within right bound)
        let inner_if = If::ge(paddle_x_plus_16, ball.get_x(), bounce);

        // Middle: paddle_x - 8 < ball_x (ball past left edge)
        let middle_if = If::lt(paddle_x_minus_8, ball.get_x(), inner_if);

        // Outer: ball_y + 5 == paddle_y (Y alignment)
        let paddle_bounce = If::eq(ball_y_plus_5, paddle.get_y(), middle_if);

        // Its labels come from the program's allocator
        asm.emit_code(paddle_bounce);
    }
    asm.comment("PaddleBounceDone");

    asm.call("UpdateKeys");

    // Input handling: the paddle stays between the walls, at OAM X 16 to 104 (limits included).
    // Key checks and limited moves take their local labels from the program's allocator,
    // like the Ifs
    let labels = asm.labels().clone();
    let left_pressed = sprite_manager
        .get_sprite_mut(0)
        .unwrap()
        .move_left_limit(&labels, 1, 16);
    let right_pressed = sprite_manager
        .get_sprite_mut(0)
        .unwrap()
        .move_right_limit(&labels, 1, 104);
    asm.emit_all(check_key(
        &labels,
        rust_boy::gb_std::inputs::PadButton::Left,
        left_pressed,
    ));
    asm.emit_all(check_key(
        &labels,
        rust_boy::gb_std::inputs::PadButton::Right,
        right_pressed,
    ));

    asm.jp("Main");
    asm.blank_line();

    // Variables management: their WRAM0 sections go last, below
    let mut counter_sec = VariableSection::new(Section::wram0("Counter"));
    let mut input_vars_sec = VariableSection::new(Section::wram0("Input Variables"));
    let mut ball_data_sec = VariableSection::new(Section::wram0("Ball Data"));
    let mut score_sec = VariableSection::new(Section::wram0("Score"));

    counter_sec.add_data("wFrameCounter", "db");
    input_vars_sec.add_data("wCurKeys", "db");
    input_vars_sec.add_data("wNewKeys", "db");
    ball_data_sec.add_data("wBallMomentumX", "db");
    ball_data_sec.add_data("wBallMomentumY", "db");
    score_sec.add_data("wScore", "db");

    let mut variables = Block::new();
    variables.emit_all(counter_sec.generate());
    variables.emit_all(input_vars_sec.generate());
    variables.emit_all(ball_data_sec.generate());
    variables.emit_all(score_sec.generate());

    // Function Management
    asm.emit_all(memcopy());
    asm.emit_all(update_keys());
    asm.emit_all(wait_vblank());
    asm.emit_all(wait_not_vblank());
    asm.emit_all(get_tile_by_pixel());
    asm.emit_all(is_specific_tile(
        "IsWallTile",
        &["$00", "$01", "$02", "$04", "$05", "$06", "$07"],
    ));

    asm.blank_line();

    // Then the tile data, the tilemap and the variables, still in that order
    asm.emit_all(tile_data).blank_line();
    asm.emit_all(add_tilemap("Tilemap", tilemap::TILEMAP));
    asm.blank_line();
    asm.emit_all(variables);

    println!("{}", asm.to_asm());
}
