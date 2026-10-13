use rust_boy::{
    engine::{
        ANIM_DISABLED, AnimationType, InputManager, PngOptions, RustBoy, RustBoyConfig, SpriteSize,
        TileOrder, TileSource,
    },
    stdlib::inputs::PadButton,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The 16x16 character is made of 8x16 sprites (the default is 8x8)
    let mut gb = RustBoy::with_config(RustBoyConfig::default().sprite_size(SpriteSize::Size8x16));

    // Add 16x16 composite sprite (two 8x16 sprites side by side)
    let player = gb.add_sprite_16x16(
        "player",
        TileSource::from_2bpp(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/fosdem/char.2bpp"
        ))?,
        TileSource::from_png(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/fosdem/char-dx.png"),
            &PngOptions::new([[255; 3], [207; 3], [104; 3], [0, 0, 1]])
                .tile_order(TileOrder::Sprite8x16),
        )?,
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

    // Input handling. Limits are OAM coordinates (screen x + 8, screen y + 16), included;
    // the player moves as one block and its leading half stops on the limit
    let mut inputs = InputManager::new();
    inputs.on_press(
        PadButton::Left,
        [
            gb.sprites.move_composite_left_limit(player, 1, 1),
            gb.sprites
                .enable_composite_animation(player, anim_walk_left),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Right,
        [
            gb.sprites.move_composite_right_limit(player, 1, 149),
            gb.sprites
                .enable_composite_animation(player, anim_walk_right),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Up,
        [
            gb.sprites.move_composite_up_limit(player, 1, 1),
            gb.sprites
                .enable_composite_animation(player, anim_walk_back),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Down,
        [
            gb.sprites.move_composite_down_limit(player, 1, 149),
            gb.sprites
                .enable_composite_animation(player, anim_walk_front),
        ]
        .concat(),
    );
    gb.add_inputs(inputs);
    println!("{}", gb.build()?);
    Ok(())
}
