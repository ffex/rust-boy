use rust_boy::prelude::*;

#[test]
fn array_shapes_ranges_and_static_indices_are_checked() {
    let mut game = RustBoy::new();
    for (element, min, max) in [
        (ArrayElement::U8, 0, 255),
        (ArrayElement::I8, -128, 127),
        (ArrayElement::U16, 0, 65535),
        (ArrayElement::I16, -32768, 32767),
    ] {
        let name = format!("wArray{element:?}");
        let array = game.vars.create_array(&name, element, 3, min);
        assert_eq!(array.len(), 3);
        assert!(!array.is_empty());
        assert_eq!(array.element_type(), element);
        assert!(!array.set(2, max).is_empty());
        for code in [
            std::panic::catch_unwind(|| array.set(3, 0)),
            std::panic::catch_unwind(|| array.get(3)),
            std::panic::catch_unwind(|| array.set(0, min - 1)),
            std::panic::catch_unwind(|| array.set(0, max + 1)),
        ] {
            assert!(code.is_err());
        }
        let same = game.vars.create_array(&name, element, 3, max);
        assert_eq!(same.id(), array.id());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game
                .vars
                .create_array(&name, element, 4, min)))
            .is_err()
        );
    }
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.vars.create_array(
            "wEmpty",
            ArrayElement::U8,
            0,
            0
        )))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.vars.create_array(
            "wTooLarge",
            ArrayElement::U16,
            65535,
            0
        )))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game
            .vars
            .create_u8("wArrayU8", 0)))
        .is_err()
    );
}

#[test]
fn arrays_share_allocation_and_report_memory_full_without_build_panics() {
    let mut game = RustBoy::new();
    let before = game.vars.create_u8("wBefore", 1);
    let words = game
        .vars
        .create_array("wWords", ArrayElement::U16, 300, 100);
    let after = game.vars.create_u8("wAfter", 2);
    assert_eq!(game.vars.get_address(before.id()), Some(hw::RAM.value));
    assert_eq!(game.vars.get_address(words.id()), Some(hw::RAM.value + 1));
    assert_eq!(game.vars.get_address(after.id()), Some(hw::RAM.value + 601));
    let output = game.build().unwrap();
    assert_eq!(game.build().unwrap(), output);
    game.vars.create_array("wFull", ArrayElement::U8, 4096, 0);
    assert!(matches!(
        game.build(),
        Err(Error::MemoryFull {
            region: MemoryRegion::Wram0,
            ..
        })
    ));
}
