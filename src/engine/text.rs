//! Font loading and resumable string/score printers.
use super::{ArrayElement, Chunk, RustBoy, TileSource, TilemapArea, Var, WramArray};
use crate::asm::{Block, Condition, Instr, LabelAllocator, R8, R16};
use crate::stdlib::graphics::text;
pub use text::Font;

/// Persistent print job. Use one main-loop producer; `step` changes all registers.
#[derive(Debug, Clone)]
pub struct TextPrinter {
    routine: String,
    cursor: Var,
    len: u16,
}
impl TextPrinter {
    /// Queue as much as fits. Carry set means retry next frame; clear means complete.
    /// The VBlank handler flushes queued glyphs. Never call from an interrupt handler.
    pub fn step(&self) -> Vec<Instr> {
        let mut code = Block::new();
        code.call(&self.routine);
        code.into_instrs()
    }
    /// Restart from the first glyph (already queued writes are retained).
    pub fn reset(&self) -> Vec<Instr> {
        self.cursor.set(0)
    }
    /// The number of glyphs the job prints.
    pub fn len(&self) -> u16 {
        self.len
    }
    /// Whether the job prints nothing (an empty string).
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
/// Fixed-width score format, with leading zeroes.
#[derive(Debug, Clone, Copy)]
pub enum NumberFormat {
    /// DE as an unsigned 16-bit number: five decimal digits (00000 to 65535)
    DecimalU16,
    /// DE as four packed-BCD digits (D = the high two); a nibble above 9 is rejected
    PackedBcd,
}
/// A mutable print job whose tile buffer is filled from DE by `set_value`.
#[derive(Debug, Clone)]
pub struct NumberPrinter {
    printer: TextPrinter,
    buffer: WramArray,
    routine: &'static str,
    zero: u8,
}
impl NumberPrinter {
    /// DE is the new value. Encodes it and restarts printing. Invalid BCD leaves the
    /// previous buffer/cursor unchanged and returns carry. Changes all registers.
    /// Call only from init/main code, then call `step` each frame until complete.
    pub fn set_value(&self, labels: &LabelAllocator) -> Vec<Instr> {
        let end = labels.local("number");
        let mut code = Block::new();
        code.ld(R16::HL, self.buffer.name())
            .ld(R8::C, self.zero)
            .call(self.routine)
            .jr_cond(Condition::C, &end);
        code.emit_all(self.printer.reset());
        code.and(R8::A).label(&end);
        code.into_instrs()
    }
    /// Queue as much of the score as fits; carry set means retry next frame (see
    /// [`TextPrinter::step`]). Never call from an interrupt handler.
    pub fn step(&self) -> Vec<Instr> {
        self.printer.step()
    }
}
impl RustBoy {
    /// Load a contiguous single-tile ASCII font into the background tile region.
    /// Glyphs follow source order; unsupported characters panic when creating a job.
    ///
    /// # Panics
    /// If the source has no tiles, more than 255, or glyphs past ASCII 127, or the tiles do
    /// not fit the background tile region or `name` cannot be a label.
    #[track_caller]
    pub fn load_font(&mut self, name: &str, source: TileSource, first: u8) -> Font {
        let count = u16::try_from(source.tile_count()).expect("too many font glyphs");
        Font::new(first, count, 0);
        let id = self.tiles.add_background(name, source);
        let tile = ((self.tiles.get_address(id).unwrap() - crate::hw::VRAM_BG_TILES)
            / crate::hw::TILE_SIZE) as u8;
        Font::new(first, count, tile)
    }
    #[track_caller]
    fn check_text_name(&self, name: &str) {
        super::sprites::check_label("text", name);
        assert!(
            !self.function_exists(&format!("PrintText{name}")),
            "duplicate text printer {name}"
        );
        for reserved in [format!("wText{name}Cursor"), format!("wText{name}Glyphs")] {
            assert!(
                !self.vars.names().any(|(existing, _)| existing == reserved),
                "duplicate text storage {reserved}"
            );
        }
    }
    fn text_job(&mut self, name: &str, writes: &[(u16, u8)], glyphs: &str) -> TextPrinter {
        let cursor_name = format!("wText{name}Cursor");
        let targets = format!("Text{name}Targets");
        let routine = format!("PrintText{name}");
        super::sprites::check_label("text", name);
        assert!(
            !self.function_exists(&routine),
            "duplicate text printer {name}"
        );
        let cursor = self.vars.create_u16(&cursor_name, 0);
        self.raw(|asm| {
            asm.chunk(Chunk::Tiles).label(&targets);
            if !writes.is_empty() {
                asm.dw(&writes
                    .iter()
                    .map(|(address, _)| format!("${address:04X}"))
                    .collect::<Vec<_>>()
                    .join(", "));
            }
        });
        self.queue_vram_write_from_registers();
        self.call_routine(&text::print(
            &routine,
            &targets,
            glyphs,
            &cursor_name,
            writes.len() as u16,
        ));
        TextPrinter {
            routine,
            cursor,
            len: writes.len() as u16,
        }
    }
    /// Create a string job. Newlines/wrapping return to the original X column; text
    /// must fit the 32×32 map. Call `step` every frame: long strings resume automatically.
    /// Queued writes use LCD-on VBlank flushing, including jobs started during init.
    ///
    /// # Panics
    /// If `name` cannot be a label or is another printer's, the text does not fit the map
    /// from (`x`, `y`), or `font` has no glyph for one of its characters.
    #[track_caller]
    pub fn create_text_printer(
        &mut self,
        name: &str,
        font: Font,
        area: TilemapArea,
        x: u8,
        y: u8,
        value: &str,
    ) -> TextPrinter {
        self.check_text_name(name);
        let writes = font.layout(area.address(), x, y, value);
        let glyphs = format!("Text{name}Glyphs");
        let printer = self.text_job(name, &writes, &glyphs);
        self.raw(|asm| {
            asm.chunk(Chunk::Tiles).label(&glyphs);
            if !writes.is_empty() {
                asm.db(&writes
                    .iter()
                    .map(|(_, glyph)| glyph.to_string())
                    .collect::<Vec<_>>()
                    .join(", "));
            }
        });
        printer
    }
    /// Create a five-digit unsigned or four-digit packed-BCD score at tile coordinates.
    /// The font must include contiguous '0' through '9'. Initially displays zeroes.
    ///
    /// # Panics
    /// If `name` cannot be a label or is another printer's, the score does not fit the map
    /// from (`x`, `y`), or `font` has no glyph for a digit.
    #[track_caller]
    pub fn create_number_printer(
        &mut self,
        name: &str,
        font: Font,
        area: TilemapArea,
        x: u8,
        y: u8,
        format: NumberFormat,
    ) -> NumberPrinter {
        self.check_text_name(name);
        let zero = font.glyph('0');
        font.glyph('9');
        let (digits, formatter) = match format {
            NumberFormat::DecimalU16 => ("00000", text::decimal()),
            NumberFormat::PackedBcd => ("0000", text::bcd()),
        };
        let writes = font.layout(area.address(), x, y, digits);
        let buffer = self.vars.create_array(
            &format!("wText{name}Glyphs"),
            ArrayElement::U8,
            digits.len() as u16,
            i32::from(zero),
        );
        let printer = self.text_job(name, &writes, buffer.name());
        self.call_routine(&formatter);
        NumberPrinter {
            printer,
            buffer,
            routine: match format {
                NumberFormat::DecimalU16 => "EncodeDecimalU16",
                NumberFormat::PackedBcd => "EncodePackedBcd",
            },
            zero,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Error, panic_message};
    #[test]
    fn jobs_are_deterministic_and_handlers_reject_the_producer() {
        let mut game = RustBoy::new();
        let font = game.load_font("Digits", TileSource::from_raw(&[["$00"; 8]; 10]), b'0');
        let printer = game.create_text_printer("Empty", font, TilemapArea::Map9800, 0, 0, "");
        assert!(printer.is_empty());
        game.add_to_main_loop(printer.step());
        let asm = game.build().unwrap();
        assert_eq!(asm, game.build().unwrap());
        assert!(
            !panic_message(|| game.create_number_printer(
                "Empty",
                font,
                TilemapArea::Map9800,
                0,
                0,
                NumberFormat::DecimalU16
            ))
            .is_empty()
        );
        assert_eq!(asm, game.build().unwrap());
        game.on_vblank(printer.step());
        assert!(matches!(game.build(), Err(Error::MainLoopOnly { .. })));
    }
    #[test]
    fn rejected_layout_does_not_allocate_storage() {
        let mut game = RustBoy::new();
        let font = Font::new(b'0', 10, 0);
        let before = game.build().unwrap();
        assert!(
            !panic_message(|| game.create_text_printer(
                "Bad",
                font,
                TilemapArea::Map9800,
                31,
                31,
                "00"
            ))
            .is_empty()
        );
        assert_eq!(before, game.build().unwrap());
    }
}
