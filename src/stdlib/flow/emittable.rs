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
    ///
    /// `args` loads the registers expected by the callee, for example `a` and `de`.
    /// The setup is emitted immediately before this call; no stack arguments or
    /// implicit conversions are added. Routines declare inputs with `Routine::with_reads`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::{Block, R8, R16};

    #[test]
    fn every_call_emits_its_register_setup_immediately_before_the_call() {
        let labels = LabelAllocator::new();
        let mut actual = Vec::new();
        for (a, de) in [(0x12, 0x3456), (0x78, 0x9ABC)] {
            let mut setup = Block::new();
            setup.ld(R8::A, a).ld(R16::DE, de);
            actual.extend(Call::with_args("Receive", setup.into_instrs()).emit(&labels));
        }
        let actual: Vec<_> = actual.iter().map(ToString::to_string).collect();
        assert_eq!(
            actual,
            [
                "ld a, 18",
                "ld de, 13398",
                "call Receive",
                "ld a, 120",
                "ld de, 39612",
                "call Receive"
            ]
        );
    }
}
