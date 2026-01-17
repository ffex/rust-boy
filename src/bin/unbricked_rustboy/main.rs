//! Unbricked game using the RustBoy high-level API
//!
//! This example demonstrates how RustBoy hides assembly details
//! while still allowing escape hatches for advanced features.

mod tilemap;
mod tiles;

use rust_boy::{
    gb_asm::Asm,
    gb_std::{
        flow::{Call, If, IfA, IfCall, IfConst, InstrOps, boxed},
        graphics::{tile_ref::TileRef, utility::is_specific_tile},
        inputs::PadButton,
    },
    rust_boy::{InputManager, RustBoy, TileSource},
};

fn main() {
    let mut gb = RustBoy::new();

    // ========================================
    // CONSTANTS - No more manual DEF statements!
    // ========================================
    gb.define_const("BRICK_LEFT", "0x05")
        .define_const("BRICK_RIGHT", "0x06")
        .define_const("BLANK_TILE", "0x08")
        .define_const("DIGIT_OFFSET", "0x1A")
        .define_const_hex("SCORE_TENS", 0x9870)
        .define_const_hex("SCORE_ONES", 0x9871);

    // ========================================
    // TILES - Auto VRAM allocation!
    // ========================================
    // Background tiles go to $9000
    gb.tiles
        .add_background("Tiles", TileSource::from_raw(tiles::TILES));

    // Tilemap goes to $9800
    gb.tiles.add_tilemap("Tilemap", tilemap::TILEMAP);

    // Sprites: tile + position + OAM in one call!
    let paddle = gb.add_sprite("Paddle", TileSource::from_raw(tiles::PADDLE), 16, 128, 0);
    let ball = gb.add_sprite("Ball", TileSource::from_raw(tiles::BALL), 32, 100, 0);

    // ========================================
    // VARIABLES - Auto WRAM allocation!
    // ========================================
    let _frame_counter = gb.vars.create_u8("wFrameCounter", 0);
    let _cur_keys = gb.vars.create_u8("wCurKeys", 0);
    let _new_keys = gb.vars.create_u8("wNewKeys", 0);
    let _ball_momentum_x = gb.vars.create_i8("wBallMomentumX", 1);
    let _ball_momentum_y = gb.vars.create_i8("wBallMomentumY", -1i8);
    let _score = gb.vars.create_u8("wScore", 0);

    // ========================================
    // FUNCTIONS - Auto WRAM allocation!
    // ========================================
    gb.define_function(
        "IsWallTile",
        is_specific_tile(
            "IsWallTile",
            &["$00", "$01", "$02", "$04", "$05", "$06", "$07"],
        ),
    );
    gb.define_function_from(
        "CheckAndHandleBrick",
        vec![
            boxed(IfConst::eq(
                Call::with_args("GetTileByPixel", gb.sprites.get_pivot(ball, 0, 1)),
                "BRICK_LEFT",
                vec![
                    TileRef::set_tile_label("BLANK_TILE"),
                    TileRef::next_tile(),
                    TileRef::set_tile_label("BLANK_TILE"),
                ],
            )),
            boxed(IfA::eq(
                "BRICK_RIGHT",
                vec![
                    TileRef::set_tile_label("BLANK_TILE"),
                    TileRef::prev_tile(),
                    TileRef::set_tile_label("BLANK_TILE"),
                ],
            )),
        ],
    );
    // ========================================
    // MAIN LOOP - Game logic
    // ========================================

    // Ball movement
    gb.add_to_main_loop(gb.sprites.move_x_var(ball, "wBallMomentumX"));
    gb.add_to_main_loop(gb.sprites.move_y_var(ball, "wBallMomentumY"));

    // Bounce on top
    gb.call_args("GetTileByPixel", gb.sprites.get_pivot(ball, 0, 1));
    gb.add_to_main_loop(IfCall::is_true(
        "IsWallTile",
        vec![
            boxed(Call::new("CheckAndHandleBrick")),
            boxed(_ball_momentum_y.set(1)),
        ],
    ));

    // Bounce on right
    gb.call_args("GetTileByPixel", gb.sprites.get_pivot(ball, -1, 0));
    gb.add_to_main_loop(IfCall::is_true("IsWallTile", _ball_momentum_x.set(-1)));

    // Bounce on left
    gb.call_args("GetTileByPixel", gb.sprites.get_pivot(ball, 1, 0));
    gb.add_to_main_loop(IfCall::is_true("IsWallTile", _ball_momentum_x.set(1)));

    // Bounce on bottom
    gb.call_args("GetTileByPixel", gb.sprites.get_pivot(ball, 0, -1));
    gb.add_to_main_loop(IfCall::is_true("IsWallTile", _ball_momentum_y.set(-1)));
    gb.add_to_main_loop({
        // make a debug label in as API
        let mut lbl_debug = Asm::new();
        lbl_debug.label("PaddleBounce");
        lbl_debug.get_main_instrs()
    });
    // Paddle bounce
    let paddle_bounce = If::eq(
        gb.sprites.get_y(paddle),
        gb.sprites.get_y(ball).plus(5),
        If::lt(
            gb.sprites.get_x(ball),
            gb.sprites.get_x(paddle).minus(8),
            If::ge(gb.sprites.get_x(ball), gb.sprites.get_x(paddle).plus(16), {
                _ball_momentum_y.set(-1)
            }),
        ),
    );
    gb.add_to_main_loop(paddle_bounce);
    gb.add_to_main_loop({
        // make a debug label in as API
        let mut lbl_debug = Asm::new();
        lbl_debug.label("PaddleBounceEND");
        lbl_debug.get_main_instrs()
    });
    // Input handling
    let mut inputs = InputManager::new();
    inputs.on_press(PadButton::Left, gb.sprites.move_left_limit(paddle, 1, 15));
    inputs.on_press(
        PadButton::Right,
        gb.sprites.move_right_limit(paddle, 1, 105),
    );
    gb.add_inputs(inputs);

    // ========================================
    // BUILD AND OUTPUT
    // ========================================
    println!("{}", gb.build());
}
