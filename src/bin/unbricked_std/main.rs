mod tilemap;
mod tiles;

use rust_boy::{
    gb_asm::Asm,
    gb_std::{
        graphics::{
            sprites::{Sprite, SpriteManager, clear_objects_screen, initialize_objects_screen},
            utility::{
                add_tilemap, add_tiles, cp_in_memory, memcopy, turn_off_screen, turn_on_screen,
                wait_not_vblank, wait_vblank,
            },
        },
        inputs::{check_key, update_keys},
        utility::header_section,
        variables::{VariableSection, def_const},
    },
};

fn main() {
    let mut asm = Asm::new();
    // TODO think about Hexadecimal rapresentation
    asm.include_hardware();
    asm.def("BRICK_LEFT", 0x05);
    asm.def("BRICK_RIGHT", 0x06);
    asm.def("BLANK_TILE", 0x08);
    asm.def("DIGIT_OFFSET", 0x1A);
    asm.def("SCORE_TENS", 0x9870);
    asm.def("SCORE_ONES", 0x9871);
    //TODO maybe we have to think about to hide the hex values... how?
    // maybe we have to implement a variable system
    // and a tile object? to manage the ids?
    asm.emit_all(header_section());
    asm.label("EntryPoint"); //TODO make a main loop method?

    asm.call("WaitVBlank");
    asm.emit_all(turn_off_screen());
    //here we put the tiles mangement in the main also. FLOW1

    //Tile management
    // TODO some intializzation can be joined. (add_tiles and cp_in_memory)
    // TODO can we also add memcopy in automatic way when it is called first time?
    // TODO some address known can be put in constants
    // or maybe build an memory management system?
    // eg: add("paddle", WRAM)
    // add("paddle" in WRAM)
    // automatically manage the address ($8000 and after $8010)
    asm.chunk(rust_boy::gb_asm::Chunk::Tiles);

    asm.emit_all(add_tiles("Tiles", tiles::TILES));
    asm.emit_all(add_tiles("Ball", tiles::BALL));
    asm.emit_all(add_tiles("Paddle", tiles::PADDLE));

    asm.chunk(rust_boy::gb_asm::Chunk::Main);
    asm.emit_all(cp_in_memory("Tiles", "$9000"));
    asm.emit_all(cp_in_memory("Ball", "$8010"));
    asm.emit_all(cp_in_memory("Paddle", "$8000"));
    asm.emit_all(cp_in_memory("Tilemap", "$9800"));

    //FLOW1 we continue with the main
    asm.emit_all(initialize_objects_screen());
    asm.emit_all(clear_objects_screen());

    //Sprite managment
    let mut sprite_manager = SpriteManager::new();
    let mut paddle = sprite_manager.add_sprite(16, 128, 0, 0);
    let mut ball = sprite_manager.add_sprite(32, 100, 1, 0);
    //TODO remove follow lines
    //asm.emit_all(paddle.draw());
    //asm.emit_all(ball.draw());
    asm.emit_all(sprite_manager.draw());

    asm.emit_all(turn_on_screen());
    // TODO follow the palette/register initialization
    asm.ld_a(0b11100100);
    asm.ld_addr_def_a("rBGP");
    asm.ld_a(0b11100100);
    asm.ld_addr_def_a("rOBP0");

    //TODO miss the ball initialization
    //TODO init variables
    asm.ld_a(0);
    asm.ld_addr_def_a("wNewKeys");
    asm.ld_addr_def_a("wCurKeys");
    //MAIN LOOP START
    // TODO probably we have to implement the main loop
    asm.label("Main");
    asm.call("WaitNotVBlank");
    asm.call("WaitVBlank");
    asm.call("UpdateKeys");
    //TODO miss the ball management
    let left_pressed = sprite_manager.get_sprite_mut(1).unwrap().move_left(1);
    let right_pressed = sprite_manager.get_sprite_mut(1).unwrap().move_right(1);
    let up_pressed = sprite_manager.get_sprite_mut(1).unwrap().move_up(1);
    let down_pressed = sprite_manager.get_sprite_mut(1).unwrap().move_down(1);
    asm.emit_all(check_key(
        rust_boy::gb_std::inputs::PadButton::Left,
        left_pressed,
    ));
    asm.emit_all(check_key(
        rust_boy::gb_std::inputs::PadButton::Right,
        right_pressed,
    ));
    asm.emit_all(check_key(
        rust_boy::gb_std::inputs::PadButton::Up,
        up_pressed,
    ));
    asm.emit_all(check_key(
        rust_boy::gb_std::inputs::PadButton::Down,
        down_pressed,
    ));
    asm.jp("Main");
    //Variables management
    asm.chunk(rust_boy::gb_asm::Chunk::Data);
    let mut counter_sec = VariableSection::new("Counter", "WRAM0");
    let mut input_vars_sec = VariableSection::new("Input Variables", "WRAM0");
    let mut ball_data_sec = VariableSection::new("Ball Data", "WRAM0");
    let mut score_sec = VariableSection::new("Score", "WRAM0");

    counter_sec.add_data("wFrameCounter", "db"); //TODO make an enum with the var type?
    input_vars_sec.add_data("wCurKeys", "db"); //TODO input vars automatically added?
    input_vars_sec.add_data("wNewKeys", "db");
    ball_data_sec.add_data("wBallMomentumX", "db");
    ball_data_sec.add_data("wBallMomentumY", "db");
    score_sec.add_data("wScore", "db");

    asm.emit_all(counter_sec.generate());
    asm.emit_all(input_vars_sec.generate());
    asm.emit_all(ball_data_sec.generate());
    asm.emit_all(score_sec.generate());

    // Function Managment
    // TODO recognize that this are function (eg.: func_memcopy(), func_wait_vblank())
    asm.chunk(rust_boy::gb_asm::Chunk::Functions);
    asm.emit_all(memcopy());
    asm.emit_all(update_keys());
    asm.emit_all(wait_vblank());
    asm.emit_all(wait_not_vblank());

    // Tilemap Management
    asm.chunk(rust_boy::gb_asm::Chunk::Tilemap);
    asm.emit_all(add_tilemap("Tilemap", tilemap::TILEMAP));

    println!("{}", asm.to_asm());
}
