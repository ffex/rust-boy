//! What the integration tests share: the example programs and the assembly they print,
//! building a ROM with RGBDS, and a headless Game Boy to run it on.
//!
//! Each test file includes it with `mod support;` and uses part of it.
#![allow(dead_code)]

pub mod examples;
pub mod gameboy;
pub mod rom;
