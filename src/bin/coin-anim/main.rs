use rust_boy::prelude::*;

fn main() -> Result<(), Error> {
    let mut gb = RustBoy::new();

    // Add sprite with coin animation tiles (7 frames: 0-6)
    let coin = gb.add_sprite("Coin", TileSource::from_file("coin.2bpp", 7), 80, 72, 0);

    // Add looping animation with relative frame indices 0 to 6
    // (animations start disabled; A starts it, B stops it)
    let coin_anim = gb
        .sprites
        .add_animation(coin, "CoinAnim", 0, 6, AnimationType::Loop);
    // Input handling
    let mut inputs = InputManager::new();
    inputs.on_press(PadButton::A, gb.sprites.enable_animation(coin, coin_anim));
    inputs.on_press(PadButton::B, gb.sprites.disable_animation(coin));
    gb.add_inputs(inputs);
    println!("{}", gb.build()?);
    Ok(())
}
