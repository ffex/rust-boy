use rust_boy::GbGen;

fn main() {
    let mut generator: GbGen = GbGen::new();
    generator.include_hardware();

    generator.def("BRICK_LEFT", 0x05);
    generator.def("BRICK_RIGHT", 0x06);
    generator.def("BLANK_TILE", 0x08);
    generator.def("DIGIT_OFFSET", 0x1A);
    generator.def("SCORE_TENS", 0x9870);
    generator.def("SCORE_ONES", 0x9871);

    generator.section("Header", "ROM0[$100]");
    generator.jp("EntryPoint");
    generator.ds("$150 - @", "0");
    generator.label("EntryPoint");

    generator.label("WaitVBlank");
    generator.ld_a_str("[rLY]");
    generator.cp(144);
    generator.jp_cond("c", "WaitVBlank");

    // Turn off LCD
    generator.ld_a(0);
    generator.ld_addr_str_a("[rLCDC]");

    // Copy tiles data
    generator.ld_de_str("Tiles");
    generator.ld_hl_str("$9000");
    generator.ld_bc_str("TilesEnd - Tiles");
    generator.call("Memcopy");

    // Copy the tilemap
    generator.ld_de_str("Tilemap");
    generator.ld_hl_str("$9800");
    generator.ld_bc_str("TilemapEnd - Tilemap");
    generator.call("Memcopy");

    // Copy the paddle tile
    generator.ld_de_str("Paddle");
    generator.ld_hl_str("$8000");
    generator.ld_bc_str("PaddleEnd - Paddle");
    generator.call("Memcopy");

    // Copy the balltile
    generator.ld_de_str("Ball");
    generator.ld_hl_str("$8010");
    generator.ld_bc_str("BallEnd - Ball");
    generator.call("Memcopy");

    // initialize OAM
    generator.ld_a(0);
    generator.ld_b(160);
    generator.ld_hl_str("_OAMRAM");

    // Clear OAM loop
    generator.label("ClearOam");
    generator.ld_hl_i_str("a");
    generator.dec("b");
    generator.jp_cond("nz", "ClearOam");

    // Draw object in OAM - paddle
    generator.ld_hl_str("_OAMRAM");
    generator.ld_a(128 + 16);
    generator.ld_hl_i_str("a");
    generator.ld_a(16 + 8);
    generator.ld_hl_i_str("a");
    generator.ld_a(0);
    generator.ld_hl_i_str("a");
    generator.ld_hl_i_str("a");

    // Draw object in OAM - ball
    generator.ld_a(100 + 16);
    generator.ld_hl_i_str("a");
    generator.ld_a(32 + 8);
    generator.ld_hl_i_str("a");
    generator.ld_a(1);
    generator.ld_hl_i_str("a");
    generator.ld_a(0);
    generator.ld_hl_i_str("a");

    generator.ld_a(1);
    generator.ld_addr_str_a("[wBallMomentumX]");
    generator.ld_a_str("-1");
    generator.ld_addr_str_a("[wBallMomentumY]");

    // Turn LCD On
    generator.ld_a_str("LCDCF_ON | LCDCF_BGON | LCDCF_OBJON");
    generator.ld_addr_str_a("[rLCDC]");

    // during first frame initialize register!
    generator.ld_a_str("%11100100");
    generator.ld_addr_str_a("[rBGP]");
    generator.ld_a_str("%11100100");
    generator.ld_addr_str_a("[rOBP0]");

    // initialize global variables
    generator.ld_a(0);
    generator.ld_addr_str_a("[wFrameCounter]");
    generator.ld_addr_str_a("[wCurKeys]");
    generator.ld_addr_str_a("[wNewKeys]");
    generator.ld_addr_str_a("[wScore]");

    // Main loop
    generator.label("Main");
    generator.comment("Wait until it's *not* VBlank");
    generator.ld_a_str("[rLY]");
    generator.cp(144);
    generator.jp_cond("nc", "Main");

    generator.label("WaitVBlank2");
    generator.ld_a_str("[rLY]");
    generator.cp(144);
    generator.jp_cond("c", "WaitVBlank2");

    // Add the ball's momentum to its position in OAM
    generator.ld_a_str("[wBallMomentumX]");
    generator.ld_b_str("a");
    generator.ld_a_str("[_OAMRAM +5]");
    generator.add("a", "b");
    generator.ld_addr_str_a("[_OAMRAM +5]");

    generator.ld_a_str("[wBallMomentumY]");
    generator.ld_b_str("a");
    generator.ld_a_str("[_OAMRAM +4]");
    generator.add("a", "b");
    generator.ld_addr_str_a("[_OAMRAM +4]");

    // BounceOnTop
    generator.label("BounceOnTop");
    generator.comment("Remember to offset the OAM position!");
    generator.comment("(8, 16) in OAM coordinates is (0, 0) on the screen.");
    generator.ld_a_str("[_OAMRAM + 4]");
    generator.sub("a", "16 + 1");
    generator.ld_c_str("a");
    generator.ld_a_str("[_OAMRAM + 5]");
    generator.sub("a", "8");
    generator.ld_b_str("a");
    generator.call("GetTileByPixel");
    generator.ld_a_str("[hl]");
    generator.call("IsWallTile");
    generator.jp_cond("nz", "BounceOnRight");
    generator.call("CheckAndHandleBrick");
    generator.ld_a(1);
    generator.ld_addr_str_a("[wBallMomentumY]");

    // BounceOnRight
    generator.label("BounceOnRight");
    generator.ld_a_str("[_OAMRAM + 4]");
    generator.sub("a", "16");
    generator.ld_c_str("a");
    generator.ld_a_str("[_OAMRAM + 5]");
    generator.sub("a", "8 - 1");
    generator.ld_b_str("a");
    generator.call("GetTileByPixel");
    generator.ld_a_str("[hl]");
    generator.call("IsWallTile");
    generator.jp_cond("nz", "BounceOnLeft");
    generator.ld_a_str("-1");
    generator.ld_addr_str_a("[wBallMomentumX]");

    // BounceOnLeft
    generator.label("BounceOnLeft");
    generator.ld_a_str("[_OAMRAM + 4]");
    generator.sub("a", "16");
    generator.ld_c_str("a");
    generator.ld_a_str("[_OAMRAM + 5]");
    generator.sub("a", "8 + 1");
    generator.ld_b_str("a");
    generator.call("GetTileByPixel");
    generator.ld_a_str("[hl]");
    generator.call("IsWallTile");
    generator.jp_cond("nz", "BounceOnBottom");
    generator.ld_a(1);
    generator.ld_addr_str_a("[wBallMomentumX]");

    // BounceOnBottom
    generator.label("BounceOnBottom");
    generator.ld_a_str("[_OAMRAM + 4]");
    generator.sub("a", "16 - 1");
    generator.ld_c_str("a");
    generator.ld_a_str("[_OAMRAM + 5]");
    generator.sub("a", "8");
    generator.ld_b_str("a");
    generator.call("GetTileByPixel");
    generator.ld_a_str("[hl]");
    generator.call("IsWallTile");
    generator.jp_cond("nz", "BounceDone");
    generator.ld_a_str("-1");
    generator.ld_addr_str_a("[wBallMomentumY]");

    generator.label("BounceDone");
    generator.comment("First, check if the ball is low enough to bounce off the paddle.");
    generator.ld_a_str("[_OAMRAM]");
    generator.ld_b_str("a");
    generator.ld_a_str("[_OAMRAM + 4]");
    generator.add("a", "5");
    generator.cp_str("b");
    generator.jp_cond("nz", "PaddleBounceDone");

    generator
        .comment("Now let's compare the X positions of the objects to see if they're touching.");
    generator.ld_a_str("[_OAMRAM + 5]");
    generator.ld_b_str("a");
    generator.ld_a_str("[_OAMRAM + 1]");
    generator.sub("a", "8");
    generator.cp_str("b");
    generator.jp_cond("nc", "PaddleBounceDone");
    generator.add("a", "8 + 16");
    generator.cp_str("b");
    generator.jp_cond("c", "PaddleBounceDone");

    generator.ld_a_str("-1");
    generator.ld_addr_str_a("[wBallMomentumY]");

    generator.label("PaddleBounceDone");

    generator.call("UpdateKeys");

    // First check if the left button is pressed
    generator.label("CheckLeft");
    generator.ld_a_str("[wCurKeys]");
    generator.and("PADF_LEFT");
    generator.jp_cond("z", "CheckRight");

    generator.label("Left");
    generator.comment("move the paddle one pixel to the left");
    generator.ld_a_str("[_OAMRAM+1]");
    generator.dec("a");
    generator.cp_str("15");
    generator.jp_cond("z", "Main");
    generator.ld_addr_str_a("[_OAMRAM+1]");
    generator.jp("Main");

    generator.label("CheckRight");
    generator.ld_a_str("[wCurKeys]");
    generator.and("PADF_RIGHT");
    generator.jp_cond("z", "Main");

    generator.label("Right");
    generator.comment("move the paddle one pixel to the left");
    generator.ld_a_str("[_OAMRAM+1]");
    generator.inc("a");
    generator.cp_str("105");
    generator.jp_cond("z", "Main");
    generator.ld_addr_str_a("[_OAMRAM+1]");
    generator.jp("Main");

    // Memcopy function
    generator.comment("Copy bytes from one area to another");
    generator.comment("@param de: source");
    generator.comment("@param hl: destination");
    generator.comment("@param bc: lenght");
    generator.label("Memcopy");
    generator.ld_a_str("[de]");
    generator.ld_hl_i_str("a");
    generator.inc("de");
    generator.dec("bc");
    generator.ld_a_str("b");
    generator.or("a", "c");
    generator.jp_cond("nz", "Memcopy");
    generator.ret();

    // UpdateKeys function
    generator.label("UpdateKeys");
    generator.comment("poll hald the controller");
    generator.ld_a_str("P1F_GET_BTN");
    generator.call(".onenibble");
    generator.ld_b_str("a");

    generator.comment("poll the other half");
    generator.ld_a_str("P1F_GET_DPAD");
    generator.call(".onenibble");
    generator.swap("a");
    generator.xor("a", "b");
    generator.ld_b_str("a");

    generator.comment("And release the controller");
    generator.ld_a_str("P1F_GET_NONE");
    generator.ldh("[rP1]", "a");

    generator.comment("Combine with previous wCurKeys to make wNew Keys");
    generator.ld_a_str("[wCurKeys]");
    generator.xor("a", "b");
    generator.and("b");
    generator.ld_addr_str_a("[wNewKeys]");
    generator.ld_a_str("b");
    generator.ld_addr_str_a("[wCurKeys]");

    generator.label(".onenibble");
    generator.ldh("[rP1]", "a");
    generator.call(".knowret");
    generator.ldh("a", "[rP1]");
    generator.ldh("a", "[rP1]");
    generator.ldh("a", "[rP1]");
    generator.or("a", "$F0");

    generator.label(".knowret");
    generator.ret();

    // CheckAndHandleBrick function
    generator.comment("check if a brick was collided with and breaks if it is possible");
    generator.comment("@param hl: address of the tile");
    generator.label("CheckAndHandleBrick");
    generator.ld_a_str("[hl]");
    generator.cp_str("BRICK_LEFT");
    generator.jr("nz", "CheckAndHandleBrickRight");
    generator.comment("break from left side");
    generator.ld_addr_str_a("[hl]");
    generator.raw("ld [hl], BLANK_TILE");
    generator.inc("hl");
    generator.raw("ld [hl], BLANK_TILE");
    generator.call("IncreaseScorePackedBCD");

    generator.label("CheckAndHandleBrickRight");
    generator.cp_str("BRICK_RIGHT");
    generator.ret_cond("nz");
    generator.raw("ld [hl], BLANK_TILE");
    generator.dec("hl");
    generator.raw("ld [hl], BLANK_TILE");
    generator.call("IncreaseScorePackedBCD");
    generator.ret();

    // GetTileByPixel function
    generator.comment("Convert a pixel position to a tilemap address");
    generator.comment("hl = $9800 + X + Y * 32");
    generator.comment("@param b: X");
    generator.comment("@param c: Y");
    generator.comment("@return hl: tile address");
    generator.label("GetTileByPixel");
    generator
        .comment("First, we need to divide by 8 to convert a pixel position to a tile position.");
    generator.comment("After this we want to multiply the Y position by 32.");
    generator
        .comment("These operations effectively cancel out so we only need to mask the Y value.");
    generator.ld_a_str("c");
    generator.and("%11111000");
    generator.ld_l_str("a");
    generator.ld_h(0);
    generator.comment("Now we have the position * 8 in hl");
    generator.add("hl", "hl");
    generator.add("hl", "hl");
    generator.comment("Convert the X position to an offset.");
    generator.ld_a_str("b");
    generator.srl("a");
    generator.srl("a");
    generator.srl("a");
    generator.comment("Add the two offsets together.");
    generator.add("a", "l");
    generator.ld_l_str("a");
    generator.adc("h");
    generator.sub("a", "l");
    generator.ld_h_str("a");
    generator.comment("Add the offset to the tilemap's base address, and we are done!");
    generator.ld_bc_str("$9800");
    generator.add("hl", "bc");
    generator.ret();

    // IsWallTile function
    generator.comment("@param a: tile ID");
    generator.comment("@return z: set if a is a wall.");
    generator.label("IsWallTile");
    generator.cp_str("$00");
    generator.ret_cond("z");
    generator.cp_str("$01");
    generator.ret_cond("z");
    generator.cp_str("$02");
    generator.ret_cond("z");
    generator.cp_str("$04");
    generator.ret_cond("z");
    generator.cp_str("$05");
    generator.ret_cond("z");
    generator.cp_str("$06");
    generator.ret_cond("z");
    generator.cp_str("$07");
    generator.ret();

    // IncreaseScorePackedBCD function
    generator.comment("Increase score by 1 and store it as a 1 byte packed BCD number");
    generator.comment("changes A and HL");
    generator.label("IncreaseScorePackedBCD");
    generator.xor("a", "a");
    generator.inc("a");
    generator.ld_hl_str("wScore");
    generator.adc("[hl]");
    generator.daa();
    generator.ld_addr_str_a("[hl]");
    generator.call("UpdateScoreBoard");
    generator.ret();

    // UpdateScoreBoard function
    generator.label("UpdateScoreBoard");
    generator.ld_a_str("[wScore]");
    generator.and("%11110000");
    generator.swap("a");
    generator.add("a", "DIGIT_OFFSET");
    generator.ld_addr_str_a("[SCORE_TENS]");

    generator.ld_a_str("[wScore]");
    generator.and("%00001111");
    generator.add("a", "DIGIT_OFFSET");
    generator.ld_addr_str_a("[SCORE_ONES]");
    generator.ret();

    // Tiles data
    generator.label("Tiles");
    // Tile 0
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`33322222");
    generator.dw("`33322222");
    generator.dw("`33322222");
    generator.dw("`33322211");
    generator.dw("`33322211");
    // Tile 1
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`11111111");
    generator.dw("`11111111");
    // Tile 2
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`22222333");
    generator.dw("`22222333");
    generator.dw("`22222333");
    generator.dw("`11222333");
    generator.dw("`11222333");
    // Tile 3
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`33333333");
    generator.dw("`33333333");
    // Tile 4
    generator.dw("`33322211");
    generator.dw("`33322211");
    generator.dw("`33322211");
    generator.dw("`33322211");
    generator.dw("`33322211");
    generator.dw("`33322211");
    generator.dw("`33322211");
    generator.dw("`33322211");
    // Tile 5
    generator.dw("`22222222");
    generator.dw("`20000000");
    generator.dw("`20111111");
    generator.dw("`20111111");
    generator.dw("`20111111");
    generator.dw("`20111111");
    generator.dw("`22222222");
    generator.dw("`33333333");
    // Tile 6
    generator.dw("`22222223");
    generator.dw("`00000023");
    generator.dw("`11111123");
    generator.dw("`11111123");
    generator.dw("`11111123");
    generator.dw("`11111123");
    generator.dw("`22222223");
    generator.dw("`33333333");
    // Tile 7
    generator.dw("`11222333");
    generator.dw("`11222333");
    generator.dw("`11222333");
    generator.dw("`11222333");
    generator.dw("`11222333");
    generator.dw("`11222333");
    generator.dw("`11222333");
    generator.dw("`11222333");
    // Tile 8
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    // Tile 9
    generator.dw("`11001100");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`21212121");
    generator.dw("`22222222");
    generator.dw("`22322232");
    generator.dw("`23232323");
    generator.dw("`33333333");
    // Logo start (paste your logo here!)
    // Tile 10
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222211");
    generator.dw("`22222211");
    generator.dw("`22222211");
    // Tile 11
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`11221111");
    generator.dw("`11221111");
    generator.dw("`11000011");
    // Tile 12
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`11222222");
    generator.dw("`11222222");
    generator.dw("`11222222");
    // Tile 13
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    // Tile 14
    generator.dw("`22222211");
    generator.dw("`22222200");
    generator.dw("`22222200");
    generator.dw("`22000000");
    generator.dw("`22000000");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    // Tile 15
    generator.dw("`11000011");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`11000022");
    // Tile 16
    generator.dw("`11222222");
    generator.dw("`11222222");
    generator.dw("`11222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    // Tile 17
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    // Tile 18
    generator.dw("`22222222");
    generator.dw("`22222200");
    generator.dw("`22222200");
    generator.dw("`22222211");
    generator.dw("`22222211");
    generator.dw("`22221111");
    generator.dw("`22221111");
    generator.dw("`22221111");
    // Tile 19
    generator.dw("`11000022");
    generator.dw("`00112222");
    generator.dw("`00112222");
    generator.dw("`11112200");
    generator.dw("`11112200");
    generator.dw("`11220000");
    generator.dw("`11220000");
    generator.dw("`11220000");
    // Tile 20
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22000000");
    generator.dw("`22000000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    // Tile 21
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`22222222");
    generator.dw("`11110022");
    generator.dw("`11110022");
    generator.dw("`11110022");
    // Tile 22
    generator.dw("`22221111");
    generator.dw("`22221111");
    generator.dw("`22221111");
    generator.dw("`22221111");
    generator.dw("`22221111");
    generator.dw("`22222211");
    generator.dw("`22222211");
    generator.dw("`22222222");
    // Tile 23
    generator.dw("`11220000");
    generator.dw("`11110000");
    generator.dw("`11110000");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`22222222");
    // Tile 24
    generator.dw("`00000000");
    generator.dw("`00111111");
    generator.dw("`00111111");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`11111111");
    generator.dw("`22222222");
    // Tile 25
    generator.dw("`11110022");
    generator.dw("`11000022");
    generator.dw("`11000022");
    generator.dw("`00002222");
    generator.dw("`00002222");
    generator.dw("`00222222");
    generator.dw("`00222222");
    generator.dw("`22222222");
    // Digits
    // 0
    generator.dw("`33333333");
    generator.dw("`33000033");
    generator.dw("`30033003");
    generator.dw("`30033003");
    generator.dw("`30033003");
    generator.dw("`30033003");
    generator.dw("`33000033");
    generator.dw("`33333333");
    // 1
    generator.dw("`33333333");
    generator.dw("`33300333");
    generator.dw("`33000333");
    generator.dw("`33300333");
    generator.dw("`33300333");
    generator.dw("`33300333");
    generator.dw("`33000033");
    generator.dw("`33333333");
    // 2
    generator.dw("`33333333");
    generator.dw("`33000033");
    generator.dw("`30330003");
    generator.dw("`33330003");
    generator.dw("`33000333");
    generator.dw("`30003333");
    generator.dw("`30000003");
    generator.dw("`33333333");
    // 3
    generator.dw("`33333333");
    generator.dw("`30000033");
    generator.dw("`33330003");
    generator.dw("`33000033");
    generator.dw("`33330003");
    generator.dw("`33330003");
    generator.dw("`30000033");
    generator.dw("`33333333");
    // 4
    generator.dw("`33333333");
    generator.dw("`33000033");
    generator.dw("`30030033");
    generator.dw("`30330033");
    generator.dw("`30330033");
    generator.dw("`30000003");
    generator.dw("`33330033");
    generator.dw("`33333333");
    // 5
    generator.dw("`33333333");
    generator.dw("`30000033");
    generator.dw("`30033333");
    generator.dw("`30000033");
    generator.dw("`33330003");
    generator.dw("`30330003");
    generator.dw("`33000033");
    generator.dw("`33333333");
    // 6
    generator.dw("`33333333");
    generator.dw("`33000033");
    generator.dw("`30033333");
    generator.dw("`30000033");
    generator.dw("`30033003");
    generator.dw("`30033003");
    generator.dw("`33000033");
    generator.dw("`33333333");
    // 7
    generator.dw("`33333333");
    generator.dw("`30000003");
    generator.dw("`33333003");
    generator.dw("`33330033");
    generator.dw("`33300333");
    generator.dw("`33000333");
    generator.dw("`33000333");
    generator.dw("`33333333");
    // 8
    generator.dw("`33333333");
    generator.dw("`33000033");
    generator.dw("`30333003");
    generator.dw("`33000033");
    generator.dw("`30333003");
    generator.dw("`30333003");
    generator.dw("`33000033");
    generator.dw("`33333333");
    // 9
    generator.dw("`33333333");
    generator.dw("`33000033");
    generator.dw("`30330003");
    generator.dw("`30330003");
    generator.dw("`33000003");
    generator.dw("`33330003");
    generator.dw("`33000033");
    generator.dw("`33333333");
    generator.label("TilesEnd");

    generator.label("Tilemap");
    generator.db("$00, $01, $01, $01, $01, $01, $01, $01, $01, $01, $01, $01, $01, $02, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $05, $06, $05, $06, $05, $06, $05, $06, $05, $06, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $0A, $0B, $0C, $0D, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $0E, $0F, $10, $11, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $12, $13, $14, $15, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $08, $07, $03, $16, $17, $18, $19, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.db("$04, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $09, $07, $03, $03, $03, $03, $03, $03, 0,0,0,0,0,0,0,0,0,0,0,0,");
    generator.label("TilemapEnd");

    // Paddle sprite
    generator.label("Paddle");
    generator.dw("`13333331");
    generator.dw("`30000003");
    generator.dw("`13333331");
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.label("PaddleEnd");

    // Ball sprite
    generator.label("Ball");
    generator.dw("`00033000");
    generator.dw("`00322300");
    generator.dw("`03222230");
    generator.dw("`03222230");
    generator.dw("`00322300");
    generator.dw("`00033000");
    generator.dw("`00000000");
    generator.dw("`00000000");
    generator.label("BallEnd");

    // WRAM sections
    generator.section("Counter", "WRAM0");
    generator.raw("wFrameCounter: db");

    generator.section("Input Variables", "WRAM0");
    generator.raw("wCurKeys: db");
    generator.raw("wNewKeys: db");

    generator.section("Ball Data", "WRAM0");
    generator.raw("wBallMomentumX: db");
    generator.raw("wBallMomentumY: db");

    generator.section("Score", "WRAM0");
    generator.raw("wScore: db");

    // Output the generated assembly
    println!("{}", generator.output());
}
