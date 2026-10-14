//! Repeating control flow for Game Boy programs.

use crate::asm::{Block, Condition, LabelAllocator, R8, R16Stack};

use super::Emittable;

/// Repeat a body while a condition emitter leaves a nonzero value in `a`.
///
/// The condition code is emitted at the top of the loop, so it is evaluated on every
/// iteration. The condition must leave its truth value in `a` and preserve the stack.
pub struct While {
    condition: Box<dyn Emittable>,
    body: Box<dyn Emittable>,
}

impl While {
    pub fn new(condition: impl Emittable + 'static, body: impl Emittable + 'static) -> Self {
        Self {
            condition: Box::new(condition),
            body: Box::new(body),
        }
    }
}

impl Emittable for While {
    fn emit(&mut self, labels: &LabelAllocator) -> Vec<crate::asm::Instr> {
        let [start, end] = labels.locals(["while", "end_while"]);
        let mut code = Block::new();
        code.label(&start)
            .emit_all(self.condition.emit(labels))
            .or(R8::A)
            .jr_cond(Condition::Z, &end)
            .emit_all(self.body.emit(labels))
            .jp(&start)
            .label(&end);
        code.into_instrs()
    }
}

/// Execute a body a fixed number of times, from zero through 255.
///
/// Clobbers A, B and flags plus the body’s changes outside BC. A zero count skips the body.
/// Saves BC around each iteration, so bodies may use both registers and nested loops work.
/// Body changes to BC are discarded. Each active iteration uses two stack bytes; the body
/// must preserve stack balance.
pub struct For {
    count: u8,
    body: Box<dyn Emittable>,
}

impl For {
    pub fn times(count: u8, body: impl Emittable + 'static) -> Self {
        Self {
            count,
            body: Box::new(body),
        }
    }
}

impl Emittable for For {
    fn emit(&mut self, labels: &LabelAllocator) -> Vec<crate::asm::Instr> {
        let [start, end] = labels.locals(["for", "end_for"]);
        let mut code = Block::new();
        code.ld(R8::B, self.count)
            .ld(R8::A, R8::B)
            .or(R8::A)
            .jr_cond(Condition::Z, &end)
            .label(&start)
            .push(R16Stack::BC)
            .emit_all(self.body.emit(labels))
            .pop(R16Stack::BC)
            .dec(R8::B)
            .jr_cond(Condition::NZ, &start)
            .label(&end);
        code.into_instrs()
    }
}

/// Select one body by comparing the current value in `a` with byte cases.
///
/// The first matching case runs. The value in `a` is unchanged by selection; case bodies
/// may change it. A default body is optional and runs when no case matches.
pub struct SwitchA {
    cases: Vec<(u8, Box<dyn Emittable>)>,
    default: Option<Box<dyn Emittable>>,
}

impl SwitchA {
    pub fn new() -> Self {
        Self {
            cases: Vec::new(),
            default: None,
        }
    }

    pub fn case(mut self, value: u8, body: impl Emittable + 'static) -> Self {
        self.cases.push((value, Box::new(body)));
        self
    }

    pub fn default(mut self, body: impl Emittable + 'static) -> Self {
        self.default = Some(Box::new(body));
        self
    }
}

impl Default for SwitchA {
    fn default() -> Self {
        Self::new()
    }
}

impl Emittable for SwitchA {
    fn emit(&mut self, labels: &LabelAllocator) -> Vec<crate::asm::Instr> {
        let end = labels.local("end_switch");
        let mut code = Block::new();
        for (value, body) in &mut self.cases {
            let next = labels.local("switch_next");
            code.cp_imm(*value)
                .jr_cond(Condition::NZ, &next)
                .emit_all(body.emit(labels))
                .jp(&end)
                .label(&next);
        }
        if let Some(default) = &mut self.default {
            code.emit_all(default.emit(labels));
        }
        code.label(&end);
        code.into_instrs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::{Block, R8, test_cpu::TestCpu};

    #[test]
    fn for_times_handles_zero_one_and_multiple_iterations() {
        for (count, expected) in [(0, 0), (1, 1), (5, 5)] {
            let mut body = Block::new();
            body.inc(R8::D);
            let mut loop_code = For::times(count, body);
            let mut cpu = TestCpu::default();
            cpu.run(&loop_code.emit(&LabelAllocator::new()));
            assert_eq!(cpu.d, expected);
        }
    }

    #[test]
    fn nested_for_and_a_body_writing_b_keep_the_outer_count() {
        let mut increment = Block::new();
        increment.inc(R8::D).ld_b(99);
        let mut code = For::times(3, For::times(4, increment));
        let mut cpu = TestCpu::default();
        cpu.run(&code.emit(&LabelAllocator::new()));
        assert_eq!(cpu.d, 12);
    }

    #[test]
    fn mixed_nested_control_flow_has_unique_local_labels() {
        use crate::asm::{Asm, Section};
        use crate::stdlib::flow::IfA;
        let mut condition = Block::new();
        condition.ld_a(0);
        let mut action = Block::new();
        action.nop();
        let switch = SwitchA::new().case(1, action);
        let nested = IfA::eq(1, For::times(2, While::new(condition, switch)));
        let mut asm = Asm::new();
        asm.section(Section::rom0("Flow"))
            .label("Main")
            .emit_code(nested)
            .ret();
        crate::asm::label_check::assert_links(&asm.to_asm());
    }

    #[test]
    fn while_rechecks_its_condition_on_each_iteration() {
        let mut condition = Block::new();
        condition.ld(R8::A, R8::C).dec(R8::C);
        let mut body = Block::new();
        body.inc(R8::D);
        let mut loop_code = While::new(condition, body);
        let mut cpu = TestCpu::default();
        cpu.c = 4;
        cpu.run(&loop_code.emit(&LabelAllocator::new()));
        assert_eq!(cpu.d, 4);
    }

    #[test]
    fn switch_runs_the_first_matching_case_or_default() {
        for (input, expected) in [(1, 0x11), (2, 0x22), (3, 0x33)] {
            let mut first = Block::new();
            first.ld(R8::D, 0x11);
            let mut second = Block::new();
            second.ld(R8::D, 0x22);
            let mut otherwise = Block::new();
            otherwise.ld(R8::D, 0x33);
            let mut switch = SwitchA::new()
                .case(1, first)
                .case(2, second)
                .default(otherwise);
            let mut cpu = TestCpu::default();
            cpu.a = input;
            cpu.run(&switch.emit(&LabelAllocator::new()));
            assert_eq!(cpu.d, expected);
        }
    }
}
