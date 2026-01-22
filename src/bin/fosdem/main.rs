use rust_boy::{
    gb_std::inputs::PadButton,
    rust_boy::{AnimationType, InputManager, RustBoy, TileSource},
};

fn main() {
    let mut gb = RustBoy::new();

    // Add 16x16 composite sprite (two 8x16 sprites side by side)
    let player = gb.add_sprite_16x16(
        "player",
        TileSource::from_file("char.2bpp", 64),
        TileSource::from_file("char-dx.2bpp", 64),
        80,
        72,
        0,
    );

    // Add looping animation to the composite sprite (applies to both halves)
    // Start with animation disabled
    gb.sprites
        .add_composite_animation(player, "playerWalk", 0, 6, AnimationType::Loop);
    gb.sprites
        .set_composite_animation_initial_state(player, "playerWalk", false);

    // Input handling
    let mut inputs = InputManager::new();
    inputs.on_press(
        PadButton::A,
        gb.sprites.enable_composite_animation(player, "playerWalk"),
    );
    inputs.on_press(
        PadButton::B,
        gb.sprites.disable_composite_animation(player, "playerWalk"),
    );
    inputs.on_press(
        PadButton::Left,
        gb.sprites.move_composite_left_limit(player, 1, 0),
    );
    inputs.on_press(
        PadButton::Right,
        gb.sprites.move_composite_right_limit(player, 1, 150),
    );
    inputs.on_press(
        PadButton::Up,
        gb.sprites.move_composite_up_limit(player, 1, 0),
    );
    inputs.on_press(
        PadButton::Down,
        gb.sprites.move_composite_down_limit(player, 1, 150),
    );
    gb.add_inputs(inputs);
    println!("{}", gb.build());
}
