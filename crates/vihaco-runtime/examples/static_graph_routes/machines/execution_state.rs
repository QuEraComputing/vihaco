// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A program machine that finishes each static route before the next fetch.

use crate::framework::{
    Compute, Describe, Route, compute, instruction_at, step_graph,
    synchronous::{RunSync, build_message_with},
};
use eyre::Result;
use std::marker::PhantomData;
use vihaco_abi::{Type, Value};
use vihaco_cpu::{CPU, cpu};
use vihaco_module::module::{FunctionInfo, LocalModule, Parameter, Signature};
use vihaco_runtime::{
    Apply, BuildMessage, Execute, HasInstructionSet, ReusableMessageRecipe, Step, composite, field,
};
use vihaco_stdlib::{ExecutionState, Image, Loadable, program::program};

#[composite]
struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    cpu: vihaco_cpu::CPU,
}

type Instruction = <Machine as HasInstructionSet>::Runtime;
type Program = LocalModule<Instruction, Value, Type>;
type ExecutionField = field!(Machine::execution);
type CpuField = field!(Machine::cpu);

impl<I, Message> BuildMessage<ExecutionField, I, Program, Message> for Machine
where
    ExecutionState: Execute<I, Message = Message>,
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
    ExecutionState: Execute<I, Effect = (), Error = eyre::Report>,
    Machine: BuildMessage<ExecutionField, I, Program>,
{
    step_graph::<I>()
        .begin(build_message_with::<ExecutionField, Program>())
        .then_execute(instruction_at::<ExecutionField>())
        .finish()
}

struct ApplyCpuEffect<E>(PhantomData<fn() -> E>);
impl<E> Compute for ApplyCpuEffect<E>
where
    ExecutionState: Apply<E>,
{
    type Machine = Machine;
    type Input = E;
    type Output = ();
    const NAME: &'static str = "apply CPU effect to the operand stack";

    fn apply(machine: &mut Machine, effect: E) -> Result<()> {
        machine.execution.apply(effect);
        Ok(())
    }
}

fn cpu_route<I>()
-> Route<I, impl for<'a> RunSync<Machine, (&'a I, &'a Program), Output = ()> + Describe>
where
    CPU: Execute<I, Error = eyre::Report>,
    Machine: BuildMessage<CpuField, I, Program>,
    ExecutionState: Apply<<CPU as Execute<I>>::Effect>,
{
    step_graph::<I>()
        .begin(build_message_with::<CpuField, Program>())
        .then_execute(instruction_at::<CpuField>())
        .then(compute::<ApplyCpuEffect<<CPU as Execute<I>>::Effect>>())
        .finish()
}

// The runner lends each Step the current instruction and its module.
impl<I> Step<ExecutionField, I, Program> for Machine
where
    ExecutionState: Execute<I, Effect = (), Error = eyre::Report>,
    Self: BuildMessage<ExecutionField, I, Program>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> Result<()> {
        execution_route::<I>().run_synchronously(self, instruction, module)
    }
}

// Arithmetic effects are applied before the runner can fetch the next instruction.
impl<I> Step<CpuField, I, Program> for Machine
where
    CPU: Execute<I, Error = eyre::Report>,
    Self: BuildMessage<CpuField, I, Program>,
    ExecutionState: Apply<<CPU as Execute<I>>::Effect>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> Result<()> {
        cpu_route::<I>().run_synchronously(self, instruction, module)
    }
}

fn program() -> Program {
    Program {
        code: vec![
            // main: skip the halt, call increment(20), multiply by 2, and return.
            Instruction::V_execution__program__Branch(program::Branch(2)),
            Instruction::V_execution__program__Halt(program::Halt),
            Instruction::V_execution__program__ConstU64(program::ConstU64(20)),
            Instruction::V_execution__program__Call(program::Call(1, 7)),
            Instruction::V_execution__program__ConstU64(program::ConstU64(2)),
            Instruction::V_cpu__cpu__MulU64(cpu::MulU64),
            Instruction::V_execution__program__Return(program::Return(1)),
            // increment(value): return value + 1.
            Instruction::V_execution__program__LoadU64(program::LoadU64(0)),
            Instruction::V_execution__program__ConstU64(program::ConstU64(1)),
            Instruction::V_cpu__cpu__AddU64(cpu::AddU64),
            Instruction::V_execution__program__Return(program::Return(1)),
        ],
        functions: vec![
            FunctionInfo {
                name: 0,
                signature: Signature {
                    params: vec![],
                    ret: vec![Type::U64],
                },
                local_count: 0,
                start_address: 0,
                end_address: 7,
                file: 0,
            },
            FunctionInfo {
                name: 1,
                signature: Signature {
                    params: vec![Parameter {
                        name: 2,
                        ty: Type::U64,
                    }],
                    ret: vec![Type::U64],
                },
                local_count: 1,
                start_address: 7,
                end_address: 11,
                file: 0,
            },
        ],
        strings: vec!["main".into(), "increment".into(), "value".into()],
        main_function: Some(0),
        ..Default::default()
    }
}

fn load(module: Program) -> Loadable<Image<Instruction>, Machine> {
    Loadable::new(
        Image::new(module),
        Machine {
            execution: ExecutionState::new(),
            cpu: CPU::default(),
        },
    )
}

pub(crate) fn demo() -> Result<()> {
    let control = execution_route::<program::Call>();
    let arithmetic = cpu_route::<cpu::MulU64>();
    assert_eq!(std::mem::size_of_val(&control), 0);
    assert_eq!(std::mem::size_of_val(&arithmetic), 0);
    control.topology().print("Call");
    arithmetic.topology().print("MulU64");

    let mut loaded = load(program());
    loaded.execute(&[])?;
    let execution = &loaded.machine().execution;
    assert_eq!(execution.stack(), &[42]);
    assert!(execution.frames().is_empty());
    assert!(execution.is_halted());
    assert_eq!(execution.pc(), 6);
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

        let error = loaded
            .execute(&[])
            .expect_err("the branch target is outside the program");
        assert_eq!(error.to_string(), "branch target out of bounds");
        assert_eq!(loaded.machine().execution.pc(), 0);
        assert!(loaded.machine().execution.stack().is_empty());
        Ok(())
    }
}
