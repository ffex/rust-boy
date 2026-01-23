use rust_boy::{
    gb_std::inputs::PadButton,
    rust_boy::{ANIM_DISABLED, AnimationType, InputManager, RustBoy, TileSource},
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

    // Add looping animations to the composite sprite (applies to both halves)
    // add_composite_animation returns the animation index
    // Animation order: front, back, left, right (frames 0-3, 4-7, 8-11, 12-15)
    let anim_walk_front =
        gb.sprites
            .add_composite_animation(player, "playerWalkFront", 0, 3, AnimationType::Loop);
    let anim_walk_back =
        gb.sprites
            .add_composite_animation(player, "playerWalkBack", 4, 7, AnimationType::Loop);
    let anim_walk_left =
        gb.sprites
            .add_composite_animation(player, "playerWalkLeft", 8, 11, AnimationType::Loop);
    let anim_walk_right =
        gb.sprites
            .add_composite_animation(player, "playerWalkRight", 12, 15, AnimationType::Loop);

    // Start with no animation (disabled)
    gb.sprites
        .set_composite_initial_animation(player, ANIM_DISABLED);

    // Input handling
    let mut inputs = InputManager::new();
    inputs.on_press(
        PadButton::Left,
        [
            gb.sprites.move_composite_left_limit(player, 1, 0),
            gb.sprites
                .enable_composite_animation(player, anim_walk_left),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Right,
        [
            gb.sprites.move_composite_right_limit(player, 1, 150),
            gb.sprites
                .enable_composite_animation(player, anim_walk_right),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Up,
        [
            gb.sprites.move_composite_up_limit(player, 1, 0),
            gb.sprites
                .enable_composite_animation(player, anim_walk_back),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Down,
        [
            gb.sprites.move_composite_down_limit(player, 1, 150),
            gb.sprites
                .enable_composite_animation(player, anim_walk_front),
        ]
        .concat(),
    );
    gb.add_inputs(inputs);
    println!("{}", gb.build());
}
