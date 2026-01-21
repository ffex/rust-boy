use rust_boy::{
    gb_std::inputs::PadButton,
    rust_boy::{AnimationType, InputManager, RustBoy, TileSource},
};

fn main() {
    let mut gb = RustBoy::new();

    // Add sprite with animation tiles (5 frames: 0-4)
    let player = gb.add_sprite("player", TileSource::from_file("char.2bpp", 64), 80, 72, 0);
    let player_dx = gb.add_sprite(
        "playerDx",
        TileSource::from_file("char-dx.2bpp", 64),
        88,
        72,
        0,
    );

    // Add looping animation with relative frame indices 0 to 6
    gb.sprites
        .add_animation(player, "playerWalk", 0, 4, AnimationType::Loop);
    gb.sprites
        .add_animation(player_dx, "playerWalkDx", 0, 4, AnimationType::Loop);
    gb.sprites.disable_animation("playerWalk");
    gb.sprites.disable_animation("playerWalkDx");
    // Input handling
    let mut inputs = InputManager::new();
    inputs.on_press(
        PadButton::A,
        [
            gb.sprites.enable_animation("playerWalk"),
            gb.sprites.enable_animation("playerWalkDx"),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::B,
        [
            gb.sprites.disable_animation("playerWalk"),
            gb.sprites.disable_animation("playerWalkDx"),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Left,
        [
            gb.sprites.move_left_limit(player, 1, 0),
            gb.sprites.move_left_limit(player_dx, 1, 0),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Right,
        [
            gb.sprites.move_right_limit(player, 1, 150),
            gb.sprites.move_right_limit(player_dx, 1, 150),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Up,
        [
            gb.sprites.move_up_limit(player, 1, 0),
            gb.sprites.move_up_limit(player_dx, 1, 0),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Down,
        [
            gb.sprites.move_down_limit(player, 1, 150),
            gb.sprites.move_down_limit(player_dx, 1, 150),
        ]
        .concat(),
    );
    gb.add_inputs(inputs);
    println!("{}", gb.build());
}
