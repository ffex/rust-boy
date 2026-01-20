use rust_boy::{
    gb_std::inputs::PadButton,
    rust_boy::{AnimationType, InputManager, RustBoy, TileSource},
};

fn main() {
    let mut gb = RustBoy::new();

    // Add sprite with coin animation tiles (7 frames: 0-6)
    let coin = gb.add_sprite("Coin", TileSource::from_file("coin.2bpp"), 80, 72, 0);

    // Add looping animation with relative frame indices 0 to 6
    gb.sprites
        .add_animation(coin, "CoinAnim", 0, 6, AnimationType::Loop);
    gb.sprites.disable_animation("CoinAnim");
    // Input handling
    let mut inputs = InputManager::new();
    inputs.on_press(PadButton::A, gb.sprites.enable_animation("CoinAnim"));
    inputs.on_press(PadButton::B, gb.sprites.disable_animation("CoinAnim"));
    gb.add_inputs(inputs);
    println!("{}", gb.build());
}
