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
    generator.ld_a_str("[rLCDC]");

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
}
