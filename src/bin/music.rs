//! Pinned hUGEDriver with a self-contained hUGETracker-format two-note song.
use rust_boy::prelude::*;
fn main() {
    let song = Song::from_rgbds(include_str!("../../examples/music/song.asm"), "DemoSong");
    let mut game = RustBoy::new();
    game.enable_music(song, MusicClock::MainFrame);
    println!("{}", game.build().expect("music program"));
}
