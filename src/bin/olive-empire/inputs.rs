use rust_boy::{
    asm::{Emittable, Instr, boxed},
    engine::{CompositeSpriteId, Direction, Hitbox, InputManager, SpriteManager, TileCollision},
    hw,
    stdlib::{flow::IfA, graphics::scroll, inputs::PadButton},
};

/// How close in pixels the player gets to a side of the view (the screen above the HUD)
/// before the background scrolls instead
const SCROLL_MARGIN: u8 = 32;

// The margins as OAM limits (screen x + 8, screen y + 16), for the sprite that leads the move:
// the leftmost / topmost one `SCROLL_MARGIN` from the left / top, the rightmost one with its
// last pixel `SCROLL_MARGIN` from the right, the lowest 8x16 one with its last line
// `SCROLL_MARGIN` above the HUD
const LEFT_LIMIT: u8 = SCROLL_MARGIN + hw::OAM_X_OFFSET;
const RIGHT_LIMIT: u8 = hw::SCRN_X.value - SCROLL_MARGIN - 8 + hw::OAM_X_OFFSET;
const TOP_LIMIT: u8 = SCROLL_MARGIN + hw::OAM_Y_OFFSET;
const BOTTOM_LIMIT: u8 =
    hw::SCRN_Y.value - crate::HUD_HEIGHT - SCROLL_MARGIN - 16 + hw::OAM_Y_OFFSET;

/// Indices of the player's walk animations, as `add_composite_animation` returned them
pub struct WalkAnimations {
    pub front: u8,
    pub back: u8,
    pub left: u8,
    pub right: u8,
}
// TODO can be do better? in RUST there is the class concept? or an interface to implement events?
/// D-pad: walk the player 1 px per frame and play the walk animation of that direction;
/// nothing moves when its `feet` would step on a `solid` tile.
/// Push-scroll: the player walks freely in the middle of the view; at `SCROLL_MARGIN` from a
/// side it stays there and the background scrolls 1 px instead (the 256x256 map wraps around).
/// On release, the walk stops on its first frame (the standing pose of that direction)
pub fn player_inputs(
    sprites: &SpriteManager,
    player: CompositeSpriteId,
    walk: &WalkAnimations,
    solid: &TileCollision,
    feet: Hitbox,
) -> InputManager {
    let halves = sprites
        .get_composite_sprites(player)
        .expect("the player is a composite");
    let (left_half, right_half) = (halves[0], halves[1]);
    let mut inputs = InputManager::new();
    for (button, direction, animation) in [
        (PadButton::Left, Direction::Left, walk.left),
        (PadButton::Right, Direction::Right, walk.right),
        (PadButton::Up, Direction::Up, walk.back),
        (PadButton::Down, Direction::Down, walk.front),
    ] {
        // The leading half's coordinate, its margin, the 1-px move up to it and the scroll
        let (leading, limit, moving, dx, dy) = match direction {
            Direction::Left => (
                sprites.get_x(left_half),
                LEFT_LIMIT,
                sprites.move_composite_left_limit(player, 1, LEFT_LIMIT),
                -1,
                0,
            ),
            Direction::Right => (
                sprites.get_x(right_half),
                RIGHT_LIMIT,
                sprites.move_composite_right_limit(player, 1, RIGHT_LIMIT),
                1,
                0,
            ),
            Direction::Up => (
                sprites.get_y(left_half),
                TOP_LIMIT,
                sprites.move_composite_up_limit(player, 1, TOP_LIMIT),
                0,
                -1,
            ),
            Direction::Down => (
                sprites.get_y(left_half),
                BOTTOM_LIMIT,
                sprites.move_composite_down_limit(player, 1, BOTTOM_LIMIT),
                0,
                1,
            ),
        };
        let step: Vec<Box<dyn Emittable>> = vec![
            boxed(leading),
            boxed(IfA::eq(limit, scroll::scroll_by(dx, dy)).or_else(moving)),
        ];
        inputs.on_press(
            button,
            [
                sprites.when_composite_free(player, direction, 1, feet, solid, step),
                sprites.enable_composite_animation(player, animation),
            ]
            .concat(),
        );
    }
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
