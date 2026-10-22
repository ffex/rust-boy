mod inputs;

use rust_boy::engine::{
    ANIM_DISABLED, AnimationType, Hitbox, PngOptions, RustBoy, RustBoyConfig, SpriteSize,
    TileCollision, TileOrder, TileSource,
};

/// One olive tree (2x2 tiles) every `TREE_STEP` tiles across and down: a 2-tile path between trees
const TREE_STEP: usize = 4;

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

    // Background: the grass tile over the whole $9800 map, and olive trees on it
    let grass = gb.tiles.add_background(
        "BgTile",
        TileSource::from_png(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/olive-empire/background.png"
            ),
            &PngOptions::default(),
        )?,
    );
    let tree = gb.tiles.add_background(
        "OliveTree",
        TileSource::from_png(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/olive-empire/olive-tree.png"
            ),
            &PngOptions::default(),
        )?,
    );
    let grass_tile = gb.tiles.background_tile_index(grass);
    // The tree's 4 tiles, row by row: top-left, top-right, bottom-left, bottom-right
    let tree_tile = gb.tiles.background_tile_index(tree);
    gb.tiles
        .add_tilemap("BgMap", &olive_grove(grass_tile, tree_tile));

    // Obstacles: the trunk row of the trees stops the player's feet; the leaves do not
    let trunks = TileCollision::new([tree_tile + 2, tree_tile + 3]);
    let feet = Hitbox::new(2, 8, 12, 8);

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
    let inputs = inputs::player_inputs(&gb.sprites, player, &walk, &trunks, feet);
    gb.add_inputs(inputs);

    println!("{}", gb.build()?);
    Ok(())
}

/// The map: grass, and a tree every `TREE_STEP` tiles from the top-left corner
fn olive_grove(grass: u8, tree: u8) -> [[u8; 32]; 32] {
    let mut map = [[grass; 32]; 32];
    for row in (0..32).step_by(TREE_STEP) {
        for column in (0..32).step_by(TREE_STEP) {
            map[row][column] = tree;
            map[row][column + 1] = tree + 1;
            map[row + 1][column] = tree + 2;
            map[row + 1][column + 1] = tree + 3;
        }
    }
    map
}
