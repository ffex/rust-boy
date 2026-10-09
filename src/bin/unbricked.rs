use rust_boy::gb_asm::{Asm, Condition, Expr, Mem, R8, R16, Section};

fn main() {
    let mut asm = Asm::new();

    // Hardware include and constants
    asm.include_hardware();
    asm.def("BRICK_LEFT", 0x05);
    asm.def("BRICK_RIGHT", 0x06);
    asm.def("BLANK_TILE", 0x08);
    asm.def("DIGIT_OFFSET", 0x1A);
    asm.def("SCORE_TENS", 0x9870);
    asm.def("SCORE_ONES", 0x9871);

    // Header section
    asm.section(Section::rom0("Header").at(0x0100));
    asm.jp("EntryPoint");
    asm.ds_fill("$150 - @", "0");

    // Entry point
    asm.label("EntryPoint");
    asm.label("WaitVBlank");
    asm.ld_a_addr_def("rLY");
    asm.cp_imm(144);
    asm.jp_cond(Condition::C, "WaitVBlank");

    // Turn off LCD
    asm.ld_a(0);
    asm.ld_addr_def_a("rLCDC");

    // Copy tiles data
    asm.ld(R16::DE, "Tiles");
    asm.ld(R16::HL, Expr::hex(0x9000));
    asm.ld(R16::BC, Expr::sym("TilesEnd") - "Tiles");
    asm.call("Memcopy");

    // Copy the tilemap
    asm.ld(R16::DE, "Tilemap");
    asm.ld(R16::HL, Expr::hex(0x9800));
    asm.ld(R16::BC, Expr::sym("TilemapEnd") - "Tilemap");
    asm.call("Memcopy");

    // Copy the paddle tile
    asm.ld(R16::DE, "Paddle");
    asm.ld(R16::HL, Expr::hex(0x8000));
    asm.ld(R16::BC, Expr::sym("PaddleEnd") - "Paddle");
    asm.call("Memcopy");

    // Copy the ball tile
    asm.ld(R16::DE, "Ball");
    asm.ld(R16::HL, Expr::hex(0x8010));
    asm.ld(R16::BC, Expr::sym("BallEnd") - "Ball");
    asm.call("Memcopy");

    // Initialize OAM
    asm.ld_a(0);
    asm.ld_b(160);
    asm.ld(R16::HL, "_OAMRAM");

    // Clear OAM loop
    asm.label("ClearOam");
    asm.ld(Mem::Hli, R8::A);
    asm.dec(R8::B);
    asm.jp_cond(Condition::NZ, "ClearOam");

    // Draw object in OAM - paddle
    asm.ld(R16::HL, "_OAMRAM");
    asm.ld_a(128 + 16);
    asm.ld(Mem::Hli, R8::A);
    asm.ld_a(16 + 8);
    asm.ld(Mem::Hli, R8::A);
    asm.ld_a(0);
    asm.ld(Mem::Hli, R8::A);
    asm.ld(Mem::Hli, R8::A);

    // Draw object in OAM - ball
    asm.ld_a(100 + 16);
    asm.ld(Mem::Hli, R8::A);
    asm.ld_a(32 + 8);
    asm.ld(Mem::Hli, R8::A);
    asm.ld_a(1);
    asm.ld(Mem::Hli, R8::A);
    asm.ld_a(0);
    asm.ld(Mem::Hli, R8::A);

    asm.ld_a(1);
    asm.ld_addr_def_a("wBallMomentumX");
    asm.ld(R8::A, -1);
    asm.ld_addr_def_a("wBallMomentumY");

    // Turn LCD On
    asm.ld(R8::A, Expr::sym("LCDCF_ON") | "LCDCF_BGON" | "LCDCF_OBJON");
    asm.ld_addr_def_a("rLCDC");

    // Initialize display registers
    asm.ld(R8::A, Expr::bin(0b11100100));
    asm.ld_addr_def_a("rBGP");
    asm.ld(R8::A, Expr::bin(0b11100100));
    asm.ld_addr_def_a("rOBP0");

    // Initialize global variables
    asm.ld_a(0);
    asm.ld_addr_def_a("wFrameCounter");
    asm.ld_addr_def_a("wCurKeys");
    asm.ld_addr_def_a("wNewKeys");
    asm.ld_addr_def_a("wScore");

    // Main loop
    asm.label("Main");
    asm.comment("Wait until it's *not* VBlank");
    asm.ld_a_addr_def("rLY");
    asm.cp_imm(144);
    asm.jp_cond(Condition::NC, "Main");

    asm.label("WaitVBlank2");
    asm.ld_a_addr_def("rLY");
    asm.cp_imm(144);
    asm.jp_cond(Condition::C, "WaitVBlank2");

    // Add the ball's momentum to its position in OAM
    asm.ld_a_addr_def("wBallMomentumX");
    asm.ld(R8::B, R8::A);
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 5);
    asm.add(R8::B);
    asm.ld_addr_def_a(Expr::sym("_OAMRAM") + 5);

    asm.ld_a_addr_def("wBallMomentumY");
    asm.ld(R8::B, R8::A);
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 4);
    asm.add(R8::B);
    asm.ld_addr_def_a(Expr::sym("_OAMRAM") + 4);

    // BounceOnTop
    asm.label("BounceOnTop");
    asm.comment("Remember to offset the OAM position!");
    asm.comment("(8, 16) in OAM coordinates is (0, 0) on the screen.");
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 4);
    asm.sub(Expr::num(16) + 1);
    asm.ld(R8::C, R8::A);
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 5);
    asm.sub(8);
    asm.ld(R8::B, R8::A);
    asm.call("GetTileByPixel");
    asm.ld(R8::A, R8::AtHl);
    asm.call("IsWallTile");
    asm.jp_cond(Condition::NZ, "BounceOnRight");
    asm.call("CheckAndHandleBrick");
    asm.ld_a(1);
    asm.ld_addr_def_a("wBallMomentumY");

    // BounceOnRight
    asm.label("BounceOnRight");
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 4);
    asm.sub(16);
    asm.ld(R8::C, R8::A);
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 5);
    asm.sub(Expr::num(8) - 1);
    asm.ld(R8::B, R8::A);
    asm.call("GetTileByPixel");
    asm.ld(R8::A, R8::AtHl);
    asm.call("IsWallTile");
    asm.jp_cond(Condition::NZ, "BounceOnLeft");
    asm.ld(R8::A, -1);
    asm.ld_addr_def_a("wBallMomentumX");

    // BounceOnLeft
    asm.label("BounceOnLeft");
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 4);
    asm.sub(16);
    asm.ld(R8::C, R8::A);
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 5);
    asm.sub(Expr::num(8) + 1);
    asm.ld(R8::B, R8::A);
    asm.call("GetTileByPixel");
    asm.ld(R8::A, R8::AtHl);
    asm.call("IsWallTile");
    asm.jp_cond(Condition::NZ, "BounceOnBottom");
    asm.ld_a(1);
    asm.ld_addr_def_a("wBallMomentumX");

    // BounceOnBottom
    asm.label("BounceOnBottom");
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 4);
    asm.sub(Expr::num(16) - 1);
    asm.ld(R8::C, R8::A);
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 5);
    asm.sub(8);
    asm.ld(R8::B, R8::A);
    asm.call("GetTileByPixel");
    asm.ld(R8::A, R8::AtHl);
    asm.call("IsWallTile");
    asm.jp_cond(Condition::NZ, "BounceDone");
    asm.ld(R8::A, -1);
    asm.ld_addr_def_a("wBallMomentumY");

    asm.label("BounceDone");
    asm.comment("First, check if the ball is low enough to bounce off the paddle.");
    asm.ld_a_addr_def("_OAMRAM");
    asm.ld(R8::B, R8::A);
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 4);
    asm.add(5);
    asm.cp(R8::B);
    asm.jp_cond(Condition::NZ, "PaddleBounceDone");

    asm.comment("Now let's compare the X positions of the objects to see if they're touching.");
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 5);
    asm.ld(R8::B, R8::A);
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 1);
    asm.sub(8);
    asm.cp(R8::B);
    asm.jp_cond(Condition::NC, "PaddleBounceDone");
    asm.add(Expr::num(8) + 16);
    asm.cp(R8::B);
    asm.jp_cond(Condition::C, "PaddleBounceDone");

    asm.ld(R8::A, -1);
    asm.ld_addr_def_a("wBallMomentumY");

    asm.label("PaddleBounceDone");
    asm.call("UpdateKeys");

    // Check if the left button is pressed
    asm.label("CheckLeft");
    asm.ld_a_addr_def("wCurKeys");
    asm.and("PADF_LEFT");
    asm.jp_cond(Condition::Z, "CheckRight");

    asm.label("Left");
    asm.comment("move the paddle one pixel to the left");
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 1);
    asm.dec(R8::A);
    asm.cp(15);
    asm.jp_cond(Condition::Z, "Main");
    asm.ld_addr_def_a(Expr::sym("_OAMRAM") + 1);
    asm.jp("Main");

    asm.label("CheckRight");
    asm.ld_a_addr_def("wCurKeys");
    asm.and("PADF_RIGHT");
    asm.jp_cond(Condition::Z, "Main");

    asm.label("Right");
    asm.comment("move the paddle one pixel to the right");
    asm.ld_a_addr_def(Expr::sym("_OAMRAM") + 1);
    asm.inc(R8::A);
    asm.cp(105);
    asm.jp_cond(Condition::Z, "Main");
    asm.ld_addr_def_a(Expr::sym("_OAMRAM") + 1);
    asm.jp("Main");

    // Memcopy function
    asm.comment("Copy bytes from one area to another");
    asm.comment("@param de: source");
    asm.comment("@param hl: destination");
    asm.comment("@param bc: length");
    asm.label("Memcopy");
    asm.ld(R8::A, Mem::De);
    asm.ld(Mem::Hli, R8::A);
    asm.inc(R16::DE);
    asm.dec(R16::BC);
    asm.ld(R8::A, R8::B);
    asm.or(R8::C);
    asm.jp_cond(Condition::NZ, "Memcopy");
    asm.ret();

    // UpdateKeys function
    asm.label("UpdateKeys");
    asm.comment("poll half the controller");
    asm.ld(R8::A, "P1F_GET_BTN");
    asm.call(".onenibble");
    asm.ld(R8::B, R8::A);

    asm.comment("poll the other half");
    asm.ld(R8::A, "P1F_GET_DPAD");
    asm.call(".onenibble");
    asm.swap(R8::A);
    asm.xor(R8::B);
    asm.ld(R8::B, R8::A);

    asm.comment("And release the controller");
    asm.ld(R8::A, "P1F_GET_NONE");
    asm.ldh(Mem::addr("rP1"), R8::A);

    asm.comment("Combine with previous wCurKeys to make wNewKeys");
    asm.ld_a_addr_def("wCurKeys");
    asm.xor(R8::B);
    asm.and(R8::B);
    asm.ld_addr_def_a("wNewKeys");
    asm.ld(R8::A, R8::B);
    asm.ld_addr_def_a("wCurKeys");
    asm.ret();

    asm.label(".onenibble");
    asm.ldh(Mem::addr("rP1"), R8::A);
    asm.call(".knowret");
    asm.ldh(R8::A, Mem::addr("rP1"));
    asm.ldh(R8::A, Mem::addr("rP1"));
    asm.or(Expr::hex(0xF0));
    asm.ret();

    asm.label(".knowret");
    asm.ret();

    // CheckAndHandleBrick function
    asm.comment("check if a brick was collided with and breaks if it is possible");
    asm.comment("@param hl: address of the tile");
    asm.label("CheckAndHandleBrick");
    asm.ld(R8::A, R8::AtHl);
    asm.cp("BRICK_LEFT");
    asm.jr_cond(Condition::NZ, "CheckAndHandleBrickRight");
    asm.comment("break from left side");
    asm.ld(R8::AtHl, "BLANK_TILE");
    asm.inc(R16::HL);
    asm.ld(R8::AtHl, "BLANK_TILE");
    asm.call("IncreaseScorePackedBCD");
    asm.ret();

    asm.label("CheckAndHandleBrickRight");
    asm.cp("BRICK_RIGHT");
    asm.ret_cond(Condition::NZ);
    asm.ld(R8::AtHl, "BLANK_TILE");
    asm.dec(R16::HL);
    asm.ld(R8::AtHl, "BLANK_TILE");
    asm.call("IncreaseScorePackedBCD");
    asm.ret();

    // GetTileByPixel function
    asm.comment("Convert a pixel position to a tilemap address");
    asm.comment("hl = $9800 + X + Y * 32");
    asm.comment("@param b: X");
    asm.comment("@param c: Y");
    asm.comment("@return hl: tile address");
    asm.label("GetTileByPixel");
    asm.comment("First, we need to divide by 8 to convert a pixel position to a tile position.");
    asm.comment("After this we want to multiply the Y position by 32.");
    asm.comment("These operations effectively cancel out so we only need to mask the Y value.");
    asm.ld(R8::A, R8::C);
    asm.and(Expr::bin(0b11111000));
    asm.ld(R8::L, R8::A);
    asm.ld_h(0);
    asm.comment("Now we have the position * 8 in hl");
    asm.add_hl(R16::HL);
    asm.add_hl(R16::HL);
    asm.comment("Convert the X position to an offset.");
    asm.ld(R8::A, R8::B);
    asm.srl(R8::A);
    asm.srl(R8::A);
    asm.srl(R8::A);
    asm.comment("Add the two offsets together.");
    asm.add(R8::L);
    asm.ld(R8::L, R8::A);
    asm.adc(R8::H);
    asm.sub(R8::L);
    asm.ld(R8::H, R8::A);
    asm.comment("Add the offset to the tilemap's base address, and we are done!");
    asm.ld(R16::BC, Expr::hex(0x9800));
    asm.add_hl(R16::BC);
    asm.ret();

    // IsWallTile function
    asm.comment("@param a: tile ID");
    asm.comment("@return z: set if a is a wall.");
    asm.label("IsWallTile");
    asm.cp(Expr::hex(0x00));
    asm.ret_cond(Condition::Z);
    asm.cp(Expr::hex(0x01));
    asm.ret_cond(Condition::Z);
    asm.cp(Expr::hex(0x02));
    asm.ret_cond(Condition::Z);
    asm.cp(Expr::hex(0x04));
    asm.ret_cond(Condition::Z);
    asm.cp(Expr::hex(0x05));
    asm.ret_cond(Condition::Z);
    asm.cp(Expr::hex(0x06));
    asm.ret_cond(Condition::Z);
    asm.cp(Expr::hex(0x07));
    asm.ret();

    // IncreaseScorePackedBCD function
    asm.comment("Increase score by 1 and store it as a 1 byte packed BCD number");
    asm.comment("changes A and HL");
    asm.label("IncreaseScorePackedBCD");
    asm.xor(R8::A);
    asm.inc(R8::A);
    asm.ld(R16::HL, "wScore");
    asm.adc(R8::AtHl);
    asm.daa();
    asm.ld(R8::AtHl, R8::A);
    asm.call("UpdateScoreBoard");
    asm.ret();

    // UpdateScoreBoard function
    asm.label("UpdateScoreBoard");
    asm.ld_a_addr_def("wScore");
    asm.and(Expr::bin(0b11110000));
    asm.swap(R8::A);
    asm.add("DIGIT_OFFSET");
    asm.ld_addr_def_a("SCORE_TENS");

    asm.ld_a_addr_def("wScore");
    asm.and(Expr::bin(0b00001111));
    asm.add("DIGIT_OFFSET");
    asm.ld_addr_def_a("SCORE_ONES");
    asm.ret();

    // Tiles data
    add_tiles(&mut asm);

    // Tilemap
    add_tilemap(&mut asm);

    // Sprites
    add_sprites(&mut asm);

    // WRAM sections
    asm.section(Section::wram0("Counter"));
    asm.raw("wFrameCounter: db");

    asm.section(Section::wram0("Input Variables"));
    asm.raw("wCurKeys: db");
    asm.raw("wNewKeys: db");

    asm.section(Section::wram0("Ball Data"));
    asm.raw("wBallMomentumX: db");
    asm.raw("wBallMomentumY: db");

    asm.section(Section::wram0("Score"));
    asm.raw("wScore: db");

    // Output the generated assembly
    println!("{}", asm.to_asm());
}

fn add_tiles(asm: &mut Asm) {
    asm.label("Tiles");
    // Tile 0-9 (wall and game tiles)
    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`33322222");
    asm.dw("`33322222");
    asm.dw("`33322222");
    asm.dw("`33322211");
    asm.dw("`33322211");

    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`22222222");
    asm.dw("`22222222");
    asm.dw("`22222222");
    asm.dw("`11111111");
    asm.dw("`11111111");

    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`22222333");
    asm.dw("`22222333");
    asm.dw("`22222333");
    asm.dw("`11222333");
    asm.dw("`11222333");

    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`33333333");
    asm.dw("`33333333");

    asm.dw("`33322211");
    asm.dw("`33322211");
    asm.dw("`33322211");
    asm.dw("`33322211");
    asm.dw("`33322211");
    asm.dw("`33322211");
    asm.dw("`33322211");
    asm.dw("`33322211");

    asm.dw("`22222222");
    asm.dw("`20000000");
    asm.dw("`20111111");
    asm.dw("`20111111");
    asm.dw("`20111111");
    asm.dw("`20111111");
    asm.dw("`22222222");
    asm.dw("`33333333");

    asm.dw("`22222223");
    asm.dw("`00000023");
    asm.dw("`11111123");
    asm.dw("`11111123");
    asm.dw("`11111123");
    asm.dw("`11111123");
    asm.dw("`22222223");
    asm.dw("`33333333");

    asm.dw("`11222333");
    asm.dw("`11222333");
    asm.dw("`11222333");
    asm.dw("`11222333");
    asm.dw("`11222333");
    asm.dw("`11222333");
    asm.dw("`11222333");
    asm.dw("`11222333");

    asm.dw("`00000000");
    asm.dw("`00000000");
    asm.dw("`00000000");
    asm.dw("`00000000");
    asm.dw("`00000000");
    asm.dw("`00000000");
    asm.dw("`00000000");
    asm.dw("`00000000");

    asm.dw("`11001100");
    asm.dw("`11111111");
    asm.dw("`11111111");
    asm.dw("`21212121");
    asm.dw("`22222222");
    asm.dw("`22322232");
    asm.dw("`23232323");
    asm.dw("`33333333");

    // Logo tiles (10-25)
    add_logo_tiles(asm);

    // Digit tiles (26-35)
    add_digit_tiles(asm);

    asm.label("TilesEnd");
}

fn add_logo_tiles(asm: &mut Asm) {
    // Tiles 10-25 (logo)
    let logo_data = [
        [
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222211",
            "`22222211",
            "`22222211",
        ],
        [
            "`22222222",
            "`22222222",
            "`22222222",
            "`11111111",
            "`11111111",
            "`11221111",
            "`11221111",
            "`11000011",
        ],
        [
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`11222222",
            "`11222222",
            "`11222222",
        ],
        [
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
        ],
        [
            "`22222211",
            "`22222200",
            "`22222200",
            "`22000000",
            "`22000000",
            "`22222222",
            "`22222222",
            "`22222222",
        ],
        [
            "`11000011",
            "`11111111",
            "`11111111",
            "`11111111",
            "`11111111",
            "`11111111",
            "`11111111",
            "`11000022",
        ],
        [
            "`11222222",
            "`11222222",
            "`11222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
        ],
        [
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
        ],
        [
            "`22222222",
            "`22222200",
            "`22222200",
            "`22222211",
            "`22222211",
            "`22221111",
            "`22221111",
            "`22221111",
        ],
        [
            "`11000022",
            "`00112222",
            "`00112222",
            "`11112200",
            "`11112200",
            "`11220000",
            "`11220000",
            "`11220000",
        ],
        [
            "`22222222",
            "`22222222",
            "`22222222",
            "`22000000",
            "`22000000",
            "`00000000",
            "`00000000",
            "`00000000",
        ],
        [
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`22222222",
            "`11110022",
            "`11110022",
            "`11110022",
        ],
        [
            "`22221111",
            "`22221111",
            "`22221111",
            "`22221111",
            "`22221111",
            "`22222211",
            "`22222211",
            "`22222222",
        ],
        [
            "`11220000",
            "`11110000",
            "`11110000",
            "`11111111",
            "`11111111",
            "`11111111",
            "`11111111",
            "`22222222",
        ],
        [
            "`00000000",
            "`00111111",
            "`00111111",
            "`11111111",
            "`11111111",
            "`11111111",
            "`11111111",
            "`22222222",
        ],
        [
            "`11110022",
            "`11000022",
            "`11000022",
            "`00002222",
            "`00002222",
            "`00222222",
            "`00222222",
            "`22222222",
        ],
    ];

    for tile in &logo_data {
        for line in tile {
            asm.dw(line);
        }
    }
}

fn add_digit_tiles(asm: &mut Asm) {
    // Digits 0-9
    let digits = [
        [
            "`33333333",
            "`33000033",
            "`30033003",
            "`30033003",
            "`30033003",
            "`30033003",
            "`33000033",
            "`33333333",
        ], // 0
        [
            "`33333333",
            "`33300333",
            "`33000333",
            "`33300333",
            "`33300333",
            "`33300333",
            "`33000033",
            "`33333333",
        ], // 1
        [
            "`33333333",
            "`33000033",
            "`30330003",
            "`33330003",
            "`33000333",
            "`30003333",
            "`30000003",
            "`33333333",
        ], // 2
        [
            "`33333333",
            "`30000033",
            "`33330003",
            "`33000033",
            "`33330003",
            "`33330003",
            "`30000033",
            "`33333333",
        ], // 3
        [
            "`33333333",
            "`33000033",
            "`30030033",
            "`30330033",
            "`30330033",
            "`30000003",
            "`33330033",
            "`33333333",
        ], // 4
        [
            "`33333333",
            "`30000033",
            "`30033333",
            "`30000033",
            "`33330003",
            "`30330003",
            "`33000033",
            "`33333333",
        ], // 5
        [
            "`33333333",
            "`33000033",
            "`30033333",
            "`30000033",
            "`30033003",
            "`30033003",
            "`33000033",
            "`33333333",
        ], // 6
        [
            "`33333333",
            "`30000003",
            "`33333003",
            "`33330033",
            "`33300333",
            "`33000333",
            "`33000333",
            "`33333333",
        ], // 7
        [
            "`33333333",
            "`33000033",
            "`30333003",
            "`33000033",
            "`30333003",
            "`30333003",
            "`33000033",
            "`33333333",
        ], // 8
        [
            "`33333333",
            "`33000033",
            "`30330003",
            "`30330003",
            "`33000003",
            "`33330003",
            "`33000033",
            "`33333333",
        ], // 9
    ];

    for digit in &digits {
        for line in digit {
            asm.dw(line);
        }
    }
}

fn add_tilemap(asm: &mut Asm) {
    asm.label("Tilemap");
    asm.db("$00, $01, $01, $01, $01, $01, $01, $01, $01, $01, $01, $01, $01, $02, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $0A, $0B, $0C, $0D, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $0E, $0F, $10, $11, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $12, $13, $14, $15, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $16, $17, $18, $19, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.db("$04, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0");
    asm.label("TilemapEnd");
}

fn add_sprites(asm: &mut Asm) {
    // Paddle sprite
    asm.label("Paddle");
    asm.dw("`13333331");
    asm.dw("`30000003");
    asm.dw("`13333331");
    asm.dw("`00000000");
    asm.dw("`00000000");
    asm.dw("`00000000");
    asm.dw("`00000000");
    asm.dw("`00000000");
    asm.label("PaddleEnd");

    // Ball sprite
    asm.label("Ball");
    asm.dw("`00033000");
    asm.dw("`00322300");
    asm.dw("`03222230");
    asm.dw("`03222230");
    asm.dw("`00322300");
    asm.dw("`00033000");
    asm.dw("`00000000");
    asm.dw("`00000000");
    asm.label("BallEnd");
}
