// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A program machine that finishes each static route before the next fetch.

use crate::{
    checked::{CheckedDriver, Inline, Module, Pause, StepContext},
    framework::{
        Describe, Route, apply_effect_at, build_message_at, instruction_at, step_graph,
        synchronous::RunSync,
    },
};
use eyre::Result;
use vihaco_abi::Type;
use vihaco_cpu::{CPU, cpu};
use vihaco_module::module::{FunctionInfo, Signature};
use vihaco_runtime::{
    Apply, BuildMessage, Execute, HasInstructionSet, ReusableMessageRecipe, Step, composite, field,
};
use vihaco_stdlib::{
    ExecutionState, Image, Loadable,
    program::program,
    program_driver::{ProgramDriver, ProgramStop, RunCommand},
};

#[composite]
struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState<Pause>,
    #[device(1)]
    cpu: vihaco_cpu::CPU,
}

type Instruction = <Machine as HasInstructionSet>::Runtime;
type Program = Module<Instruction>;
type ExecutionField = field!(Machine::execution);
type CpuField = field!(Machine::cpu);

impl<I, Message> BuildMessage<ExecutionField, I, Program, Message> for Machine
where
    ExecutionState<Pause>: Execute<I, Message = Message>,
    Message: ReusableMessageRecipe<Self, ExecutionField, I, Program>,
{
    fn build_message(&mut self, instruction: &I, program: &Program) -> Result<Message> {
        Message::prepare(self, instruction, program)
    }
}

impl<I, Message> BuildMessage<CpuField, I, Program, Message> for Machine
where
    CPU: Execute<I, Message = Message>,
    Message: ReusableMessageRecipe<Self, CpuField, I, Program>,
{
    fn build_message(&mut self, instruction: &I, program: &Program) -> Result<Message> {
        Message::prepare(self, instruction, program)
    }
}

fn execution_route<I>()
-> Route<I, impl for<'a> RunSync<Machine, (&'a I, &'a Program), Output = ()> + Describe>
where
    ExecutionState<Pause>: Execute<I, Effect = (), Error = eyre::Report>,
    Machine: BuildMessage<ExecutionField, I, Program>,
{
    step_graph::<I>()
        .begin(build_message_at::<ExecutionField>())
        .then_execute(instruction_at::<ExecutionField>())
        .finish()
}

fn cpu_route<I>()
-> Route<I, impl for<'a> RunSync<Machine, (&'a I, &'a Program), Output = ()> + Describe>
where
    CPU: Execute<I, Error = eyre::Report>,
    Machine: BuildMessage<CpuField, I, Program>,
    ExecutionState<Pause>:
        Apply<<CPU as Execute<I>>::Effect, Effect = (), Error: Into<eyre::Report>>,
{
    step_graph::<I>()
        .begin(build_message_at::<CpuField>())
        .then_execute(instruction_at::<CpuField>())
        .then(apply_effect_at::<ExecutionField>())
        .finish()
}

// The runner lends each Step the current instruction and its module.
impl<I> Step<ExecutionField, I, StepContext<'_, Instruction, Inline>> for Machine
where
    ExecutionState<Pause>: Execute<I, Effect = (), Error = eyre::Report>,
    Self: BuildMessage<ExecutionField, I, Program>,
{
    fn step(
        &mut self,
        instruction: &I,
        context: &StepContext<'_, Instruction, Inline>,
    ) -> Result<()> {
        execution_route::<I>().run_synchronously(self, instruction, context.module)
    }
}

// Arithmetic effects are applied before the runner can fetch the next instruction.
impl<I> Step<CpuField, I, StepContext<'_, Instruction, Inline>> for Machine
where
    CPU: Execute<I, Error = eyre::Report>,
    Self: BuildMessage<CpuField, I, Program>,
    ExecutionState<Pause>:
        Apply<<CPU as Execute<I>>::Effect, Effect = (), Error: Into<eyre::Report>>,
{
    fn step(
        &mut self,
        instruction: &I,
        context: &StepContext<'_, Instruction, Inline>,
    ) -> Result<()> {
        cpu_route::<I>().run_synchronously(self, instruction, context.module)
    }
}

fn program() -> Program {
    Program {
        code: vec![
            // Branch skips the early halt. The CPU route leaves 6 * 7 on the stack.
            Instruction::V_execution__program__Branch(program::Branch(2)),
            Instruction::V_execution__program__Halt(program::Halt),
            Instruction::V_execution__program__ConstU64(program::ConstU64(6)),
            Instruction::V_execution__program__ConstU64(program::ConstU64(7)),
            Instruction::V_cpu__cpu__MulU64(cpu::MulU64),
            Instruction::V_execution__program__Return(program::Return(1)),
        ],
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![],
                ret: vec![Type::U64],
            },
            local_count: 0,
            start_address: 0,
            end_address: 6,
            file: 0,
        }],
        strings: vec!["main".into()],
        main_function: Some(0),
        ..Default::default()
    }
}

fn load(module: Program) -> CheckedDriver<Machine, Instruction, Inline> {
    let machine = Machine {
        execution: ExecutionState::default(),
        cpu: CPU::default(),
    };
    CheckedDriver::new(Loadable::new(Image::new(module), machine), Inline)
}

pub(crate) fn demo() -> Result<()> {
    let control = execution_route::<program::Branch>();
    let arithmetic = cpu_route::<cpu::MulU64>();
    assert_eq!(std::mem::size_of_val(&control), 0);
    assert_eq!(std::mem::size_of_val(&arithmetic), 0);
    control.topology().print("Branch");
    arithmetic.topology().print("MulU64");

    let mut loaded = load(program());
    loaded.start((), &[])?;
    assert_eq!(
        loaded.run((), RunCommand::Continue)?,
        ProgramStop::Halted { pc: 5 }
    );
    let execution = &loaded.machine().execution;
    assert_eq!(execution.stack(), &[42]);
    assert!(execution.frames().is_empty());
    assert!(execution.is_halted());
    assert_eq!(execution.pc(), 5);
    println!("Result stack: {:?}", execution.stack());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_static_routes_in_program_order() -> Result<()> {
        demo()
    }

    #[test]
    fn checks_the_current_module_before_executing_a_branch() -> Result<()> {
        let mut module = program();
        module.code[0] = Instruction::V_execution__program__Branch(program::Branch(u32::MAX));
        let mut loaded = load(module);

        loaded.start((), &[])?;
        let error = loaded
            .run((), RunCommand::Continue)
            .expect_err("the branch target is outside the program");
        assert_eq!(error.to_string(), "branch target out of bounds");
        assert_eq!(loaded.machine().execution.pc(), 0);
        assert!(loaded.machine().execution.stack().is_empty());
        Ok(())
    }
}
