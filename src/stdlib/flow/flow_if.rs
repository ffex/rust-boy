use crate::asm::{
    Block, Condition as AsmCondition, Expr, Instr, JumpTarget, LabelAllocator, R8, R16Stack,
};
use crate::stdlib::routine::Regs;

use super::emittable::Emittable;

/// Comparison operators for conditions
///
/// IMPORTANT NOTE about LE and GT:
/// The Game Boy CPU only has Z (zero) and C (carry) flags after a compare.
/// After `cp B` (comparing A with B; in `If`, A = left and B = right):
/// - E  (A == B): Z flag set
/// - NE (A != B): Z flag clear
/// - LT (A < B):  C flag set
/// - GE (A >= B): C flag clear
/// - LE (A <= B): C flag set OR Z flag set (requires two checks)
/// - GT (A > B):  C flag clear AND Z flag clear (requires two checks)
#[derive(Clone, Debug)]
pub enum ComparisonOp {
    E,  // == (equal)
    NE, // != (not equal)
    LT, // <  (less than)
    GE, // >= (greater or equal)
    LE, // <= (less or equal)
    GT, // >  (greater than)
}

impl ComparisonOp {
    /// Get the inverted condition for jumping AWAY from the then branch.
    /// This is used to skip the then branch when the condition is FALSE.
    fn inverted_asm_condition(&self) -> AsmCondition {
        match self {
            ComparisonOp::E => AsmCondition::NZ,  // Skip then if not equal
            ComparisonOp::NE => AsmCondition::Z,  // Skip then if equal
            ComparisonOp::LT => AsmCondition::NC, // Skip then if >= (not less than)
            ComparisonOp::GE => AsmCondition::C,  // Skip then if < (not greater/equal)
            // LE and GT need special multi-check handling, these are placeholders
            ComparisonOp::LE => AsmCondition::NC,
            ComparisonOp::GT => AsmCondition::C,
        }
    }
}

/// High-level If statement that hides register management.
///
/// `If::lt(left, right, body)` runs `body` when `left < right` (unsigned compare);
/// every operator reads the same way.
///
/// The If statement automatically handles:
/// - Loading right value into A, saving to B
/// - Loading left value into A
/// - Comparing A (left) with B (right)
/// - Conditional jumps and label generation
///
/// # Registers
/// The `If` uses `a`, `b` and the flags ([`If::clobbers`]): the right code runs first and
/// leaves its value in `a`, which the `If` copies to `b`; then the left code leaves its
/// value in `a`, and `cp a, b` sets the flags. So the bodies start with `a` = left,
/// `b` = right, and every other register as the operand code left it; the `If` itself
/// changes nothing else. Its two operands load `a`; what else they and the bodies change
/// is up to their code.
///
/// The left code may change `b` (or `c`) too: a `call` (`Call::with_args("GetTileByPixel",
/// ..)`), raw code, or an instruction that writes `b`. Then the `If` saves `bc` around it
/// (`push bc` before, `pop bc` after: 2 bytes, 7 M-cycles), so the compare still reads
/// the right value; the left code's changes to `b` and `c` are undone. Left code that
/// cannot change `b` ([`Regs::written_by`]), such as a load of a variable or of a sprite
/// coordinate, gets no `push` / `pop`.
///
/// The left code must leave the stack as it found it, as any operand code must: when the
/// `If` saved `bc`, a `pop bc` in the left code would take the `If`'s saved value (and the
/// `If`'s own `pop bc` the left code's), and a `ret` there would return to it.
///
/// # Example
/// ```ignore
/// // Simple if
/// gb.add_to_main_loop(
///     If::eq(
///         sprite.get_y(ball),    // loads ball Y into A
///         sprite.get_y(paddle),  // loads paddle Y into A
///         bounce_body,
///     )
/// );
///
/// // With else
/// gb.add_to_main_loop(
///     If::lt(left, right, then_body)
///         .or_else(else_body)
/// );
///
/// // Nested - inner If is Emittable too!
/// gb.add_to_main_loop(
///     If::eq(outer_left, outer_right,
///         If::lt(inner_left, inner_right, inner_body)
///     )
/// );
/// ```
pub struct If {
    /// Instructions that load left value into A
    left: Box<dyn Emittable>,
    /// Instructions that load right value into A
    right: Box<dyn Emittable>,
    /// Comparison operator
    op: ComparisonOp,
    /// Then branch (can be raw instructions or another If)
    then_branch: Box<dyn Emittable>,
    /// Optional else branch
    else_branch: Option<Box<dyn Emittable>>,
}

impl If {
    /// Create an If with equality comparison (left == right)
    pub fn eq(
        left: impl Emittable + 'static,
        right: impl Emittable + 'static,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            left: Box::new(left),
            right: Box::new(right),
            op: ComparisonOp::E,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an If with not-equal comparison (left != right)
    pub fn ne(
        left: impl Emittable + 'static,
        right: impl Emittable + 'static,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            left: Box::new(left),
            right: Box::new(right),
            op: ComparisonOp::NE,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an If with less-than comparison (left < right)
    pub fn lt(
        left: impl Emittable + 'static,
        right: impl Emittable + 'static,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            left: Box::new(left),
            right: Box::new(right),
            op: ComparisonOp::LT,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an If with greater-or-equal comparison (left >= right)
    pub fn ge(
        left: impl Emittable + 'static,
        right: impl Emittable + 'static,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            left: Box::new(left),
            right: Box::new(right),
            op: ComparisonOp::GE,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an If with less-or-equal comparison (left <= right)
    pub fn le(
        left: impl Emittable + 'static,
        right: impl Emittable + 'static,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            left: Box::new(left),
            right: Box::new(right),
            op: ComparisonOp::LE,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an If with greater-than comparison (left > right)
    pub fn gt(
        left: impl Emittable + 'static,
        right: impl Emittable + 'static,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            left: Box::new(left),
            right: Box::new(right),
            op: ComparisonOp::GT,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Add an else branch to the if statement
    pub fn or_else(mut self, else_branch: impl Emittable + 'static) -> Self {
        self.else_branch = Some(Box::new(else_branch));
        self
    }

    /// The registers an `If` changes itself: `a` (the operands' values), `b` (the right
    /// value) and the flags (the compare). Its operand code and bodies change what their
    /// own code changes (see [`If`], Registers).
    pub fn clobbers(&self) -> Regs {
        Regs::A | Regs::B | Regs::F
    }

    /// Generate assembly for simple conditions (E, NE, LT, GE)
    fn emit_simple(
        &mut self,
        asm: &mut Block,
        labels: &LabelAllocator,
        end_label: &str,
        else_label: &str,
    ) {
        if self.else_branch.is_some() {
            // Jump to else branch if condition is false
            asm.jp_cond(self.op.inverted_asm_condition(), else_label);
            asm.emit_all(self.then_branch.emit(labels));
            asm.jp(end_label);
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(labels));
            }
        } else {
            // Jump to end if condition is false (skip then branch)
            asm.jp_cond(self.op.inverted_asm_condition(), end_label);
            asm.emit_all(self.then_branch.emit(labels));
        }
    }

    /// Generate assembly for LE (A <= B): true if C || Z
    fn emit_le(
        &mut self,
        asm: &mut Block,
        labels: &LabelAllocator,
        end_label: &str,
        else_label: &str,
        then_label: &str,
    ) {
        if self.else_branch.is_some() {
            // Jump to then if C (A < B)
            asm.jp_cond(AsmCondition::C, then_label);
            // Jump to then if Z (A == B)
            asm.jp_cond(AsmCondition::Z, then_label);
            // Otherwise jump to else
            asm.jp(else_label);
            // Then branch
            asm.label(then_label);
            asm.emit_all(self.then_branch.emit(labels));
            asm.jp(end_label);
            // Else branch
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(labels));
            }
        } else {
            // Jump to then if C (A < B)
            asm.jp_cond(AsmCondition::C, then_label);
            // Jump to then if Z (A == B)
            asm.jp_cond(AsmCondition::Z, then_label);
            // Otherwise skip to end
            asm.jp(end_label);
            // Then branch
            asm.label(then_label);
            asm.emit_all(self.then_branch.emit(labels));
        }
    }

    /// Generate assembly for GT (A > B): true if NC && NZ
    fn emit_gt(
        &mut self,
        asm: &mut Block,
        labels: &LabelAllocator,
        end_label: &str,
        else_label: &str,
    ) {
        let else_or_end = if self.else_branch.is_some() {
            else_label
        } else {
            end_label
        };

        // Skip to else/end if C (A < B)
        asm.jp_cond(AsmCondition::C, else_or_end);
        // Skip to else/end if Z (A == B)
        asm.jp_cond(AsmCondition::Z, else_or_end);
        // Fall through to then branch (only if NC && NZ, i.e., A > B)
        asm.emit_all(self.then_branch.emit(labels));

        if let Some(ref mut else_instrs) = self.else_branch {
            asm.jp(end_label);
            asm.label(else_label);
            asm.emit_all(else_instrs.emit(labels));
        }
    }
}

impl Emittable for If {
    /// Generate the assembly code for this if statement.
    ///
    /// Generated pattern:
    /// ```asm
    /// ; right instructions (result in A)
    /// ld B, A              ; save right to B
    /// ; (push bc, if the left instructions may change B)
    /// ; left instructions (result in A)
    /// ; (pop bc)
    /// cp B                 ; compare A (left) with B (right)
    /// jp <condition>, .end_if_N
    /// ; then branch
    /// .end_if_N:
    /// ```
    fn emit(&mut self, labels: &LabelAllocator) -> Vec<Instr> {
        let mut asm = Block::new();

        // One number from the program's allocator for the labels of this if
        let [end_label, else_label, then_label] = labels.locals(["end_if", "else", "then"]);

        // Step 1: Execute right instructions (result in A)
        asm.emit_all(self.right.emit(labels));

        // Step 2: Save right value to B
        asm.ld(R8::B, R8::A);

        // Step 3: Execute left instructions (result in A); if they may change B, save BC
        // around them, so B still holds the right value for the compare
        let left = self.left.emit(labels);
        let saves_b = Regs::written_by(&left).is_none_or(|written| written.intersects(Regs::B));
        if saves_b {
            asm.push(R16Stack::BC);
        }
        asm.emit_all(left);
        if saves_b {
            asm.pop(R16Stack::BC);
        }

        // Step 4: Compare A (left) with B (right): the flags describe left - right
        asm.cp(R8::B);

        // Step 5: Handle conditional jumps based on operator type
        match self.op {
            ComparisonOp::E | ComparisonOp::NE | ComparisonOp::LT | ComparisonOp::GE => {
                self.emit_simple(&mut asm, labels, &end_label, &else_label);
            }
            ComparisonOp::LE => {
                self.emit_le(&mut asm, labels, &end_label, &else_label, &then_label);
            }
            ComparisonOp::GT => {
                self.emit_gt(&mut asm, labels, &end_label, &else_label);
            }
        }

        // Emit end label
        asm.label(&end_label);

        asm.into_instrs()
    }
}

/// If statement that compares register A with a constant/label.
///
/// This is a simpler and more efficient variant of `If` when you want to compare
/// the current value in A against a compile-time constant or label.
///
/// # Example
/// ```ignore
/// // Compare A with a constant
/// gb.add_to_main_loop(
///     IfConst::eq(
///         sprite.get_tile(),  // loads tile index into A
///         "WALL_TILE",        // constant label to compare against
///         handle_wall,
///     )
/// );
///
/// // With else branch
/// gb.add_to_main_loop(
///     IfConst::lt(sprite.get_y(), "SCREEN_TOP", clamp_top)
///         .or_else(continue_movement)
/// );
/// ```
pub struct IfConst {
    /// Instructions that load value into A
    value: Box<dyn Emittable>,
    /// The constant to compare against: a `DEF` name, a number, an expression
    constant: Expr,
    /// Comparison operator
    op: ComparisonOp,
    /// Then branch
    then_branch: Box<dyn Emittable>,
    /// Optional else branch
    else_branch: Option<Box<dyn Emittable>>,
}

impl IfConst {
    /// Create an IfConst with equality comparison (A == const)
    pub fn eq(
        value: impl Emittable + 'static,
        constant: impl Into<Expr>,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            value: Box::new(value),
            constant: constant.into(),
            op: ComparisonOp::E,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an IfConst with not-equal comparison (A != const)
    pub fn ne(
        value: impl Emittable + 'static,
        constant: impl Into<Expr>,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            value: Box::new(value),
            constant: constant.into(),
            op: ComparisonOp::NE,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an IfConst with less-than comparison (A < const)
    pub fn lt(
        value: impl Emittable + 'static,
        constant: impl Into<Expr>,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            value: Box::new(value),
            constant: constant.into(),
            op: ComparisonOp::LT,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an IfConst with greater-or-equal comparison (A >= const)
    pub fn ge(
        value: impl Emittable + 'static,
        constant: impl Into<Expr>,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            value: Box::new(value),
            constant: constant.into(),
            op: ComparisonOp::GE,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an IfConst with less-or-equal comparison (A <= const)
    pub fn le(
        value: impl Emittable + 'static,
        constant: impl Into<Expr>,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            value: Box::new(value),
            constant: constant.into(),
            op: ComparisonOp::LE,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an IfConst with greater-than comparison (A > const)
    pub fn gt(
        value: impl Emittable + 'static,
        constant: impl Into<Expr>,
        then_branch: impl Emittable + 'static,
    ) -> Self {
        Self {
            value: Box::new(value),
            constant: constant.into(),
            op: ComparisonOp::GT,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Add an else branch to the if statement
    pub fn or_else(mut self, else_branch: impl Emittable + 'static) -> Self {
        self.else_branch = Some(Box::new(else_branch));
        self
    }

    /// The registers an `IfConst` changes itself: `a` (the value its code loads) and the
    /// flags (`cp a, constant`). The bodies start with `a` = the value; the value code and
    /// the bodies change what their own code changes.
    pub fn clobbers(&self) -> Regs {
        Regs::A | Regs::F
    }

    /// Generate assembly for simple conditions (E, NE, LT, GE)
    fn emit_simple(
        &mut self,
        asm: &mut Block,
        labels: &LabelAllocator,
        end_label: &str,
        else_label: &str,
    ) {
        if self.else_branch.is_some() {
            asm.jp_cond(self.op.inverted_asm_condition(), else_label);
            asm.emit_all(self.then_branch.emit(labels));
            asm.jp(end_label);
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(labels));
            }
        } else {
            asm.jp_cond(self.op.inverted_asm_condition(), end_label);
            asm.emit_all(self.then_branch.emit(labels));
        }
    }

    /// Generate assembly for LE (A <= const): true if C || Z
    fn emit_le(
        &mut self,
        asm: &mut Block,
        labels: &LabelAllocator,
        end_label: &str,
        else_label: &str,
        then_label: &str,
    ) {
        if self.else_branch.is_some() {
            asm.jp_cond(AsmCondition::C, then_label);
            asm.jp_cond(AsmCondition::Z, then_label);
            asm.jp(else_label);
            asm.label(then_label);
            asm.emit_all(self.then_branch.emit(labels));
            asm.jp(end_label);
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(labels));
            }
        } else {
            asm.jp_cond(AsmCondition::C, then_label);
            asm.jp_cond(AsmCondition::Z, then_label);
            asm.jp(end_label);
            asm.label(then_label);
            asm.emit_all(self.then_branch.emit(labels));
        }
    }

    /// Generate assembly for GT (A > const): true if NC && NZ
    fn emit_gt(
        &mut self,
        asm: &mut Block,
        labels: &LabelAllocator,
        end_label: &str,
        else_label: &str,
    ) {
        let else_or_end = if self.else_branch.is_some() {
            else_label
        } else {
            end_label
        };

        asm.jp_cond(AsmCondition::C, else_or_end);
        asm.jp_cond(AsmCondition::Z, else_or_end);
        asm.emit_all(self.then_branch.emit(labels));

        if let Some(ref mut else_instrs) = self.else_branch {
            asm.jp(end_label);
            asm.label(else_label);
            asm.emit_all(else_instrs.emit(labels));
        }
    }
}

impl Emittable for IfConst {
    /// Generate the assembly code for this if-const statement.
    ///
    /// Generated pattern:
    /// ```asm
    /// ; value instructions (result in A)
    /// cp CONST_LABEL       ; compare A with constant
    /// jp <condition>, .end_if_N
    /// ; then branch
    /// .end_if_N:
    /// ```
    fn emit(&mut self, labels: &LabelAllocator) -> Vec<Instr> {
        let mut asm = Block::new();

        let [end_label, else_label, then_label] = labels.locals(["end_if", "else", "then"]);

        // Step 1: Execute value instructions (result in A)
        asm.emit_all(self.value.emit(labels));

        // Step 2: Compare A with constant label
        asm.cp(self.constant.clone());

        // Step 3: Handle conditional jumps based on operator type
        match self.op {
            ComparisonOp::E | ComparisonOp::NE | ComparisonOp::LT | ComparisonOp::GE => {
                self.emit_simple(&mut asm, labels, &end_label, &else_label);
            }
            ComparisonOp::LE => {
                self.emit_le(&mut asm, labels, &end_label, &else_label, &then_label);
            }
            ComparisonOp::GT => {
                self.emit_gt(&mut asm, labels, &end_label, &else_label);
            }
        }

        asm.label(&end_label);

        asm.into_instrs()
    }
}

/// If statement that compares register A (already loaded) with a constant/label.
///
/// This is the simplest variant - it assumes A already contains the value to compare.
/// Use this when you've already loaded A in previous instructions.
///
/// # Example
/// ```ignore
/// // A is already loaded, just compare with constant
/// gb.add_to_main_loop(vec![
///     load_something_into_a(),
///     IfA::eq("WALL_TILE", handle_wall),
/// ]);
/// ```
pub struct IfA {
    /// The constant to compare against: a `DEF` name, a number, an expression
    constant: Expr,
    /// Comparison operator
    op: ComparisonOp,
    /// Then branch
    then_branch: Box<dyn Emittable>,
    /// Optional else branch
    else_branch: Option<Box<dyn Emittable>>,
}

impl IfA {
    /// Create an IfA with equality comparison (A == const)
    pub fn eq(constant: impl Into<Expr>, then_branch: impl Emittable + 'static) -> Self {
        Self {
            constant: constant.into(),
            op: ComparisonOp::E,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an IfA with not-equal comparison (A != const)
    pub fn ne(constant: impl Into<Expr>, then_branch: impl Emittable + 'static) -> Self {
        Self {
            constant: constant.into(),
            op: ComparisonOp::NE,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an IfA with less-than comparison (A < const)
    pub fn lt(constant: impl Into<Expr>, then_branch: impl Emittable + 'static) -> Self {
        Self {
            constant: constant.into(),
            op: ComparisonOp::LT,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an IfA with greater-or-equal comparison (A >= const)
    pub fn ge(constant: impl Into<Expr>, then_branch: impl Emittable + 'static) -> Self {
        Self {
            constant: constant.into(),
            op: ComparisonOp::GE,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an IfA with less-or-equal comparison (A <= const)
    pub fn le(constant: impl Into<Expr>, then_branch: impl Emittable + 'static) -> Self {
        Self {
            constant: constant.into(),
            op: ComparisonOp::LE,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Create an IfA with greater-than comparison (A > const)
    pub fn gt(constant: impl Into<Expr>, then_branch: impl Emittable + 'static) -> Self {
        Self {
            constant: constant.into(),
            op: ComparisonOp::GT,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// The registers an `IfA` changes itself: the flags only (`cp a, constant` reads `a`
    /// and keeps it). The bodies change what their own code changes.
    pub fn clobbers(&self) -> Regs {
        Regs::F
    }

    /// Add an else branch to the if statement
    pub fn or_else(mut self, else_branch: impl Emittable + 'static) -> Self {
        self.else_branch = Some(Box::new(else_branch));
        self
    }

    /// Generate assembly for simple conditions (E, NE, LT, GE)
    fn emit_simple(
        &mut self,
        asm: &mut Block,
        labels: &LabelAllocator,
        end_label: &str,
        else_label: &str,
    ) {
        if self.else_branch.is_some() {
            asm.jp_cond(self.op.inverted_asm_condition(), else_label);
            asm.emit_all(self.then_branch.emit(labels));
            asm.jp(end_label);
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(labels));
            }
        } else {
            asm.jp_cond(self.op.inverted_asm_condition(), end_label);
            asm.emit_all(self.then_branch.emit(labels));
        }
    }

    /// Generate assembly for LE (A <= const): true if C || Z
    fn emit_le(
        &mut self,
        asm: &mut Block,
        labels: &LabelAllocator,
        end_label: &str,
        else_label: &str,
        then_label: &str,
    ) {
        if self.else_branch.is_some() {
            asm.jp_cond(AsmCondition::C, then_label);
            asm.jp_cond(AsmCondition::Z, then_label);
            asm.jp(else_label);
            asm.label(then_label);
            asm.emit_all(self.then_branch.emit(labels));
            asm.jp(end_label);
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(labels));
            }
        } else {
            asm.jp_cond(AsmCondition::C, then_label);
            asm.jp_cond(AsmCondition::Z, then_label);
            asm.jp(end_label);
            asm.label(then_label);
            asm.emit_all(self.then_branch.emit(labels));
        }
    }

    /// Generate assembly for GT (A > const): true if NC && NZ
    fn emit_gt(
        &mut self,
        asm: &mut Block,
        labels: &LabelAllocator,
        end_label: &str,
        else_label: &str,
    ) {
        let else_or_end = if self.else_branch.is_some() {
            else_label
        } else {
            end_label
        };

        asm.jp_cond(AsmCondition::C, else_or_end);
        asm.jp_cond(AsmCondition::Z, else_or_end);
        asm.emit_all(self.then_branch.emit(labels));

        if let Some(ref mut else_instrs) = self.else_branch {
            asm.jp(end_label);
            asm.label(else_label);
            asm.emit_all(else_instrs.emit(labels));
        }
    }
}

impl Emittable for IfA {
    /// Generate the assembly code for this if-a statement.
    ///
    /// Generated pattern:
    /// ```asm
    /// cp CONST_LABEL       ; compare A with constant
    /// jp <condition>, .end_if_N
    /// ; then branch
    /// .end_if_N:
    /// ```
    fn emit(&mut self, labels: &LabelAllocator) -> Vec<Instr> {
        let mut asm = Block::new();

        let [end_label, else_label, then_label] = labels.locals(["end_if", "else", "then"]);

        // Compare A with constant label (A already loaded)
        asm.cp(self.constant.clone());

        // Handle conditional jumps based on operator type
        match self.op {
            ComparisonOp::E | ComparisonOp::NE | ComparisonOp::LT | ComparisonOp::GE => {
                self.emit_simple(&mut asm, labels, &end_label, &else_label);
            }
            ComparisonOp::LE => {
                self.emit_le(&mut asm, labels, &end_label, &else_label, &then_label);
            }
            ComparisonOp::GT => {
                self.emit_gt(&mut asm, labels, &end_label, &else_label);
            }
        }

        asm.label(&end_label);

        asm.into_instrs()
    }
}

/// If statement that branches based on a function call result.
///
/// This is useful for functions that set CPU flags to indicate their result.
/// For example, `IsWallTile` returns true (Z flag) if the tile is a wall.
///
/// # Example
/// ```ignore
/// // Execute body if IsWallTile returns true
/// gb.add_to_main_loop(IfCall::is_true("IsWallTile", body));
///
/// // With setup code before the call:
/// gb.add_to_main_loop(
///     IfCall::is_true("IsWallTile", body).with_setup(setup_instrs)
/// );
///
/// // With else branch:
/// gb.add_to_main_loop(
///     IfCall::is_true("IsWallTile", then_body).or_else(else_body)
/// );
/// ```
pub struct IfCall {
    /// Optional setup instructions to run before the call
    setup: Option<Box<dyn Emittable>>,
    /// Function name to call
    func_name: String,
    /// Condition to check (Z means "execute if zero flag set")
    condition: AsmCondition,
    /// Then branch
    then_branch: Box<dyn Emittable>,
    /// Optional else branch
    else_branch: Option<Box<dyn Emittable>>,
}

impl IfCall {
    /// Execute body if the function returns true (Z flag set).
    ///
    /// Most "Is*" functions (like `IsWallTile`) set the Z flag when the
    /// condition is true.
    ///
    /// # Example
    /// ```ignore
    /// IfCall::is_true("IsWallTile", bounce_body)
    /// ```
    pub fn is_true(func_name: &str, then_branch: impl Emittable + 'static) -> Self {
        Self {
            setup: None,
            func_name: func_name.to_string(),
            condition: AsmCondition::Z,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Execute body if the function returns false (Z flag not set).
    ///
    /// # Example
    /// ```ignore
    /// IfCall::is_false("IsWallTile", not_wall_body)
    /// ```
    pub fn is_false(func_name: &str, then_branch: impl Emittable + 'static) -> Self {
        Self {
            setup: None,
            func_name: func_name.to_string(),
            condition: AsmCondition::NZ,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Execute body if the function result indicates "less than" (C flag set).
    ///
    /// Useful for comparison functions that set carry on less-than.
    pub fn is_less(func_name: &str, then_branch: impl Emittable + 'static) -> Self {
        Self {
            setup: None,
            func_name: func_name.to_string(),
            condition: AsmCondition::C,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Execute body if the function result indicates "greater or equal" (C flag not set).
    ///
    /// Useful for comparison functions that clear carry on greater-or-equal.
    pub fn is_greater_eq(func_name: &str, then_branch: impl Emittable + 'static) -> Self {
        Self {
            setup: None,
            func_name: func_name.to_string(),
            condition: AsmCondition::NC,
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Add setup instructions to run before the function call.
    ///
    /// This is useful for setting up registers/memory before the call.
    pub fn with_setup(mut self, setup: impl Emittable + 'static) -> Self {
        self.setup = Some(Box::new(setup));
        self
    }

    /// Add an else branch to the if statement.
    pub fn or_else(mut self, else_branch: impl Emittable + 'static) -> Self {
        self.else_branch = Some(Box::new(else_branch));
        self
    }

    /// The registers an `IfCall` changes: the flags, the result of the routine it calls (the
    /// `IfCall`'s own instructions, `call` and the jumps, change nothing). The routine
    /// changes what its calling convention says besides
    /// ([`Routine::changes`](crate::stdlib::routine::Routine::changes)), and the setup code
    /// and the bodies what their own code changes.
    pub fn clobbers(&self) -> Regs {
        Regs::F
    }

    /// Get the inverted condition for jumping AWAY from the then branch.
    fn inverted_condition(&self) -> AsmCondition {
        match self.condition {
            AsmCondition::Z => AsmCondition::NZ,
            AsmCondition::NZ => AsmCondition::Z,
            AsmCondition::C => AsmCondition::NC,
            AsmCondition::NC => AsmCondition::C,
        }
    }
}

impl Emittable for IfCall {
    /// Generate the assembly code for this if-call statement.
    ///
    /// Generated pattern (without else):
    /// ```asm
    /// ; setup instructions (optional)
    /// call FuncName
    /// jp <inverted_condition>, .end_if_N
    /// ; then branch
    /// .end_if_N:
    /// ```
    ///
    /// Generated pattern (with else):
    /// ```asm
    /// ; setup instructions (optional)
    /// call FuncName
    /// jp <inverted_condition>, .else_N
    /// ; then branch
    /// jp .end_if_N
    /// .else_N:
    /// ; else branch
    /// .end_if_N:
    /// ```
    fn emit(&mut self, labels: &LabelAllocator) -> Vec<Instr> {
        let mut asm = Block::new();

        let [end_label, else_label] = labels.locals(["end_if", "else"]);

        // Step 1: Emit setup instructions (if any)
        if let Some(ref mut setup) = self.setup {
            asm.emit_all(setup.emit(labels));
        }

        // Step 2: Call the function
        asm.emit(Instr::Call {
            target: JumpTarget::Label(self.func_name.clone()),
        });

        // Step 3: Conditional jump and branches
        if self.else_branch.is_some() {
            // Jump to else if condition is false
            asm.jp_cond(self.inverted_condition(), &else_label);
            asm.emit_all(self.then_branch.emit(labels));
            asm.jp(&end_label);
            asm.label(&else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(labels));
            }
        } else {
            // Jump to end if condition is false
            asm.jp_cond(self.inverted_condition(), &end_label);
            asm.emit_all(self.then_branch.emit(labels));
        }

        asm.label(&end_label);

        asm.into_instrs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::test_cpu::TestCpu;
    use crate::asm::{Dst, Operand};

    #[test]
    fn test_emittable_vec() {
        let mut instrs = vec![Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::from(42),
        }];

        let labels = LabelAllocator::new();
        let result = instrs.emit(&labels);

        assert_eq!(result.len(), 1);
        assert_eq!(labels.next_id(), 0); // plain code takes no label number
    }

    #[test]
    fn test_simple_if_eq() {
        let left = vec![Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::from(10),
        }];
        let right = vec![Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::from(20),
        }];
        let then_body = vec![Instr::Ld {
            dst: Dst::R8(R8::C),
            src: Operand::from(1),
        }];

        let mut if_stmt = If::eq(left, right, then_body);
        let labels = LabelAllocator::new();
        let result = if_stmt.emit(&labels);

        assert!(!result.is_empty());
        assert_eq!(labels.next_id(), 1); // the If took one number
    }

    #[test]
    fn test_if_with_else() {
        let left = vec![Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::from(10),
        }];
        let right = vec![Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::from(20),
        }];
        let then_body = vec![Instr::Ld {
            dst: Dst::R8(R8::C),
            src: Operand::from(1),
        }];
        let else_body = vec![Instr::Ld {
            dst: Dst::R8(R8::C),
            src: Operand::from(0),
        }];

        let mut if_stmt = If::eq(left, right, then_body).or_else(else_body);
        let labels = LabelAllocator::new();
        let result = if_stmt.emit(&labels);

        assert!(!result.is_empty());
        assert_eq!(labels.next_id(), 1);
    }

    #[test]
    fn test_nested_if() {
        let outer_left = vec![Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::from(1),
        }];
        let outer_right = vec![Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::from(1),
        }];

        let inner_left = vec![Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::from(2),
        }];
        let inner_right = vec![Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::from(2),
        }];
        let inner_body = vec![Instr::Ld {
            dst: Dst::R8(R8::D),
            src: Operand::from(99),
        }];

        let inner_if = If::lt(inner_left, inner_right, inner_body);
        let mut outer_if = If::eq(outer_left, outer_right, inner_if);

        let labels = LabelAllocator::new();
        let result = outer_if.emit(&labels);

        assert!(!result.is_empty());
        assert_eq!(labels.next_id(), 2); // each if took one number
        // The outer If takes its number first
        let text: Vec<String> = result.iter().map(|instr| instr.to_string()).collect();
        assert_eq!(text.last().map(String::as_str), Some(".end_if_0:"));
        assert!(text.contains(&".end_if_1:".to_string()));
    }

    #[test]
    fn test_each_if_takes_the_next_label_number() {
        let make_if = || {
            If::eq(
                vec![Instr::Ld {
                    dst: Dst::R8(R8::A),
                    src: Operand::from(1),
                }],
                vec![Instr::Ld {
                    dst: Dst::R8(R8::A),
                    src: Operand::from(1),
                }],
                vec![Instr::Ld {
                    dst: Dst::R8(R8::A),
                    src: Operand::from(0),
                }],
            )
        };

        // Numbers are shared with every other user of the allocator (snippets, ...)
        let labels = LabelAllocator::new();

        let mut if1 = make_if();
        assert!(if1.emit(&labels).contains(&Instr::Label {
            name: ".end_if_0".to_string()
        }));
        assert_eq!(labels.local("check_left"), ".check_left_1");

        let mut if2 = make_if();
        assert!(if2.emit(&labels).contains(&Instr::Label {
            name: ".end_if_2".to_string()
        }));
        assert_eq!(labels.next_id(), 3);
    }

    /// Runs the code emitted by `If` on the test CPU and returns register C
    fn run(instrs: &[Instr]) -> u8 {
        let mut cpu = TestCpu::default();
        cpu.run(instrs);
        cpu.c
    }

    fn load_a(value: u8) -> Vec<Instr> {
        vec![Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::from(value),
        }]
    }

    fn set_c(value: u8) -> Vec<Instr> {
        vec![Instr::Ld {
            dst: Dst::R8(R8::C),
            src: Operand::from(value),
        }]
    }

    type MakeIf = fn(Vec<Instr>, Vec<Instr>, Vec<Instr>) -> If;
    /// (operator name, constructor, expected result for (left, right))
    type Case = (&'static str, MakeIf, fn(u8, u8) -> bool);

    #[test]
    fn test_if_compares_left_with_right() {
        let cases: [Case; 6] = [
            ("eq", |l, r, t| If::eq(l, r, t), |l, r| l == r),
            ("ne", |l, r, t| If::ne(l, r, t), |l, r| l != r),
            ("lt", |l, r, t| If::lt(l, r, t), |l, r| l < r),
            ("ge", |l, r, t| If::ge(l, r, t), |l, r| l >= r),
            ("le", |l, r, t| If::le(l, r, t), |l, r| l <= r),
            ("gt", |l, r, t| If::gt(l, r, t), |l, r| l > r),
        ];
        let values = [0u8, 1, 5, 10, 127, 128, 254, 255];

        for (name, make, expected) in cases {
            for l in values {
                for r in values {
                    let want = expected(l, r);

                    // Without else: C becomes 1 only when the condition holds
                    let mut if_stmt = make(load_a(l), load_a(r), set_c(1));
                    let ran_then = run(&if_stmt.emit(&LabelAllocator::new())) == 1;
                    assert_eq!(ran_then, want, "If::{}({}, {})", name, l, r);

                    // With else: C becomes 1 (then) or 2 (else)
                    let mut if_stmt = make(load_a(l), load_a(r), set_c(1)).or_else(set_c(2));
                    let branch = run(&if_stmt.emit(&LabelAllocator::new()));
                    let want_branch = if want { 1 } else { 2 };
                    assert_eq!(branch, want_branch, "If::{}({}, {}) with else", name, l, r);
                }
            }
        }
    }

    // ==================== Registers (B5, Phase 2) ====================

    use crate::asm::{Mem, R16};
    use crate::stdlib::routine::tests::{run_and_compare, seeded_cpus};

    /// A body that marks that it ran: `ld [var], a`, which changes no register
    fn mark(var: &str) -> Vec<Instr> {
        let mut code = Block::new();
        code.ld(Mem::addr(var), R8::A);
        code.into_instrs()
    }

    /// Run `code` (an If and its bodies, then `ret`, then `routines`) on each seeded CPU,
    /// with `wThen` / `wElse` in memory; return the registers it changed, and whether the
    /// then body ran, per CPU
    fn run_if(
        code: Vec<Instr>,
        routines: &[Instr],
        setup: &dyn Fn(&mut TestCpu),
    ) -> Vec<(Regs, bool)> {
        let mut all = Block::new();
        all.emit_all(code).ret().emit_all(routines.to_vec());
        let all = all.into_instrs();
        seeded_cpus()
            .into_iter()
            .map(|mut cpu| {
                setup(&mut cpu);
                cpu.mem.insert("wThen".to_string(), 0);
                cpu.mem.insert("wElse".to_string(), 0);
                let changes = run_and_compare(&mut cpu, &all);
                (changes, wrote(&cpu, "wThen"))
            })
            .collect()
    }

    /// Whether the code that ran on `cpu` wrote `var`
    fn wrote(cpu: &TestCpu, var: &str) -> bool {
        cpu.trace.iter().any(
            |event| matches!(event, crate::asm::test_cpu::Event::Write(name, _) if name == var),
        )
    }

    type MakeIfConst = fn(Vec<Instr>, u8, Vec<Instr>) -> IfConst;
    type MakeIfA = fn(u8, Vec<Instr>) -> IfA;

    #[test]
    fn test_each_if_kind_changes_only_what_it_lists() {
        // The registers each If kind uses, on the test CPU: every other register keeps its
        // value, whichever branch runs, with or without else
        let ifs: [(&str, MakeIf); 6] = [
            ("eq", |l, r, t| If::eq(l, r, t)),
            ("ne", |l, r, t| If::ne(l, r, t)),
            ("lt", |l, r, t| If::lt(l, r, t)),
            ("ge", |l, r, t| If::ge(l, r, t)),
            ("le", |l, r, t| If::le(l, r, t)),
            ("gt", |l, r, t| If::gt(l, r, t)),
        ];
        let if_consts: [MakeIfConst; 6] = [
            |v, c, t| IfConst::eq(v, c, t),
            |v, c, t| IfConst::ne(v, c, t),
            |v, c, t| IfConst::lt(v, c, t),
            |v, c, t| IfConst::ge(v, c, t),
            |v, c, t| IfConst::le(v, c, t),
            |v, c, t| IfConst::gt(v, c, t),
        ];
        let if_as: [MakeIfA; 6] = [
            |c, t| IfA::eq(c, t),
            |c, t| IfA::ne(c, t),
            |c, t| IfA::lt(c, t),
            |c, t| IfA::ge(c, t),
            |c, t| IfA::le(c, t),
            |c, t| IfA::gt(c, t),
        ];
        let mut seen = [Regs::NONE; 3];
        for (index, (name, make)) in ifs.into_iter().enumerate() {
            for (l, r) in [(1u8, 2u8), (2, 2), (3, 2)] {
                for with_else in [false, true] {
                    let mut stmt = make(load_a(l), load_a(r), mark("wThen"));
                    if with_else {
                        stmt = stmt.or_else(mark("wElse"));
                    }
                    let clobbers = stmt.clobbers();
                    for (changes, _) in run_if(stmt.emit(&LabelAllocator::new()), &[], &|_| {}) {
                        assert!(
                            clobbers.contains(changes),
                            "If::{}({}, {}) changes {}",
                            name,
                            l,
                            r,
                            changes
                        );
                        seen[0] |= changes;
                    }

                    let mut stmt = if_consts[index](load_a(l), r, mark("wThen"));
                    if with_else {
                        stmt = stmt.or_else(mark("wElse"));
                    }
                    let clobbers = stmt.clobbers();
                    for (changes, _) in run_if(stmt.emit(&LabelAllocator::new()), &[], &|_| {}) {
                        assert!(
                            clobbers.contains(changes),
                            "IfConst::{} changes {}",
                            name,
                            changes
                        );
                        seen[1] |= changes;
                    }

                    let mut stmt = if_as[index](r, mark("wThen"));
                    if with_else {
                        stmt = stmt.or_else(mark("wElse"));
                    }
                    let clobbers = stmt.clobbers();
                    let set_a = move |cpu: &mut TestCpu| cpu.a = l;
                    for (changes, _) in run_if(stmt.emit(&LabelAllocator::new()), &[], &set_a) {
                        assert!(
                            clobbers.contains(changes),
                            "IfA::{} changes {}",
                            name,
                            changes
                        );
                        seen[2] |= changes;
                    }
                }
            }
        }
        // And the lists are exact: each register listed changed in some case
        assert_eq!(
            seen,
            [Regs::A | Regs::B | Regs::F, Regs::A | Regs::F, Regs::F]
        );

        // IfCall: the flags, the routine's result; the routine here changes nothing else
        let mut is_five = Block::new();
        is_five.label("IsFive").cp(5).ret();
        let routine = is_five.into_instrs();
        for value in [5u8, 6] {
            for with_else in [false, true] {
                let mut stmt = IfCall::is_true("IsFive", mark("wThen"));
                if with_else {
                    stmt = stmt.or_else(mark("wElse"));
                }
                assert_eq!(stmt.clobbers(), Regs::F);
                let set_a = move |cpu: &mut TestCpu| cpu.a = value;
                for (changes, then_ran) in
                    run_if(stmt.emit(&LabelAllocator::new()), &routine, &set_a)
                {
                    assert!(
                        Regs::F.contains(changes),
                        "IfCall with a = {}: {}",
                        value,
                        changes
                    );
                    assert_eq!(then_ran, value == 5);
                }
            }
        }
    }

    #[test]
    fn test_if_left_code_that_changes_b() {
        // B5 (latent): the left code ran after the right value was put in b, so left code
        // that changed b (here: writes b and c, as a get_pivot does) made the If compare
        // with something else than the right value. Now the If saves bc around it.
        let cases: [Case; 6] = [
            ("eq", |l, r, t| If::eq(l, r, t), |l, r| l == r),
            ("ne", |l, r, t| If::ne(l, r, t), |l, r| l != r),
            ("lt", |l, r, t| If::lt(l, r, t), |l, r| l < r),
            ("ge", |l, r, t| If::ge(l, r, t), |l, r| l >= r),
            ("le", |l, r, t| If::le(l, r, t), |l, r| l <= r),
            ("gt", |l, r, t| If::gt(l, r, t), |l, r| l > r),
        ];
        let left_writing_bc = |value: u8| {
            let mut code = Block::new();
            code.ld(R8::B, 0x99).ld(R8::C, 0x77).ld_a(value);
            code.into_instrs()
        };
        for (name, make, expected) in cases {
            for l in [0u8, 5, 10, 0x99, 255] {
                for r in [0u8, 5, 10, 0x99, 255] {
                    let mut stmt = make(left_writing_bc(l), load_a(r), mark("wThen"));
                    let code = stmt.emit(&LabelAllocator::new());
                    let text: Vec<String> = code.iter().map(|i| i.to_string()).collect();
                    assert!(text.contains(&"push bc".to_string()), "{:?}", text);
                    for mut cpu in seeded_cpus() {
                        cpu.mem.insert("wThen".to_string(), 0);
                        let c = cpu.c;
                        cpu.run(&code);
                        assert_eq!(
                            wrote(&cpu, "wThen"),
                            expected(l, r),
                            "If::{}({}, {})",
                            name,
                            l,
                            r
                        );
                        // The left code's change to c is undone, b holds the right value
                        assert_eq!((cpu.known(R8::B), cpu.known(R8::C)), (Some(r), Some(c)));
                    }
                }
            }
        }
    }

    #[test]
    fn test_if_left_code_that_calls_a_routine() {
        // A left operand that calls GetTileByPixel (which changes bc): the tile under the
        // pixel is compared with the right value
        use crate::stdlib::flow::Call;
        use crate::stdlib::graphics::utility::get_tile_by_pixel;
        let tile_at = |addr: u16| (addr % 251) as u8;
        let routine = get_tile_by_pixel();
        for (x, y) in [(0u8, 0u8), (100, 57), (255, 143)] {
            let addr = crate::hw::SCRN0.value + u16::from(y / 8) * 32 + u16::from(x / 8);
            for right in [tile_at(addr), tile_at(addr).wrapping_add(1)] {
                let mut pivot = Block::new();
                pivot.ld(R8::B, x).ld(R8::C, y);
                let mut stmt = If::eq(
                    Call::with_args("GetTileByPixel", pivot.into_instrs()),
                    load_a(right),
                    mark("wThen"),
                );
                let mut code = Block::new();
                code.emit_all(stmt.emit(&LabelAllocator::new()))
                    .ret()
                    .emit_all(routine.clone());
                let mut cpu = TestCpu::default();
                for a in crate::hw::SCRN0.value..crate::hw::SCRN1.value {
                    cpu.mem.insert(format!("${:04X}", a), tile_at(a));
                }
                cpu.mem.insert("wThen".to_string(), 0);
                cpu.run(&code.into_instrs());
                assert_eq!(
                    wrote(&cpu, "wThen"),
                    right == tile_at(addr),
                    "pixel ({}, {}), right {}",
                    x,
                    y,
                    right
                );
            }
        }
    }

    #[test]
    fn test_if_left_code_that_keeps_b_is_emitted_as_before() {
        // No push / pop when the left code cannot change b: the examples' Ifs (a variable or
        // a sprite coordinate, plus or minus a constant) are the same code as before
        let mut left = Block::new();
        left.ld(R8::A, Mem::addr("wBallY")).add(5);
        let mut stmt = If::eq(left.into_instrs(), load_a(3), set_c(1));
        let text: Vec<String> = stmt
            .emit(&LabelAllocator::new())
            .iter()
            .map(|i| i.to_string())
            .collect();
        assert_eq!(
            text,
            [
                "ld a, 3",
                "ld b, a",
                "ld a, [wBallY]",
                "add a, 5",
                "cp a, b",
                "jp nz, .end_if_0",
                "ld c, 1",
                ".end_if_0:"
            ]
        );
        // Left code that changes other pairs (de, hl) keeps b too
        let mut left = Block::new();
        left.ld(R16::HL, "Table")
            .ld(R8::A, Mem::Hli)
            .ld(R8::D, R8::A);
        let mut stmt = If::eq(left.into_instrs(), load_a(3), set_c(1));
        assert!(
            !stmt
                .emit(&LabelAllocator::new())
                .contains(&Instr::Push { pair: R16Stack::BC })
        );
    }
}
