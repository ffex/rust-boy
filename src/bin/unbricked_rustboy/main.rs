//! Unbricked game using the RustBoy high-level API
//!
//! This example demonstrates how RustBoy hides assembly details
//! while still allowing escape hatches for advanced features.

mod tilemap;
mod tiles;

use rust_boy::{
    gb_asm::{Asm, Condition, Operand, Register},
    gb_std::{
        flow::{ComparisonOp, ConditionOperand, If, IfCondition},
        graphics::utility::{get_tile_by_pixel, is_specific_tile},
        inputs::{PadButton, check_key, update_keys},
    },
    rust_boy::{BuiltinFunction, RustBoy, TileSource},
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
    // MAIN LOOP - Game logic
    // ========================================

    // Ball movement
    gb.add_to_main_loop(gb.sprites.move_x_var(ball, "wBallMomentumX"));
    gb.add_to_main_loop(gb.sprites.move_y_var(ball, "wBallMomentumY"));

    // Bounce on top
    {
        let mut bounce_top = Asm::new();
        bounce_top.label("BounceOnTop");
        bounce_top.emit_all(gb.sprites.get_pivot(ball, 0, 1));
        bounce_top.call("GetTileByPixel");
        bounce_top.ld_a_addr_reg(Register::HL);
        bounce_top.call("IsWallTile");
        bounce_top.jp_cond(Condition::NZ, "BounceOnTopEnd");
        bounce_top.ld_a(1);
        bounce_top.ld_addr_def_a("wBallMomentumY");
        bounce_top.label("BounceOnTopEnd");
        gb.add_to_main_loop(bounce_top.get_main_instrs());
    }

    // Bounce on right
    {
        let mut bounce_right = Asm::new();
        bounce_right.label("BounceOnRight");
        bounce_right.emit_all(gb.sprites.get_pivot(ball, -1, 0));
        bounce_right.call("GetTileByPixel");
        bounce_right.ld_a_addr_reg(Register::HL);
        bounce_right.call("IsWallTile");
        bounce_right.jp_cond(Condition::NZ, "BounceOnRightEnd");
        bounce_right.ld_a_label("-1");
        bounce_right.ld_addr_def_a("wBallMomentumX");
        bounce_right.label("BounceOnRightEnd");
        gb.add_to_main_loop(bounce_right.get_main_instrs());
    }

    // Bounce on left
    {
        let mut bounce_left = Asm::new();
        bounce_left.label("BounceOnLeft");
        bounce_left.emit_all(gb.sprites.get_pivot(ball, 1, 0));
        bounce_left.call("GetTileByPixel");
        bounce_left.ld_a_addr_reg(Register::HL);
        bounce_left.call("IsWallTile");
        bounce_left.jp_cond(Condition::NZ, "BounceOnLeftEnd");
        bounce_left.ld_a_label("1");
        bounce_left.ld_addr_def_a("wBallMomentumX");
        bounce_left.label("BounceOnLeftEnd");
        gb.add_to_main_loop(bounce_left.get_main_instrs());
    }

    // Bounce on bottom
    {
        let mut bounce_bottom = Asm::new();
        bounce_bottom.label("BounceOnBottom");
        bounce_bottom.emit_all(gb.sprites.get_pivot(ball, 0, -1));
        bounce_bottom.call("GetTileByPixel");
        bounce_bottom.ld_a_addr_reg(Register::HL);
        bounce_bottom.call("IsWallTile");
        bounce_bottom.jp_cond(Condition::NZ, "BounceOnBottomEnd");
        bounce_bottom.ld_a_label("-1");
        bounce_bottom.ld_addr_def_a("wBallMomentumY");
        bounce_bottom.label("BounceOnBottomEnd");
        gb.add_to_main_loop(bounce_bottom.get_main_instrs());
    }

    // Paddle bounce (nested if statements)
    {
        let mut paddle_bounce = Asm::new();
        paddle_bounce.comment("Paddle bounce check");
        paddle_bounce.emit_all(gb.sprites.get_y(paddle, Register::B));
        paddle_bounce.emit_all(gb.sprites.get_y(ball, Register::A));
        paddle_bounce.add(Operand::Reg(Register::A), Operand::Imm(5));

        // Nested if: check Y position
        let if_ball_y_check = If::new(
            IfCondition::new(
                ConditionOperand::Register(Register::A),
                ConditionOperand::Register(Register::B),
                ComparisonOp::E,
            ),
            {
                let mut bounce_x_check = Asm::new();
                bounce_x_check.emit_all(gb.sprites.get_x(ball, Register::B));
                bounce_x_check.emit_all(gb.sprites.get_x(paddle, Register::A));
                bounce_x_check.sub(Operand::Reg(Register::A), Operand::Imm(8));

                let if_ball_x_check = If::new(
                    IfCondition::new(
                        ConditionOperand::Register(Register::A),
                        ConditionOperand::Register(Register::B),
                        ComparisonOp::LT,
                    ),
                    {
                        let mut bounce_x_check_2 = Asm::new();
                        bounce_x_check_2.add(Operand::Reg(Register::A), Operand::Imm(8 + 16));

                        let if_ball_x_check_2 = If::new(
                            IfCondition::new(
                                ConditionOperand::Register(Register::A),
                                ConditionOperand::Register(Register::B),
                                ComparisonOp::GE,
                            ),
                            {
                                let mut bounce = Asm::new();
                                bounce.ld_a_label("-1");
                                bounce.ld_addr_def_a("wBallMomentumY");
                                bounce.get_main_instrs()
                            },
                        )
                        .with_label_counter(2);

                        bounce_x_check_2.emit_all(if_ball_x_check_2.emit_to());
                        bounce_x_check_2.get_main_instrs()
                    },
                )
                .with_label_counter(1);

                bounce_x_check.emit_all(if_ball_x_check.emit_to());
                bounce_x_check.get_main_instrs()
            },
        )
        .with_label_counter(0);

        paddle_bounce.emit_all(if_ball_y_check.emit_to());
        paddle_bounce.comment("Paddle bounce done");

        gb.add_to_main_loop(paddle_bounce.get_main_instrs());
    }

    // Input handling
    {
        let mut input_code = Asm::new();
        input_code.call("UpdateKeys");

        let left_pressed = gb.sprites.move_left_limit(paddle, 1, 15);
        let right_pressed = gb.sprites.move_right_limit(paddle, 1, 105);

        input_code.emit_all(check_key(PadButton::Left, left_pressed));
        input_code.emit_all(check_key(PadButton::Right, right_pressed));

        gb.add_to_main_loop(input_code.get_main_instrs());
    }

    // Mark functions as used (for auto-include)
    gb.use_function(BuiltinFunction::UpdateKeys);
    gb.use_function(BuiltinFunction::GetTileByPixel);

    // Add custom IsWallTile function using raw()
    gb.raw(|asm| {
        asm.chunk(rust_boy::gb_asm::Chunk::Functions);
        asm.emit_all(is_specific_tile(
            "IsWallTile",
            &["$00", "$01", "$02", "$04", "$05", "$06", "$07"],
        ));
    });

    // ========================================
    // BUILD AND OUTPUT
    // ========================================
    println!("{}", gb.build());
}
