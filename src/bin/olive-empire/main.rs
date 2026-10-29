mod inputs;

use rust_boy::hw;

use rust_boy::engine::{
    ANIM_DISABLED, AnimationType, Hitbox, NumberFormat, PngOptions, RustBoy, RustBoyConfig,
    SpriteSize, TileCollision, TileOrder, TileSource, TilemapArea, Window,
};

/// One olive tree (2x2 tiles) every `TREE_STEP` tiles across and down: a 2-tile path between trees
const TREE_STEP: usize = 4;
/// Height in pixels of the HUD strip at the bottom of the screen (the window, 2 tile rows)
pub const HUD_HEIGHT: u8 = 16;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The HUD is the window, on the $9C00 map, over the bottom `HUD_HEIGHT` lines of the screen
    let mut gb = RustBoy::with_config(
        RustBoyConfig::default()
            .sprite_size(SpriteSize::Size8x16)
            .window(Window::new(
                TilemapArea::Map9C00,
                0,
                hw::SCRN_Y.value - HUD_HEIGHT,
            )),
    );

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
    let full_tree = gb.tiles.add_background(
        "OliveTreeFull",
        TileSource::from_png(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/olive-empire/olive-tree-olive.png"
            ),
            &PngOptions::default(),
        )?,
    );
    let grass_tile = gb.tiles.background_tile_index(grass);
    // Each tree's 4 tiles, row by row: top-left, top-right, bottom-left, bottom-right
    let bare_tile = gb.tiles.background_tile_index(tree);
    let full_tile = gb.tiles.background_tile_index(full_tree);
    // Every tree starts with olives
    gb.tiles
        .add_tilemap("BgMap", &olive_grove(grass_tile, full_tile));

    // HUD: the olive icon (2x2 tiles) at columns 1-2 and the harvest counter on its lower row,
    // on a blank (colour 0) strip
    let hud_blank = gb
        .tiles
        .add_background("HudBlank", TileSource::from_2bpp_bytes(&[0; 16])?);
    let hud_olive = gb.tiles.add_background(
        "HudOlive",
        TileSource::from_png(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/olive-empire/olive-hud.edited.png"
            ),
            &PngOptions::default(),
        )?,
    );
    let digits = gb.load_font(
        "Digits",
        TileSource::from_png(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/olive-empire/digits.png"
            ),
            &PngOptions::default(),
        )?,
        b'0',
    );
    let blank_tile = gb.tiles.background_tile_index(hud_blank);
    let olive_tile = gb.tiles.background_tile_index(hud_olive);
    let mut hud = [[blank_tile; 32]; 2];
    hud[0][1] = olive_tile;
    hud[0][2] = olive_tile + 1;
    hud[1][1] = olive_tile + 2;
    hud[1][2] = olive_tile + 3;
    gb.tiles.add_tilemap_at("Hud", TilemapArea::Map9C00, &hud);
    let olives = gb.create_number_printer(
        "Olives",
        digits,
        TilemapArea::Map9C00,
        4,
        1,
        NumberFormat::DecimalU16,
    );
    gb.add_to_main_loop(olives.step());

    // Obstacles: the trunk row of the trees, with or without olives, stops the player's feet;
    // the leaves do not
    let trunks = TileCollision::new([full_tile + 2, full_tile + 3, bare_tile + 2, bare_tile + 3]);
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
