use crate::gb_asm::{Instr, JumpTarget};

/// Trait for anything that can emit assembly instructions.
///
/// This provides a unified interface for both raw instructions and
/// control flow structures like If statements.
pub trait Emittable {
    /// Emit assembly instructions, using the counter for generating unique labels.
    fn emit(&mut self, counter: &mut usize) -> Vec<Instr>;
}

/// A function call with optional argument setup instructions.
///
/// This is useful when you need to pass a function call as an `Emittable`,
/// for example as the first argument to `IfConst` or `IfA`.
///
/// # Example
/// ```ignore
/// IfConst::eq(
///     Call::with_args("GetTileByPixel", sprite.get_pivot(ball, 0, 1)),
///     "BRICK_LEFT",
///     handle_brick,
/// )
/// ```
pub struct Call {
    func_name: String,
    args: Vec<Instr>,
}

impl Call {
    /// Create a function call with argument setup instructions.
    pub fn with_args(func_name: &str, args: Vec<Instr>) -> Self {
        Self {
            func_name: func_name.to_string(),
            args,
        }
    }

    /// Create a simple function call without arguments.
    pub fn new(func_name: &str) -> Self {
        Self {
            func_name: func_name.to_string(),
            args: Vec::new(),
        }
    }
}

impl Emittable for Call {
    fn emit(&mut self, _counter: &mut usize) -> Vec<Instr> {
        let mut instrs = std::mem::take(&mut self.args);
        instrs.push(Instr::Call {
            target: JumpTarget::Label(self.func_name.clone()),
        });
        instrs
    }
}

/// Implementation for raw instruction vectors - just returns the instructions.
impl Emittable for Vec<Instr> {
    fn emit(&mut self, _counter: &mut usize) -> Vec<Instr> {
        std::mem::take(self)
    }
}

/// Implementation for nested instruction vectors - flattens them.
/// This allows combining multiple instruction sequences cleanly:
/// ```ignore
/// vec![
///     TileRef::set_tile_label("BLANK_TILE"),
///     TileRef::next_tile(),
///     TileRef::set_tile_label("BLANK_TILE"),
/// ]
/// ```
impl Emittable for Vec<Vec<Instr>> {
    fn emit(&mut self, _counter: &mut usize) -> Vec<Instr> {
        std::mem::take(self).into_iter().flatten().collect()
    }
}

/// Implementation for boxed trait objects - allows mixing different Emittable types.
/// ```ignore
/// vec![
///     boxed(IfConst::eq(...)),
///     boxed(IfA::eq(...)),
/// ]
/// ```
impl Emittable for Vec<Box<dyn Emittable>> {
    fn emit(&mut self, counter: &mut usize) -> Vec<Instr> {
        self.iter_mut().flat_map(|e| e.emit(counter)).collect()
    }
}

/// Helper to box an Emittable for use in heterogeneous vectors.
///
/// # Example
/// ```ignore
/// gb.define_function_from("MyFunc", vec![
///     boxed(IfConst::eq(value, "CONST", body1)),
///     boxed(IfA::eq("OTHER", body2)),
/// ]);
/// ```
pub fn boxed(e: impl Emittable + 'static) -> Box<dyn Emittable> {
    Box::new(e)
}
