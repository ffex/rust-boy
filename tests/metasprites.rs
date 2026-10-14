use rust_boy::prelude::*;

fn part(x: i16, y: i16, tiles: usize) -> MetaspritePart {
    MetaspritePart::new(TileSource::from_raw(&vec![["$ff"; 8]; tiles]), x, y, 0u8)
}

#[test]
fn invalid_metasprite_layout_is_rejected_before_registration() {
    for parts in [
        vec![],
        vec![part(-90, 0, 1)],
        vec![part(168, 0, 1)],
        vec![part(0, 168, 1)],
        vec![part(0, 0, 0)],
        vec![part(0, 0, 257)],
        vec![part(0, 0, 1); 41],
    ] {
        let mut game = RustBoy::new();
        let before = game.build().unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || game.add_metasprite("Invalid", 80, 72, &parts)
            ))
            .is_err()
        );
        assert_eq!(game.build().unwrap(), before);
    }
    let mut game = RustBoy::with_config(RustBoyConfig::default().sprite_size(SpriteSize::Size8x16));
    let before = game.build().unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.add_metasprite(
            "Odd",
            80,
            72,
            &[part(0, 0, 1)]
        )))
        .is_err()
    );
    assert_eq!(game.build().unwrap(), before);
}

#[test]
fn metasprites_are_deterministic_and_names_do_not_collide() {
    let mut game = RustBoy::new();
    let id = game.add_metasprite(
        "Actor",
        80,
        72,
        &[part(0, 0, 2), part(8, 8, 2), part(-8, 16, 2)],
    );
    game.sprites
        .add_composite_animation(id, "Walk", 0, 1, AnimationType::Loop);
    game.sprites.set_composite_initial_animation(id, 0);
    let before = game.build().unwrap();
    assert_eq!(game.build().unwrap(), before);
    assert!(before.find("Actor_part0:").unwrap() < before.find("Actor_part1:").unwrap());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.add_metasprite(
            "Actor",
            80,
            72,
            &[part(0, 0, 1)]
        )))
        .is_err()
    );
    assert_eq!(game.build().unwrap(), before);
}

/// The file of the panic `f` raises: `#[track_caller]` makes it the caller's
fn panic_file(f: impl FnOnce()) -> String {
    use std::cell::RefCell;
    thread_local!(static FILE: RefCell<String> = const { RefCell::new(String::new()) });
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|info| {
        let file = info
            .location()
            .map(|l| l.file().to_string())
            .unwrap_or_default();
        FILE.with(|cell| *cell.borrow_mut() = file);
    }));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(previous);
    assert!(result.is_err(), "it did not panic");
    FILE.with(|cell| cell.borrow().clone())
}

#[test]
fn metasprite_panics_point_at_the_caller() {
    for parts in [vec![], vec![part(0, 0, 0)], vec![part(-90, 0, 1)]] {
        let file = panic_file(|| {
            RustBoy::new().add_metasprite("Bad", 80, 72, &parts);
        });
        assert!(file.ends_with("metasprites.rs"), "{file}");
    }
}
