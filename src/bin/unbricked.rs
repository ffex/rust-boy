use rust_boy::gb_asm::{Asm, Condition, Operand, Register};

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
    asm.section("Header", "ROM0[$100]");
    asm.jp("EntryPoint");
    asm.ds("$150 - @", "0");

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
    asm.ld_de_label("Tiles");
    asm.ld_hl_label("$9000");
    asm.ld_bc_label("TilesEnd - Tiles");
    asm.call("Memcopy");

    // Copy the tilemap
    asm.ld_de_label("Tilemap");
    asm.ld_hl_label("$9800");
    asm.ld_bc_label("TilemapEnd - Tilemap");
    asm.call("Memcopy");

    // Copy the paddle tile
    asm.ld_de_label("Paddle");
    asm.ld_hl_label("$8000");
    asm.ld_bc_label("PaddleEnd - Paddle");
    asm.call("Memcopy");

    // Copy the ball tile
    asm.ld_de_label("Ball");
    asm.ld_hl_label("$8010");
    asm.ld_bc_label("BallEnd - Ball");
    asm.call("Memcopy");

    // Initialize OAM
    asm.ld_a(0);
    asm.ld_b(160);
    asm.ld_hl_label("_OAMRAM");

    // Clear OAM loop
    asm.label("ClearOam");
    asm.ld_hli_label("a");
    asm.dec_label("b");
    asm.jp_cond(Condition::NZ, "ClearOam");

    // Draw object in OAM - paddle
    asm.ld_hl_label("_OAMRAM");
    asm.ld_a(128 + 16);
    asm.ld_hli_label("a");
    asm.ld_a(16 + 8);
    asm.ld_hli_label("a");
    asm.ld_a(0);
    asm.ld_hli_label("a");
    asm.ld_hli_label("a");

    // Draw object in OAM - ball
    asm.ld_a(100 + 16);
    asm.ld_hli_label("a");
    asm.ld_a(32 + 8);
    asm.ld_hli_label("a");
    asm.ld_a(1);
    asm.ld_hli_label("a");
    asm.ld_a(0);
    asm.ld_hli_label("a");

    asm.ld_a(1);
    asm.ld_addr_def_a("wBallMomentumX");
    asm.ld_a_label("-1");
    asm.ld_addr_def_a("wBallMomentumY");

    // Turn LCD On
    asm.ld_a_label("LCDCF_ON | LCDCF_BGON | LCDCF_OBJON");
    asm.ld_addr_def_a("rLCDC");

    // Initialize display registers
    asm.ld_a_label("%11100100");
    asm.ld_addr_def_a("rBGP");
    asm.ld_a_label("%11100100");
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
    asm.ld_b_label("a");
    asm.ld_a_addr_def("_OAMRAM +5");
    asm.add_label("a", "b");
    asm.ld_addr_def_a("_OAMRAM +5");

    asm.ld_a_addr_def("wBallMomentumY");
    asm.ld_b_label("a");
    asm.ld_a_addr_def("_OAMRAM +4");
    asm.add_label("a", "b");
    asm.ld_addr_def_a("_OAMRAM +4");

    // BounceOnTop
    asm.label("BounceOnTop");
    asm.comment("Remember to offset the OAM position!");
    asm.comment("(8, 16) in OAM coordinates is (0, 0) on the screen.");
    asm.ld_a_addr_def("_OAMRAM + 4");
    asm.sub_label("a", "16 + 1");
    asm.ld_c_label("a");
    asm.ld_a_addr_def("_OAMRAM + 5");
    asm.sub_label("a", "8");
    asm.ld_b_label("a");
    asm.call("GetTileByPixel");
    asm.ld_a_addr_reg(Register::HL);
    asm.call("IsWallTile");
    asm.jp_cond(Condition::NZ, "BounceOnRight");
    asm.call("CheckAndHandleBrick");
    asm.ld_a(1);
    asm.ld_addr_def_a("wBallMomentumY");

    // BounceOnRight
    asm.label("BounceOnRight");
    asm.ld_a_addr_def("_OAMRAM + 4");
    asm.sub_label("a", "16");
    asm.ld_c_label("a");
    asm.ld_a_addr_def("_OAMRAM + 5");
    asm.sub_label("a", "8 - 1");
    asm.ld_b_label("a");
    asm.call("GetTileByPixel");
    asm.ld_a_addr_reg(Register::HL);
    asm.call("IsWallTile");
    asm.jp_cond(Condition::NZ, "BounceOnLeft");
    asm.ld_a_label("-1");
    asm.ld_addr_def_a("wBallMomentumX");

    // BounceOnLeft
    asm.label("BounceOnLeft");
    asm.ld_a_addr_def("_OAMRAM + 4");
    asm.sub_label("a", "16");
    asm.ld_c_label("a");
    asm.ld_a_addr_def("_OAMRAM + 5");
    asm.sub_label("a", "8 + 1");
    asm.ld_b_label("a");
    asm.call("GetTileByPixel");
    asm.ld_a_addr_reg(Register::HL);
    asm.call("IsWallTile");
    asm.jp_cond(Condition::NZ, "BounceOnBottom");
    asm.ld_a(1);
    asm.ld_addr_def_a("wBallMomentumX");

    // BounceOnBottom
    asm.label("BounceOnBottom");
    asm.ld_a_addr_def("_OAMRAM + 4");
    asm.sub_label("a", "16 - 1");
    asm.ld_c_label("a");
    asm.ld_a_addr_def("_OAMRAM + 5");
    asm.sub_label("a", "8");
    asm.ld_b_label("a");
    asm.call("GetTileByPixel");
    asm.ld_a_addr_reg(Register::HL);
    asm.call("IsWallTile");
    asm.jp_cond(Condition::NZ, "BounceDone");
    asm.ld_a_label("-1");
    asm.ld_addr_def_a("wBallMomentumY");

    asm.label("BounceDone");
    asm.comment("First, check if the ball is low enough to bounce off the paddle.");
    asm.ld_a_addr_def("_OAMRAM");
    asm.ld_b_label("a");
    asm.ld_a_addr_def("_OAMRAM + 4");
    asm.add_label("a", "5");
    asm.cp_label("b");
    asm.jp_cond(Condition::NZ, "PaddleBounceDone");

    asm.comment("Now let's compare the X positions of the objects to see if they're touching.");
    asm.ld_a_addr_def("_OAMRAM + 5");
    asm.ld_b_label("a");
    asm.ld_a_addr_def("_OAMRAM + 1");
    asm.sub_label("a", "8");
    asm.cp_label("b");
    asm.jp_cond(Condition::NC, "PaddleBounceDone");
    asm.add_label("a", "8 + 16");
    asm.cp_label("b");
    asm.jp_cond(Condition::C, "PaddleBounceDone");

    asm.ld_a_label("-1");
    asm.ld_addr_def_a("wBallMomentumY");

    asm.label("PaddleBounceDone");
    asm.call("UpdateKeys");

    // Check if the left button is pressed
    asm.label("CheckLeft");
    asm.ld_a_addr_def("wCurKeys");
    asm.and_label("PADF_LEFT");
    asm.jp_cond(Condition::Z, "CheckRight");

    asm.label("Left");
    asm.comment("move the paddle one pixel to the left");
    asm.ld_a_addr_def("_OAMRAM+1");
    asm.dec(Operand::Reg(Register::A));
    asm.cp_label("15");
    asm.jp_cond(Condition::Z, "Main");
    asm.ld_addr_def_a("_OAMRAM+1");
    asm.jp("Main");

    asm.label("CheckRight");
    asm.ld_a_addr_def("wCurKeys");
    asm.and_label("PADF_RIGHT");
    asm.jp_cond(Condition::Z, "Main");

    asm.label("Right");
    asm.comment("move the paddle one pixel to the right");
    asm.ld_a_addr_def("_OAMRAM+1");
    asm.inc(Operand::Reg(Register::A));
    asm.cp_label("105");
    asm.jp_cond(Condition::Z, "Main");
    asm.ld_addr_def_a("_OAMRAM+1");
    asm.jp("Main");

    // Memcopy function
    asm.comment("Copy bytes from one area to another");
    asm.comment("@param de: source");
    asm.comment("@param hl: destination");
    asm.comment("@param bc: length");
    asm.label("Memcopy");
    asm.ld_a_addr_reg(Register::DE);
    asm.ld_hli_label("a");
    asm.inc_label("de");
    asm.dec_label("bc");
    asm.ld_a_label("b");
    asm.or_label("a", "c");
    asm.jp_cond(Condition::NZ, "Memcopy");
    asm.ret();

    // UpdateKeys function
    asm.label("UpdateKeys");
    asm.comment("poll half the controller");
    asm.ld_a_label("P1F_GET_BTN");
    asm.call(".onenibble");
    asm.ld_b_label("a");

    asm.comment("poll the other half");
    asm.ld_a_label("P1F_GET_DPAD");
    asm.call(".onenibble");
    asm.swap_label("a");
    asm.xor_label("a", "b");
    asm.ld_b_label("a");

    asm.comment("And release the controller");
    asm.ld_a_label("P1F_GET_NONE");
    asm.ldh(
        Operand::AddrDef("rP1".to_string()),
        Operand::Reg(Register::A),
    );

    asm.comment("Combine with previous wCurKeys to make wNewKeys");
    asm.ld_a_addr_def("wCurKeys");
    asm.xor_label("a", "b");
    asm.and_label("b");
    asm.ld_addr_def_a("wNewKeys");
    asm.ld_a_label("b");
    asm.ld_addr_def_a("wCurKeys");
    asm.ret();

    asm.label(".onenibble");
    asm.ldh(
        Operand::AddrDef("rP1".to_string()),
        Operand::Reg(Register::A),
    );
    asm.call(".knowret");
    asm.ldh(
        Operand::Reg(Register::A),
        Operand::AddrDef("rP1".to_string()),
    );
    asm.ldh(
        Operand::Reg(Register::A),
        Operand::AddrDef("rP1".to_string()),
    );
    asm.or_label("a", "$F0");
    asm.ret();

    asm.label(".knowret");
    asm.ret();

    // CheckAndHandleBrick function
    asm.comment("check if a brick was collided with and breaks if it is possible");
    asm.comment("@param hl: address of the tile");
    asm.label("CheckAndHandleBrick");
    asm.ld_a_addr_reg(Register::HL);
    asm.cp_label("BRICK_LEFT");
    asm.jr_cond(Condition::NZ, "CheckAndHandleBrickRight");
    asm.comment("break from left side");
    asm.raw("ld [hl], BLANK_TILE");
    asm.inc_label("hl");
    asm.raw("ld [hl], BLANK_TILE");
    asm.call("IncreaseScorePackedBCD");
    asm.ret();

    asm.label("CheckAndHandleBrickRight");
    asm.cp_label("BRICK_RIGHT");
    asm.ret_cond(Condition::NZ);
    asm.raw("ld [hl], BLANK_TILE");
    asm.dec_label("hl");
    asm.raw("ld [hl], BLANK_TILE");
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
    asm.ld_a_label("c");
    asm.and_label("%11111000");
    asm.ld_l_label("a");
    asm.ld_h(0);
    asm.comment("Now we have the position * 8 in hl");
    asm.add_label("hl", "hl");
    asm.add_label("hl", "hl");
    asm.comment("Convert the X position to an offset.");
    asm.ld_a_label("b");
    asm.srl_label("a");
    asm.srl_label("a");
    asm.srl_label("a");
    asm.comment("Add the two offsets together.");
    asm.add_label("a", "l");
    asm.ld_l_label("a");
    asm.adc_label("h");
    asm.sub_label("a", "l");
    asm.ld_h_label("a");
    asm.comment("Add the offset to the tilemap's base address, and we are done!");
    asm.ld_bc_label("$9800");
    asm.add_label("hl", "bc");
    asm.ret();

    // IsWallTile function
    asm.comment("@param a: tile ID");
    asm.comment("@return z: set if a is a wall.");
    asm.label("IsWallTile");
    asm.cp_label("$00");
    asm.ret_cond(Condition::Z);
    asm.cp_label("$01");
    asm.ret_cond(Condition::Z);
    asm.cp_label("$02");
    asm.ret_cond(Condition::Z);
    asm.cp_label("$04");
    asm.ret_cond(Condition::Z);
    asm.cp_label("$05");
    asm.ret_cond(Condition::Z);
    asm.cp_label("$06");
    asm.ret_cond(Condition::Z);
    asm.cp_label("$07");
    asm.ret();

    // IncreaseScorePackedBCD function
    asm.comment("Increase score by 1 and store it as a 1 byte packed BCD number");
    asm.comment("changes A and HL");
    asm.label("IncreaseScorePackedBCD");
    asm.xor_label("a", "a");
    asm.inc_label("a");
    asm.ld_hl_label("wScore");
    asm.adc_label("[hl]");
    asm.daa();
    asm.ld_addr_label_a("[hl]");
    asm.call("UpdateScoreBoard");
    asm.ret();

    // UpdateScoreBoard function
    asm.label("UpdateScoreBoard");
    asm.ld_a_label("[wScore]");
    asm.and_label("%11110000");
    asm.swap_label("a");
    asm.add_label("a", "DIGIT_OFFSET");
    asm.ld_addr_label_a("[SCORE_TENS]");

    asm.ld_a_label("[wScore]");
    asm.and_label("%00001111");
    asm.add_label("a", "DIGIT_OFFSET");
    asm.ld_addr_label_a("[SCORE_ONES]");
    asm.ret();

    // Tiles data
    add_tiles(&mut asm);

    // Tilemap
    add_tilemap(&mut asm);

    // Sprites
    add_sprites(&mut asm);

    // WRAM sections
    asm.section("Counter", "WRAM0");
    asm.raw("wFrameCounter: db");

    asm.section("Input Variables", "WRAM0");
    asm.raw("wCurKeys: db");
    asm.raw("wNewKeys: db");

    asm.section("Ball Data", "WRAM0");
    asm.raw("wBallMomentumX: db");
    asm.raw("wBallMomentumY: db");

    asm.section("Score", "WRAM0");
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
