// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "program-image-execution")]

use vihaco_module::module::{FunctionInfo, LocalModule, Signature};
use vihaco_runtime::{
    Apply, BuildMessage, Execute, HasInstructionSet, ReusableMessageRecipe, Step, composite, field,
};
use vihaco_stdlib::{
    ExecutionState, Image, Loadable, RunState,
    debugger::{DebugAction, DebuggerState, HasBreakpointInstruction, dialect::debug},
    program::program,
};

#[composite]
pub struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState<DebuggerState>,
    #[device(1)]
    debugger: vihaco_stdlib::debugger::Debugger<u32, Instruction>,
    fail_next: bool,
    executed: usize,
}

type Instruction = <Machine as HasInstructionSet>::Runtime;
type Program = LocalModule<Instruction, vihaco_abi::Value, vihaco_abi::Type>;
type Loaded = Loadable<Image<Instruction>, Machine>;
type DebuggerField = field!(Machine::debugger);

// This machine delegates message construction to the standard recipes.
impl<I, M> BuildMessage<field!(Machine::execution), I, Program, M> for Machine
where
    ExecutionState<DebuggerState>: Execute<I, Message = M>,
    M: ReusableMessageRecipe<Self, field!(Machine::execution), I, Program>,
{
    fn build_message(&mut self, instruction: &I, context: &Program) -> eyre::Result<M> {
        <M as ReusableMessageRecipe<Self, field!(Machine::execution), I, Program>>::prepare(
            self,
            instruction,
            context,
        )
    }
}

impl BuildMessage<DebuggerField, debug::Breakpoint, Program> for Machine {
    fn build_message(
        &mut self,
        instruction: &debug::Breakpoint,
        context: &Program,
    ) -> eyre::Result<vihaco_stdlib::debugger::BreakpointMessage> {
        <vihaco_stdlib::debugger::BreakpointMessage as ReusableMessageRecipe<
            Self,
            DebuggerField,
            debug::Breakpoint,
            Program,
        >>::prepare(self, instruction, context)
    }
}

impl HasBreakpointInstruction for Instruction {
    fn new_breakpoint() -> Self {
        Self::V_debugger__debug__Breakpoint(debug::Breakpoint)
    }
}

impl<I> Step<field!(Machine::execution), I, Program> for Machine
where
    ExecutionState<DebuggerState>: Execute<I, Effect = (), Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::execution), I, Program>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
        if std::mem::take(&mut self.fail_next) {
            eyre::bail!("injected instruction failure");
        }
        let message =
            <Self as BuildMessage<field!(Machine::execution), I, Program>>::build_message(
                self,
                instruction,
                module,
            )?;
        self.execution.execute(instruction, message)?;
        self.executed += 1;
        Ok(())
    }
}

impl Step<DebuggerField, debug::Breakpoint, Program> for Machine {
    fn step(&mut self, instruction: &debug::Breakpoint, module: &Program) -> eyre::Result<()> {
        let message =
            <Self as BuildMessage<DebuggerField, debug::Breakpoint, Program>>::build_message(
                self,
                instruction,
                module,
            )?;
        let effect = self.debugger.execute(instruction, message)?;
        self.execution.apply(effect);
        Ok(())
    }
}

fn function(start: u32, end: u32) -> FunctionInfo<vihaco_abi::Type> {
    FunctionInfo {
        name: 0,
        signature: Signature {
            params: vec![],
            ret: vec![],
        },
        local_count: 0,
        start_address: start,
        end_address: end,
        file: 0,
    }
}

fn image(code: Vec<Instruction>) -> Image<Instruction> {
    let end = u32::try_from(code.len()).unwrap();
    Image::new(Program {
        code,
        main_function: Some(0),
        functions: vec![function(0, end)],
        ..Default::default()
    })
}

fn loaded(code: Vec<Instruction>) -> Loaded {
    Loadable::new(
        image(code),
        Machine {
            execution: ExecutionState::default(),
            debugger: Default::default(),
            fail_next: false,
            executed: 0,
        },
    )
}

fn constant(value: u64) -> Instruction {
    Instruction::V_execution__program__ConstU64(program::ConstU64(value))
}

fn ret(count: u32) -> Instruction {
    Instruction::V_execution__program__Return(program::Return(count))
}

fn assert_paused(loaded: &Loaded, pc: u32) {
    assert_eq!(loaded.machine().execution.pc(), pc);
    assert_eq!(
        loaded.machine().execution.run_state(),
        &RunState::Other(DebuggerState::Breakpoint { resume_pc: pc + 1 })
    );
}

#[test]
fn explicit_breakpoint_resumes_at_the_following_instruction() -> eyre::Result<()> {
    let mut loaded = loaded(vec![Instruction::new_breakpoint(), constant(42), ret(1)]);
    loaded.start(&[])?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert_paused(&loaded, 0);
    assert!(loaded.machine().execution.stack().is_empty());

    // The ordinary runner leaves a suspended invocation untouched.
    loaded.resume()?;
    assert_paused(&loaded, 0);
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Step)?;
    assert_eq!(loaded.machine().execution.pc(), 2);
    assert_eq!(loaded.machine().execution.stack(), &[42]);
    assert_eq!(loaded.machine().executed, 1);

    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert!(loaded.machine().execution.is_halted());
    assert_eq!(loaded.machine().execution.stack(), &[42]);
    Ok(())
}

#[test]
fn stepping_a_gutter_breakpoint_executes_the_displaced_instruction_once() -> eyre::Result<()> {
    let mut loaded = loaded(vec![constant(42), ret(1)]);
    loaded.set_breakpoint::<DebuggerField>(0);
    loaded.start(&[])?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert_paused(&loaded, 0);
    assert_eq!(loaded.machine().executed, 0);

    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Step)?;
    assert_eq!(loaded.machine().execution.pc(), 1);
    assert_eq!(loaded.machine().execution.stack(), &[42]);
    assert_eq!(loaded.machine().executed, 1);
    assert!(loaded.machine().debugger.contains(&0));

    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert!(loaded.machine().execution.is_halted());
    assert_eq!(loaded.machine().execution.stack(), &[42]);
    assert_eq!(loaded.machine().executed, 2);
    Ok(())
}

#[test]
fn displaced_branch_keeps_its_target_and_breaks_again_on_a_loop() -> eyre::Result<()> {
    let mut loaded = loaded(vec![
        constant(7),
        Instruction::V_execution__program__Branch(program::Branch(0)),
    ]);
    loaded.set_breakpoint::<DebuggerField>(1);
    loaded.start(&[])?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert_paused(&loaded, 1);
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Step)?;
    assert_eq!(loaded.machine().execution.pc(), 0);
    assert_eq!(loaded.machine().execution.run_state(), &RunState::Runnable);

    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert_paused(&loaded, 1);
    assert_eq!(loaded.machine().execution.stack(), &[7, 7]);
    assert_eq!(loaded.machine().executed, 3);
    Ok(())
}

#[test]
fn clearing_a_gutter_breakpoint_while_paused_executes_the_restored_instruction() -> eyre::Result<()>
{
    let mut loaded = loaded(vec![constant(42), ret(1)]);
    loaded.set_breakpoint::<DebuggerField>(0);
    loaded.start(&[])?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert_paused(&loaded, 0);

    loaded.clear_breakpoint::<DebuggerField>(0);
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert!(loaded.machine().execution.is_halted());
    assert_eq!(loaded.machine().execution.stack(), &[42]);
    assert_eq!(loaded.machine().executed, 2);
    assert!(!loaded.machine().debugger.contains(&0));
    Ok(())
}

#[test]
fn failed_displaced_instruction_can_be_retried_without_hitting_the_trap() -> eyre::Result<()> {
    let mut loaded = loaded(vec![constant(42), ret(1)]);
    loaded.set_breakpoint::<DebuggerField>(0);
    loaded.start(&[])?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    loaded.machine_mut().fail_next = true;
    let error = loaded
        .resume_with_breaks::<DebuggerField>(DebugAction::Step)
        .unwrap_err();
    assert!(error.to_string().contains("injected instruction failure"));
    assert_paused(&loaded, 0);
    assert!(loaded.machine().execution.stack().is_empty());

    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Step)?;
    assert_eq!(loaded.machine().execution.pc(), 1);
    assert_eq!(loaded.machine().execution.stack(), &[42]);
    assert_eq!(loaded.machine().executed, 1);
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert!(loaded.machine().execution.is_halted());
    Ok(())
}

#[test]
fn gutter_breakpoint_over_an_explicit_breakpoint_preserves_both_stops() -> eyre::Result<()> {
    let mut loaded = loaded(vec![Instruction::new_breakpoint(), constant(42), ret(1)]);
    loaded.set_breakpoint::<DebuggerField>(0);
    loaded.start(&[])?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert_paused(&loaded, 0);
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert_paused(&loaded, 0);
    assert_eq!(loaded.machine().executed, 0);

    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert!(loaded.machine().execution.is_halted());
    assert_eq!(loaded.machine().execution.stack(), &[42]);
    Ok(())
}

#[test]
fn displaced_call_returns_to_the_instruction_after_the_call() -> eyre::Result<()> {
    let mut loaded = loaded(vec![
        Instruction::V_execution__program__Call(program::Call(0, 2)),
        ret(1),
        constant(42),
        ret(1),
    ]);
    let (mut image, machine) = loaded.into_parts();
    image.module.functions = vec![function(0, 2), function(2, 4)];
    loaded = Loadable::new(image, machine);
    loaded.set_breakpoint::<DebuggerField>(0);
    loaded.start(&[])?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Step)?;
    assert_eq!(loaded.machine().execution.pc(), 2);
    assert_eq!(loaded.machine().execution.frames().len(), 2);

    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert!(loaded.machine().execution.is_halted());
    assert_eq!(loaded.machine().execution.stack(), &[42]);
    assert_eq!(loaded.machine().executed, 4);
    Ok(())
}

#[test]
fn conditional_branches_and_indirect_calls_use_debugger_execution_state() -> eyre::Result<()> {
    for (condition, expected) in [(0, 9), (1, 7)] {
        let mut loaded = loaded(vec![
            constant(condition),
            Instruction::V_execution__program__ConditionalBranch(program::ConditionalBranch(2, 4)),
            constant(7),
            Instruction::V_execution__program__Branch(program::Branch(6)),
            constant(9),
            Instruction::V_execution__program__Branch(program::Branch(6)),
            Instruction::V_execution__program__ConstFunctionRef(program::ConstFunctionRef(
                vihaco_abi::word::encode_function_ref(1),
            )),
            Instruction::V_execution__program__IndirectCall(program::IndirectCall),
            ret(2),
            constant(42),
            ret(1),
        ]);
        let (mut image, machine) = loaded.into_parts();
        image.module.functions = vec![function(0, 9), function(9, 11)];
        loaded = Loadable::new(image, machine);
        loaded.set_breakpoint::<DebuggerField>(1);
        loaded.set_breakpoint::<DebuggerField>(7);
        loaded.start(&[])?;

        loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
        assert_paused(&loaded, 1);
        loaded.resume_with_breaks::<DebuggerField>(DebugAction::Step)?;
        assert_eq!(
            loaded.machine().execution.pc(),
            if condition == 0 { 4 } else { 2 }
        );
        assert!(loaded.machine().execution.stack().is_empty());

        loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
        assert_paused(&loaded, 7);
        loaded.resume_with_breaks::<DebuggerField>(DebugAction::Step)?;
        assert_eq!(loaded.machine().execution.pc(), 9);
        assert_eq!(loaded.machine().execution.frames().len(), 2);
        assert_eq!(loaded.machine().execution.stack(), &[expected]);

        loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
        assert!(loaded.machine().execution.is_halted());
        assert_eq!(loaded.machine().execution.stack(), &[expected, 42]);
    }
    Ok(())
}

#[test]
fn forgetting_breakpoints_allows_replacement_and_machine_reuse() -> eyre::Result<()> {
    let mut loaded = loaded(vec![constant(11), ret(1)]);
    loaded.set_breakpoint::<DebuggerField>(0);
    loaded.start(&[])?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    loaded.reset();
    loaded.replace_image(image(vec![constant(42), ret(1)]))?;
    loaded.machine_mut().debugger.forget_breakpoints();
    loaded.clear_breakpoint::<DebuggerField>(0);
    loaded.set_breakpoint::<DebuggerField>(0);
    loaded.start(&[])?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert_paused(&loaded, 0);
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert_eq!(loaded.machine().execution.stack(), &[42]);

    let (_, mut machine) = loaded.into_parts();
    machine.debugger.forget_breakpoints();
    let mut reused = Loadable::new(image(vec![constant(99), ret(1)]), machine);
    reused.clear_breakpoint::<DebuggerField>(0);
    reused.execute(&[])?;
    assert_eq!(reused.machine().execution.stack(), &[99]);
    Ok(())
}
