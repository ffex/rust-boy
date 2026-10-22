use rust_boy::{
    asm::Instr,
    engine::{CompositeSpriteId, Hitbox, InputManager, SpriteManager, TileCollision},
    stdlib::inputs::PadButton,
};

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
/// Limits are OAM coordinates (screen x + 8, screen y + 16), included; the player moves as
/// one block and its leading half stops on the limit. On release, the walk stops on its
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
            sprites.move_composite_left_blocked(player, 1, 1, solid, feet),
            sprites.enable_composite_animation(player, walk.left),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Right,
        [
            sprites.move_composite_right_blocked(player, 1, 149, solid, feet),
            sprites.enable_composite_animation(player, walk.right),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Up,
        [
            sprites.move_composite_up_blocked(player, 1, 1, solid, feet),
            sprites.enable_composite_animation(player, walk.back),
        ]
        .concat(),
    );
    inputs.on_press(
        PadButton::Down,
        [
            sprites.move_composite_down_blocked(player, 1, 149, solid, feet),
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
