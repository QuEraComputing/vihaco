// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Run with:
//! `cargo run -p vihaco-stdlib --features program-image-execution --example debugger`

use vihaco_abi::{Type, Value};
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
}

type Instruction = <Machine as HasInstructionSet>::Runtime;
type Program = LocalModule<Instruction, Value, Type>;
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

// Program instructions update frames, operands, and control flow directly.
impl<I> Step<field!(Machine::execution), I, Program> for Machine
where
    ExecutionState<DebuggerState>: Execute<I, Effect = (), Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::execution), I, Program>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
        let message =
            <Self as BuildMessage<field!(Machine::execution), I, Program>>::build_message(
                self,
                instruction,
                module,
            )?;
        self.execution.execute(instruction, message)
    }
}

// Applying Break suspends the invocation at the current breakpoint.
impl Step<DebuggerField, debug::Breakpoint, Program> for Machine {
    fn step(&mut self, instruction: &debug::Breakpoint, module: &Program) -> eyre::Result<()> {
        let message =
            <Self as BuildMessage<DebuggerField, debug::Breakpoint, Program>>::build_message(
                self,
                instruction,
                module,
            )?;
        let effect = self.debugger.execute(instruction, message)?;
        self.execution.apply(effect)?;
        Ok(())
    }
}

fn image(value: u64) -> Image<Instruction> {
    Image::new(Program {
        code: vec![
            Instruction::V_execution__program__ConstU64(program::ConstU64(value)),
            Instruction::V_execution__program__Return(program::Return(1)),
        ],
        main_function: Some(0),
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![],
                ret: vec![Type::U64],
            },
            local_count: 0,
            start_address: 0,
            end_address: 2,
            file: 0,
        }],
        ..Default::default()
    })
}

fn main() -> eyre::Result<()> {
    let machine = Machine {
        execution: ExecutionState::default(),
        debugger: Default::default(),
    };
    let mut loaded = Loadable::new(image(42), machine);
    loaded.set_breakpoint::<DebuggerField>(0);
    loaded.start(&[])?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert_eq!(loaded.machine().execution.pc(), 0);
    assert!(matches!(
        loaded.machine().execution.run_state(),
        RunState::Other(DebuggerState::Breakpoint { .. })
    ));

    // Ordinary resume leaves the breakpoint suspension unchanged.
    loaded.resume()?;
    assert_eq!(loaded.machine().execution.pc(), 0);

    // Step executes the saved ConstU64 once; Continue then executes Return.
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Step)?;
    assert_eq!(loaded.machine().execution.stack(), &[42]);
    assert_eq!(loaded.machine().execution.pc(), 1);
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    assert!(loaded.machine().execution.is_halted());

    // Forget old instructions only AFTER replacement succeeds. If replacement
    // fails, the installed image still needs its saved breakpoint instructions.
    loaded.replace_image(image(99))?;
    loaded.machine_mut().debugger.forget_breakpoints();
    loaded.execute(&[])?;
    assert_eq!(loaded.machine().execution.stack(), &[99]);

    // When abandoning an invocation to replace its image, reset first. Reset
    // alone preserves installed breakpoints, so replacement still needs cleanup.
    loaded.set_breakpoint::<DebuggerField>(0);
    loaded.start(&[])?;
    loaded.resume_with_breaks::<DebuggerField>(DebugAction::Continue)?;
    loaded.reset();
    loaded.replace_image(image(7))?;
    loaded.machine_mut().debugger.forget_breakpoints();

    // Reusing a machine with a different image has the same cleanup contract.
    let (_, mut machine) = loaded.into_parts();
    machine.debugger.forget_breakpoints();
    let mut reused = Loadable::new(image(123), machine);
    reused.execute(&[])?;
    assert_eq!(reused.machine().execution.stack(), &[123]);
    Ok(())
}
