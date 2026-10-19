mod inputs;

use rust_boy::engine::{
    ANIM_DISABLED, AnimationType, PngOptions, RustBoy, RustBoyConfig, SpriteSize, TileOrder,
    TileSource,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut gb = RustBoy::with_config(RustBoyConfig::default().sprite_size(SpriteSize::Size8x16)); //TODO check: This can be done more simple?

    let player = gb.add_sprite_16x16(
        "player",
        TileSource::from_png(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/olive-empire/char-l.edited.png"
            ),
            &PngOptions::default().tile_order(TileOrder::Sprite8x16),
        )?,
        TileSource::from_png(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/olive-empire/char-r.edited.png"
            ),
            &PngOptions::default().tile_order(TileOrder::Sprite8x16),
        )?,
        80,
        72,
        0,
    );

    //Animation
    //
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

    gb.sprites
        .set_composite_initial_animation(player, ANIM_DISABLED);

    let walk = inputs::WalkAnimations {
        front: anim_walk_front,
        back: anim_walk_back,
        left: anim_walk_left,
        right: anim_walk_right,
    };
    let inputs = inputs::player_inputs(&gb.sprites, player, &walk);
    gb.add_inputs(inputs);

    println!("{}", gb.build()?);
    Ok(())
}
