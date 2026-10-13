use rust_boy::prelude::*;

#[test]
fn pools_and_static_sprites_share_checked_oam_capacity() {
    let mut game = RustBoy::new();
    let first = game.create_sprite_pool("First", 20);
    let second = game.create_sprite_pool("Second", 20);
    assert_eq!((first.first_slot(), first.capacity()), (0, 20));
    assert_eq!((second.first_slot(), second.capacity()), (20, 20));
    let before = game.build().unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || game.create_sprite_pool("TooMany", 1)
        ))
        .is_err()
    );
    assert_eq!(game.build().unwrap(), before);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || game.create_sprite_pool("First", 20)
        ))
        .is_err()
    );
    assert_eq!(game.build().unwrap(), before);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || game.create_sprite_pool("Empty", 0)
        ))
        .is_err()
    );
    assert_eq!(game.build().unwrap(), before);
}

#[test]
fn pool_mutation_is_rejected_in_handlers_directly_and_transitively() {
    for through_function in [false, true] {
        for operation in 0..4 {
            let mut game = RustBoy::new();
            let pool = game.create_sprite_pool("Actors", 3);
            let calls = [pool.allocate(), pool.release(), pool.write(), pool.hide()];
            if through_function {
                game.define_function_from("MutatePool", calls[operation].clone());
                game.on_vblank(Call::new("MutatePool"));
            } else {
                game.on_vblank(calls[operation].clone());
            }
            assert!(matches!(game.build(), Err(Error::MainLoopOnly { .. })));
        }
    }
}

#[test]
fn the_sprite_size_cannot_change_after_a_pool() {
    // A pool's Write routine checks 8x16 tiles when the pool is created: switching to 8x16
    // afterwards would leave it accepting odd tiles
    for size in [SpriteSize::Size8x16, SpriteSize::Size8x8] {
        let other = if size == SpriteSize::Size8x16 {
            SpriteSize::Size8x8
        } else {
            SpriteSize::Size8x16
        };
        let mut game = RustBoy::with_config(RustBoyConfig::default().sprite_size(size));
        game.create_sprite_pool("Actors", 2);
        assert!(!game.sprites.is_empty(), "a pool reserves OAM");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            game.set_sprite_size(other);
        }));
        let message = result.unwrap_err();
        let message = message
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_default();
        assert!(message.contains("sprite pool"), "{message}");
        game.set_sprite_size(size);
        game.build().unwrap();
    }
}
