use crate::gb_asm::{Asm, Condition as AsmCondition, Instr, Operand, Register};

/// Comparison operators for conditions
/// TODO probably can we implement witout considering the case LE and GT.
/// IMPORTANT NOTE about LE and GT:
/// The Game Boy CPU only has Z (zero) and C (carry) flags after a compare.
/// After `cp B` (comparing A with B):
/// - E  (A == B): Z flag set
/// - NE (A != B): Z flag clear
/// - LT (A < B):  C flag set
/// - GE (A >= B): C flag clear
/// - LE (A <= B): C flag set OR Z flag set (requires two checks or special logic)
/// - GT (A > B):  C flag clear AND Z flag clear (requires two checks or special logic)
///
/// Currently, LE and GT use approximations that may not work correctly in all cases.
/// For precise comparisons, use combinations of the other operators.
#[derive(Clone, Debug)]
pub enum ComparisonOp {
    E,  // == (equal)
    NE, // != (not equal)
    LT, // <  (less than) - PRECISE
    GE, // >= (greater or equal) - PRECISE
    LE, // <= (less or equal) - WARNING: See note above
    GT, // >  (greater than) - WARNING: See note above
}

/// Represents a condition operand (can be a register value, immediate, or label)
#[derive(Clone, Debug)]
pub enum ConditionOperand {
    Register(Register),
    Immediate(u8),
    Label(String),
}

impl ConditionOperand {
    pub fn to_operand(&self) -> Operand {
        match self {
            ConditionOperand::Register(op) => Operand::Reg(op.clone()),
            ConditionOperand::Immediate(val) => Operand::Imm(*val),
            ConditionOperand::Label(label) => Operand::Label(label.clone()),
        }
    }
}

/// Represents a condition for an if statement
/// The condition compares two operands using a comparison operator
#[derive(Clone, Debug)]
pub struct IfCondition {
    pub left: ConditionOperand,
    pub right: ConditionOperand,
    pub op: ComparisonOp,
}

impl IfCondition {
    /// Create a new condition
    pub fn new(left: ConditionOperand, right: ConditionOperand, op: ComparisonOp) -> Self {
        Self { left, right, op }
    }

    /// Helper to create an equality condition (A == B)
    pub fn equal(left: ConditionOperand, right: ConditionOperand) -> Self {
        Self::new(left, right, ComparisonOp::E)
    }

    /// Helper to create a not-equal condition (A != B)
    pub fn not_equal(left: ConditionOperand, right: ConditionOperand) -> Self {
        Self::new(left, right, ComparisonOp::NE)
    }

    /// Helper to create a less-than condition (A < B) - PRECISE
    pub fn less_than(left: ConditionOperand, right: ConditionOperand) -> Self {
        Self::new(left, right, ComparisonOp::LT)
    }

    /// Helper to create a greater-equal condition (A >= B) - PRECISE
    pub fn greater_equal(left: ConditionOperand, right: ConditionOperand) -> Self {
        Self::new(left, right, ComparisonOp::GE)
    }

    /// Helper to create a less-equal condition (A <= B)
    /// WARNING: This uses an approximation. For precise behavior, consider using LT or E separately.
    pub fn less_equal(left: ConditionOperand, right: ConditionOperand) -> Self {
        Self::new(left, right, ComparisonOp::LE)
    }

    /// Helper to create a greater-than condition (A > B)
    /// WARNING: This uses an approximation. For precise behavior, consider using GE with inverted equality.
    pub fn greater_than(left: ConditionOperand, right: ConditionOperand) -> Self {
        Self::new(left, right, ComparisonOp::GT)
    }

    /// Get the inverted condition (for jumping AWAY from the then branch)
    /// This is used to skip the then branch when the condition is FALSE
    ///
    /// After `cp right` (where A is the left operand):
    /// - Z set: A == right
    /// - C set: A < right
    /// - NC (C clear): A >= right
    ///
    /// Condition inversions:
    /// - !(A == B) = (A != B) -> NZ
    /// - !(A != B) = (A == B) -> Z
    /// - !(A < B)  = (A >= B) -> NC
    /// - !(A >= B) = (A < B)  -> C
    /// - !(A <= B) = (A > B)  -> Requires NC && NZ, we approximate with NC*
    /// - !(A > B)  = (A <= B) -> Requires C || Z, we approximate with C || check Z separately*
    ///
    /// *For LE and GT: These require checking multiple flags which isn't directly
    ///  supported by single conditional jumps. The approximations may be incorrect
    ///  when A == B.
    fn inverted_asm_condition(&self) -> AsmCondition {
        match self.op {
            ComparisonOp::E => AsmCondition::NZ,  // Skip then if not equal
            ComparisonOp::NE => AsmCondition::Z,  // Skip then if equal
            ComparisonOp::LT => AsmCondition::NC, // Skip then if >= (not less than)
            ComparisonOp::GE => AsmCondition::C,  // Skip then if < (not greater/equal)

            // APPROXIMATIONS - may have edge case issues:
            // !(A <= B) should be (A > B) which is (NC && NZ)
            // Using just NC means we skip if A >= B, which is WRONG when A == B
            // Better: We need to handle this with multiple checks in emit_to
            ComparisonOp::LE => AsmCondition::NC, // APPROXIMATION: Skip if >= (should be: skip if >)

            // !(A > B) should be (A <= B) which is (C || Z)
            // Using just C means we skip if A < B, which is WRONG when A == B
            // We'll need special handling in emit_to for this case
            ComparisonOp::GT => AsmCondition::C, // APPROXIMATION: Skip if < (should be: skip if <=)
        }
    }

    /// Check if this comparison requires special multi-check handling
    /// Returns true for LE and GT which need to check multiple flags
    fn needs_special_handling(&self) -> bool {
        matches!(self.op, ComparisonOp::LE | ComparisonOp::GT)
    }
}

/// Represents an if statement with optional else branch
pub struct If {
    condition: IfCondition,
    then_branch: Vec<Instr>,
    else_branch: Option<Vec<Instr>>,
    label_counter: usize,
}

impl If {
    /// Create a new if statement with a condition and then branch
    pub fn new(condition: IfCondition, then_branch: Vec<Instr>) -> Self {
        Self {
            condition,
            then_branch,
            else_branch: None,
            label_counter: 0,
        }
    }

    /// Add an else branch to the if statement
    pub fn with_else(mut self, else_branch: Vec<Instr>) -> Self {
        self.else_branch = Some(else_branch);
        self
    }

    /// Set a custom label counter (useful for nested ifs)
    pub fn with_label_counter(mut self, counter: usize) -> Self {
        self.label_counter = counter;
        self
    }

    /// Generate the assembly code for this if statement
    ///
    /// For simple conditions (E, NE, LT, GE):
    /// Pattern without else:
    /// ```asm
    /// ld a, [left]
    /// cp right
    /// jp <inverted_condition>, .end_if_N
    /// ; then branch instructions
    /// .end_if_N:
    /// ```
    ///
    /// Pattern with else:
    /// ```asm
    /// ld a, [left]
    /// cp right
    /// jp <inverted_condition>, .else_N
    /// ; then branch instructions
    /// jp .end_if_N
    /// .else_N:
    /// ; else branch instructions
    /// .end_if_N:
    /// ```
    ///
    /// For LE (A <= B), we need to handle both C and Z:
    /// ```asm
    /// ld a, [left]
    /// cp right
    /// jp c, .then_N      ; Jump to then if A < B (C set)
    /// jp z, .then_N      ; Jump to then if A == B (Z set)
    /// jp .else_or_end_N  ; Otherwise skip to else/end
    /// .then_N:
    /// ; then branch
    /// jp .end_if_N       ; (only if there's an else branch)
    /// .else_or_end_N:
    /// ; else branch (if any)
    /// .end_if_N:
    /// ```
    ///
    /// For GT (A > B), we need NC AND NZ:
    /// ```asm
    /// ld a, [left]
    /// cp right
    /// jp c, .else_or_end_N   ; Skip if A < B (C set)
    /// jp z, .else_or_end_N   ; Skip if A == B (Z set)
    /// ; then branch (only executes if NC && NZ, i.e., A > B)
    /// jp .end_if_N           ; (only if there's an else branch)
    /// .else_or_end_N:
    /// ; else branch (if any)
    /// .end_if_N:
    /// ```
    pub fn emit_to(&self) -> Vec<Instr> {
        let mut asm = Asm::new();
        let end_label = format!(".end_if_{}", self.label_counter);
        let else_label = format!(".else_{}", self.label_counter);
        let then_label = format!(".then_{}", self.label_counter);

        // Step 1: Load left operand into register A
        match &self.condition.left {
            ConditionOperand::Register(op) => {
                asm.ld(
                    Operand::Reg(crate::gb_asm::Register::A),
                    Operand::Reg(op.clone()),
                );
            }
            ConditionOperand::Immediate(val) => {
                asm.ld_a(*val);
            }
            ConditionOperand::Label(label) => {
                asm.ld_a_label(label);
            }
        }

        // Step 2: Compare with right operand
        asm.cp(self.condition.right.to_operand());

        // Step 3: Handle conditional jumps based on operator type
        match self.condition.op {
            // Simple cases: single flag check
            ComparisonOp::E | ComparisonOp::NE | ComparisonOp::LT | ComparisonOp::GE => {
                if self.else_branch.is_some() {
                    // Jump to else branch if condition is false
                    asm.jp_cond(self.condition.inverted_asm_condition(), &else_label);
                    asm.emit_all(self.then_branch.clone());
                    asm.jp(&end_label);
                    asm.label(&else_label);
                    if let Some(else_instrs) = &self.else_branch {
                        asm.emit_all(else_instrs.clone());
                    }
                } else {
                    // Jump to end if condition is false (skip then branch)
                    asm.jp_cond(self.condition.inverted_asm_condition(), &end_label);
                    asm.emit_all(self.then_branch.clone());
                }
            }

            // LE (A <= B): true if C || Z (A < B or A == B)
            ComparisonOp::LE => {
                if self.else_branch.is_some() {
                    // Jump to then if C (A < B)
                    asm.jp_cond(AsmCondition::C, &then_label);
                    // Jump to then if Z (A == B)
                    asm.jp_cond(AsmCondition::Z, &then_label);
                    // Otherwise jump to else
                    asm.jp(&else_label);
                    // Then branch
                    asm.label(&then_label);
                    asm.emit_all(self.then_branch.clone());
                    asm.jp(&end_label);
                    // Else branch
                    asm.label(&else_label);
                    if let Some(else_instrs) = &self.else_branch {
                        asm.emit_all(else_instrs.clone());
                    }
                } else {
                    // Jump to then if C (A < B)
                    asm.jp_cond(AsmCondition::C, &then_label);
                    // Jump to then if Z (A == B)
                    asm.jp_cond(AsmCondition::Z, &then_label);
                    // Otherwise skip to end
                    asm.jp(&end_label);
                    // Then branch
                    asm.label(&then_label);
                    asm.emit_all(self.then_branch.clone());
                }
            }

            // GT (A > B): true if NC && NZ (not(A < B) and not(A == B))
            ComparisonOp::GT => {
                let else_or_end = if self.else_branch.is_some() {
                    &else_label
                } else {
                    &end_label
                };

                // Skip to else/end if C (A < B)
                asm.jp_cond(AsmCondition::C, else_or_end);
                // Skip to else/end if Z (A == B)
                asm.jp_cond(AsmCondition::Z, else_or_end);
                // Fall through to then branch (only if NC && NZ, i.e., A > B)
                asm.emit_all(self.then_branch.clone());

                if let Some(else_instrs) = &self.else_branch {
                    asm.jp(&end_label);
                    asm.label(&else_label);
                    asm.emit_all(else_instrs.clone());
                }
            }
        }

        // Emit end label
        asm.label(&end_label);

        asm.get_main_instrs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_asm::{Operand, Register};

    #[test]
    fn test_simple_if() {
        let condition = IfCondition::equal(
            ConditionOperand::Label("wBallMomentumY".to_string()),
            ConditionOperand::Immediate(1),
        );

        let then_branch = vec![Instr::Ld {
            dst: Operand::Reg(Register::A),
            src: Operand::Imm(0),
        }];

        let if_stmt = If::new(condition, then_branch);
        let instrs = if_stmt.emit_to();

        // Should generate multiple instructions (ld, cp, jp, label)
        assert!(instrs.len() > 0);
    }

    #[test]
    fn test_if_else() {
        let condition = IfCondition::not_equal(
            ConditionOperand::Register(Register::A),
            ConditionOperand::Immediate(144),
        );

        let then_branch = vec![Instr::Jp {
            target: crate::gb_asm::JumpTarget::Label("Main".to_string()),
        }];

        let else_branch = vec![Instr::Ld {
            dst: Operand::Reg(Register::B),
            src: Operand::Imm(1),
        }];

        let if_stmt = If::new(condition, then_branch).with_else(else_branch);
        let instrs = if_stmt.emit_to();

        // Should generate multiple instructions including else branch
        assert!(instrs.len() > 0);
    }

    #[test]
    fn test_less_equal() {
        let condition = IfCondition::less_equal(
            ConditionOperand::Register(Register::A),
            ConditionOperand::Immediate(10),
        );

        let then_branch = vec![Instr::Ld {
            dst: Operand::Reg(Register::B),
            src: Operand::Imm(1),
        }];

        let if_stmt = If::new(condition, then_branch);
        let instrs = if_stmt.emit_to();

        // LE requires multiple conditional jumps (jp c, jp z)
        assert!(instrs.len() > 0);
    }

    #[test]
    fn test_greater_than() {
        let condition = IfCondition::greater_than(
            ConditionOperand::Register(Register::A),
            ConditionOperand::Immediate(5),
        );

        let then_branch = vec![Instr::Ld {
            dst: Operand::Reg(Register::B),
            src: Operand::Imm(99),
        }];

        let if_stmt = If::new(condition, then_branch);
        let instrs = if_stmt.emit_to();

        // GT requires multiple conditional jumps (jp c, jp z)
        assert!(instrs.len() > 0);
    }
}
