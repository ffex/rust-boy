use crate::gb_asm::{Asm, Condition as AsmCondition, Expr, Instr, JumpTarget, R8};

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
/// - Loading left value into A (the left code must not change B)
/// - Comparing A (left) with B (right)
/// - Conditional jumps and label generation
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

    /// Generate assembly for simple conditions (E, NE, LT, GE)
    fn emit_simple(
        &mut self,
        asm: &mut Asm,
        counter: &mut usize,
        end_label: &str,
        else_label: &str,
    ) {
        if self.else_branch.is_some() {
            // Jump to else branch if condition is false
            asm.jp_cond(self.op.inverted_asm_condition(), else_label);
            asm.emit_all(self.then_branch.emit(counter));
            asm.jp(end_label);
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(counter));
            }
        } else {
            // Jump to end if condition is false (skip then branch)
            asm.jp_cond(self.op.inverted_asm_condition(), end_label);
            asm.emit_all(self.then_branch.emit(counter));
        }
    }

    /// Generate assembly for LE (A <= B): true if C || Z
    fn emit_le(
        &mut self,
        asm: &mut Asm,
        counter: &mut usize,
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
            asm.emit_all(self.then_branch.emit(counter));
            asm.jp(end_label);
            // Else branch
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(counter));
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
            asm.emit_all(self.then_branch.emit(counter));
        }
    }

    /// Generate assembly for GT (A > B): true if NC && NZ
    fn emit_gt(&mut self, asm: &mut Asm, counter: &mut usize, end_label: &str, else_label: &str) {
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
        asm.emit_all(self.then_branch.emit(counter));

        if let Some(ref mut else_instrs) = self.else_branch {
            asm.jp(end_label);
            asm.label(else_label);
            asm.emit_all(else_instrs.emit(counter));
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
    /// ; left instructions (result in A)
    /// cp B                 ; compare A (left) with B (right)
    /// jp <condition>, .end_if_N
    /// ; then branch
    /// .end_if_N:
    /// ```
    fn emit(&mut self, counter: &mut usize) -> Vec<Instr> {
        let mut asm = Asm::new();

        // Get unique counter for this if
        let my_counter = *counter;
        *counter += 1;

        let end_label = format!(".end_if_{}", my_counter);
        let else_label = format!(".else_{}", my_counter);
        let then_label = format!(".then_{}", my_counter);

        // Step 1: Execute right instructions (result in A)
        asm.emit_all(self.right.emit(counter));

        // Step 2: Save right value to B
        asm.ld(R8::B, R8::A);

        // Step 3: Execute left instructions (result in A)
        asm.emit_all(self.left.emit(counter));

        // Step 4: Compare A (left) with B (right): the flags describe left - right
        asm.cp(R8::B);

        // Step 5: Handle conditional jumps based on operator type
        match self.op {
            ComparisonOp::E | ComparisonOp::NE | ComparisonOp::LT | ComparisonOp::GE => {
                self.emit_simple(&mut asm, counter, &end_label, &else_label);
            }
            ComparisonOp::LE => {
                self.emit_le(&mut asm, counter, &end_label, &else_label, &then_label);
            }
            ComparisonOp::GT => {
                self.emit_gt(&mut asm, counter, &end_label, &else_label);
            }
        }

        // Emit end label
        asm.label(&end_label);

        asm.get_main_instrs()
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

    /// Generate assembly for simple conditions (E, NE, LT, GE)
    fn emit_simple(
        &mut self,
        asm: &mut Asm,
        counter: &mut usize,
        end_label: &str,
        else_label: &str,
    ) {
        if self.else_branch.is_some() {
            asm.jp_cond(self.op.inverted_asm_condition(), else_label);
            asm.emit_all(self.then_branch.emit(counter));
            asm.jp(end_label);
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(counter));
            }
        } else {
            asm.jp_cond(self.op.inverted_asm_condition(), end_label);
            asm.emit_all(self.then_branch.emit(counter));
        }
    }

    /// Generate assembly for LE (A <= const): true if C || Z
    fn emit_le(
        &mut self,
        asm: &mut Asm,
        counter: &mut usize,
        end_label: &str,
        else_label: &str,
        then_label: &str,
    ) {
        if self.else_branch.is_some() {
            asm.jp_cond(AsmCondition::C, then_label);
            asm.jp_cond(AsmCondition::Z, then_label);
            asm.jp(else_label);
            asm.label(then_label);
            asm.emit_all(self.then_branch.emit(counter));
            asm.jp(end_label);
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(counter));
            }
        } else {
            asm.jp_cond(AsmCondition::C, then_label);
            asm.jp_cond(AsmCondition::Z, then_label);
            asm.jp(end_label);
            asm.label(then_label);
            asm.emit_all(self.then_branch.emit(counter));
        }
    }

    /// Generate assembly for GT (A > const): true if NC && NZ
    fn emit_gt(&mut self, asm: &mut Asm, counter: &mut usize, end_label: &str, else_label: &str) {
        let else_or_end = if self.else_branch.is_some() {
            else_label
        } else {
            end_label
        };

        asm.jp_cond(AsmCondition::C, else_or_end);
        asm.jp_cond(AsmCondition::Z, else_or_end);
        asm.emit_all(self.then_branch.emit(counter));

        if let Some(ref mut else_instrs) = self.else_branch {
            asm.jp(end_label);
            asm.label(else_label);
            asm.emit_all(else_instrs.emit(counter));
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
    fn emit(&mut self, counter: &mut usize) -> Vec<Instr> {
        let mut asm = Asm::new();

        let my_counter = *counter;
        *counter += 1;

        let end_label = format!(".end_if_{}", my_counter);
        let else_label = format!(".else_{}", my_counter);
        let then_label = format!(".then_{}", my_counter);

        // Step 1: Execute value instructions (result in A)
        asm.emit_all(self.value.emit(counter));

        // Step 2: Compare A with constant label
        asm.cp(self.constant.clone());

        // Step 3: Handle conditional jumps based on operator type
        match self.op {
            ComparisonOp::E | ComparisonOp::NE | ComparisonOp::LT | ComparisonOp::GE => {
                self.emit_simple(&mut asm, counter, &end_label, &else_label);
            }
            ComparisonOp::LE => {
                self.emit_le(&mut asm, counter, &end_label, &else_label, &then_label);
            }
            ComparisonOp::GT => {
                self.emit_gt(&mut asm, counter, &end_label, &else_label);
            }
        }

        asm.label(&end_label);

        asm.get_main_instrs()
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

    /// Add an else branch to the if statement
    pub fn or_else(mut self, else_branch: impl Emittable + 'static) -> Self {
        self.else_branch = Some(Box::new(else_branch));
        self
    }

    /// Generate assembly for simple conditions (E, NE, LT, GE)
    fn emit_simple(
        &mut self,
        asm: &mut Asm,
        counter: &mut usize,
        end_label: &str,
        else_label: &str,
    ) {
        if self.else_branch.is_some() {
            asm.jp_cond(self.op.inverted_asm_condition(), else_label);
            asm.emit_all(self.then_branch.emit(counter));
            asm.jp(end_label);
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(counter));
            }
        } else {
            asm.jp_cond(self.op.inverted_asm_condition(), end_label);
            asm.emit_all(self.then_branch.emit(counter));
        }
    }

    /// Generate assembly for LE (A <= const): true if C || Z
    fn emit_le(
        &mut self,
        asm: &mut Asm,
        counter: &mut usize,
        end_label: &str,
        else_label: &str,
        then_label: &str,
    ) {
        if self.else_branch.is_some() {
            asm.jp_cond(AsmCondition::C, then_label);
            asm.jp_cond(AsmCondition::Z, then_label);
            asm.jp(else_label);
            asm.label(then_label);
            asm.emit_all(self.then_branch.emit(counter));
            asm.jp(end_label);
            asm.label(else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(counter));
            }
        } else {
            asm.jp_cond(AsmCondition::C, then_label);
            asm.jp_cond(AsmCondition::Z, then_label);
            asm.jp(end_label);
            asm.label(then_label);
            asm.emit_all(self.then_branch.emit(counter));
        }
    }

    /// Generate assembly for GT (A > const): true if NC && NZ
    fn emit_gt(&mut self, asm: &mut Asm, counter: &mut usize, end_label: &str, else_label: &str) {
        let else_or_end = if self.else_branch.is_some() {
            else_label
        } else {
            end_label
        };

        asm.jp_cond(AsmCondition::C, else_or_end);
        asm.jp_cond(AsmCondition::Z, else_or_end);
        asm.emit_all(self.then_branch.emit(counter));

        if let Some(ref mut else_instrs) = self.else_branch {
            asm.jp(end_label);
            asm.label(else_label);
            asm.emit_all(else_instrs.emit(counter));
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
    fn emit(&mut self, counter: &mut usize) -> Vec<Instr> {
        let mut asm = Asm::new();

        let my_counter = *counter;
        *counter += 1;

        let end_label = format!(".end_if_{}", my_counter);
        let else_label = format!(".else_{}", my_counter);
        let then_label = format!(".then_{}", my_counter);

        // Compare A with constant label (A already loaded)
        asm.cp(self.constant.clone());

        // Handle conditional jumps based on operator type
        match self.op {
            ComparisonOp::E | ComparisonOp::NE | ComparisonOp::LT | ComparisonOp::GE => {
                self.emit_simple(&mut asm, counter, &end_label, &else_label);
            }
            ComparisonOp::LE => {
                self.emit_le(&mut asm, counter, &end_label, &else_label, &then_label);
            }
            ComparisonOp::GT => {
                self.emit_gt(&mut asm, counter, &end_label, &else_label);
            }
        }

        asm.label(&end_label);

        asm.get_main_instrs()
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
    fn emit(&mut self, counter: &mut usize) -> Vec<Instr> {
        let mut asm = Asm::new();

        let my_counter = *counter;
        *counter += 1;

        let end_label = format!(".end_if_{}", my_counter);
        let else_label = format!(".else_{}", my_counter);

        // Step 1: Emit setup instructions (if any)
        if let Some(ref mut setup) = self.setup {
            asm.emit_all(setup.emit(counter));
        }

        // Step 2: Call the function
        asm.emit(Instr::Call {
            target: JumpTarget::Label(self.func_name.clone()),
        });

        // Step 3: Conditional jump and branches
        if self.else_branch.is_some() {
            // Jump to else if condition is false
            asm.jp_cond(self.inverted_condition(), &else_label);
            asm.emit_all(self.then_branch.emit(counter));
            asm.jp(&end_label);
            asm.label(&else_label);
            if let Some(ref mut else_instrs) = self.else_branch {
                asm.emit_all(else_instrs.emit(counter));
            }
        } else {
            // Jump to end if condition is false
            asm.jp_cond(self.inverted_condition(), &end_label);
            asm.emit_all(self.then_branch.emit(counter));
        }

        asm.label(&end_label);

        asm.get_main_instrs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::test_cpu::TestCpu;
    use crate::gb_asm::{Dst, Operand};

    #[test]
    fn test_emittable_vec() {
        let mut instrs = vec![Instr::Ld {
            dst: Dst::R8(R8::A),
            src: Operand::from(42),
        }];

        let mut counter = 0;
        let result = instrs.emit(&mut counter);

        assert_eq!(result.len(), 1);
        assert_eq!(counter, 0); // Vec doesn't increment counter
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
        let mut counter = 0;
        let result = if_stmt.emit(&mut counter);

        assert!(!result.is_empty());
        assert_eq!(counter, 1); // If increments counter
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
        let mut counter = 0;
        let result = if_stmt.emit(&mut counter);

        assert!(!result.is_empty());
        assert_eq!(counter, 1);
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

        let mut counter = 0;
        let result = outer_if.emit(&mut counter);

        assert!(!result.is_empty());
        assert_eq!(counter, 2); // Both ifs increment counter
    }

    #[test]
    fn test_counter_increments_correctly() {
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

        let mut counter = 0;

        let mut if1 = make_if();
        if1.emit(&mut counter);
        assert_eq!(counter, 1);

        let mut if2 = make_if();
        if2.emit(&mut counter);
        assert_eq!(counter, 2);

        let mut if3 = make_if();
        if3.emit(&mut counter);
        assert_eq!(counter, 3);
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
                    let ran_then = run(&if_stmt.emit(&mut 0)) == 1;
                    assert_eq!(ran_then, want, "If::{}({}, {})", name, l, r);

                    // With else: C becomes 1 (then) or 2 (else)
                    let mut if_stmt = make(load_a(l), load_a(r), set_c(1)).or_else(set_c(2));
                    let branch = run(&if_stmt.emit(&mut 0));
                    let want_branch = if want { 1 } else { 2 };
                    assert_eq!(branch, want_branch, "If::{}({}, {}) with else", name, l, r);
                }
            }
        }
    }
}
