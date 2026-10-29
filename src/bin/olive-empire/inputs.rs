use rust_boy::{
    asm::Instr,
    engine::{CompositeSpriteId, Hitbox, InputManager, SpriteManager, TileCollision},
    hw,
    stdlib::inputs::PadButton,
};

// The screen edges as OAM limits (screen x + 8, screen y + 16), for the sprite that leads the
// move: the leftmost / topmost one at screen 0, the rightmost one at screen x 152 (its last
// pixel on 159), the lowest 8x16 one on the line above the HUD
const LEFT_LIMIT: u8 = hw::OAM_X_OFFSET;
const RIGHT_LIMIT: u8 = hw::SCRN_X.value - 8 + hw::OAM_X_OFFSET;
const TOP_LIMIT: u8 = hw::OAM_Y_OFFSET;
const BOTTOM_LIMIT: u8 = hw::SCRN_Y.value - crate::HUD_HEIGHT - 16 + hw::OAM_Y_OFFSET;

/// Indices of the player's walk animations, as `add_composite_animation` returned them
pub struct WalkAnimations {
    pub front: u8,
    pub back: u8,
    pub left: u8,
    pub right: u8,
}
// TODO can be do better? in RUST there is the class concept? or an interface to implement events?
/// D-pad: move the player 1 px per frame and play the walk animation of that direction;
/// the player does not move when its `feet` would step on a `solid` tile.
/// The player stays whole on the screen: it moves as one block and its leading half stops on
/// the screen edge. On release, the walk stops on its
/// first frame (the standing pose of that direction)
pub fn player_inputs(
    sprites: &SpriteManager,
    player: CompositeSpriteId,
    walk: &WalkAnimations,
    solid: &TileCollision,
    feet: Hitbox,
) -> InputManager {
    let mut inputs = InputManager::new();
    inputs.on_press(
        PadButton::Left,
        [
            sprites.move_composite_left_blocked(player, 1, LEFT_LIMIT, solid, feet),
            sprites.enable_composite_animation(player, walk.left),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Right,
        [
            sprites.move_composite_right_blocked(player, 1, RIGHT_LIMIT, solid, feet),
            sprites.enable_composite_animation(player, walk.right),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Up,
        [
            sprites.move_composite_up_blocked(player, 1, TOP_LIMIT, solid, feet),
            sprites.enable_composite_animation(player, walk.back),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Down,
        [
            sprites.move_composite_down_blocked(player, 1, BOTTOM_LIMIT, solid, feet),
            sprites.enable_composite_animation(player, walk.front),
        ]
        .concat(),
    );
    for (button, animation) in [
        (PadButton::Left, walk.left),
        (PadButton::Right, walk.right),
        (PadButton::Up, walk.back),
        (PadButton::Down, walk.front),
    ] {
        inputs.on_release(button, stop_walk(sprites, player, animation));
    }
    inputs
}

/// Show the first frame of `animation` and stop animating
fn stop_walk(sprites: &SpriteManager, player: CompositeSpriteId, animation: u8) -> Vec<Instr> {
    [
        sprites.restart_composite_animation(player, animation),
        sprites.disable_composite_animation(player),
    ]
    .concat()
}
