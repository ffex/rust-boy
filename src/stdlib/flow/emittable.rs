use crate::asm::{Instr, JumpTarget, LabelAllocator};

// `Emittable` and `boxed` live in the asm layer (`asm::block`), with `Block`; they are
// re-exported here, where the control-flow structures that implement them are
pub use crate::asm::{Emittable, boxed};

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
    fn emit(&mut self, _labels: &LabelAllocator) -> Vec<Instr> {
        let mut instrs = std::mem::take(&mut self.args);
        instrs.push(Instr::Call {
            target: JumpTarget::Label(self.func_name.clone()),
        });
        instrs
    }
}
