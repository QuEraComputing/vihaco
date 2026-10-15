// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]

use vihaco::traits::{StackFrame, StackMemory};
use vihaco::{BuildMessage, Execute, NoMessage, Step, composite, field};
pub use vihaco_cpu::{__vihaco_fields_CPU, __vihaco_inventory_CPU};
use vihaco_cpu::{
    CPU, StepOutcome, cpu, decode_function_ref, decode_string_id, encode_function_ref,
    encode_string_id, message,
};

#[composite]
#[derive(Default)]
struct Machine {
    #[device(0)]
    cpu: CPU,
    functions: Vec<message::FunctionInfo>,
    strings: Vec<String>,
    outcomes: Vec<StepOutcome>,
    print_messages_built: usize,
}

impl BuildMessage<field!(Machine::cpu), cpu::Call, message::FunctionInfo> for Machine {
    fn build_message(&mut self, instruction: &cpu::Call) -> eyre::Result<message::FunctionInfo> {
        let function = self
            .functions
            .iter()
            .find(|f| f.start_address == instruction.1)
            .ok_or_else(|| eyre::eyre!("unknown call target"))?;
        eyre::ensure!(function.arity == instruction.0, "call arity mismatch");
        Ok(*function)
    }
}

impl BuildMessage<field!(Machine::cpu), cpu::IndirectCall, message::FunctionInfo> for Machine {
    fn build_message(&mut self, _: &cpu::IndirectCall) -> eyre::Result<message::FunctionInfo> {
        let index = decode_function_ref(*self.cpu.stack_top()?);
        self.functions
            .get(index as usize)
            .copied()
            .ok_or_else(|| eyre::eyre!("unknown function reference"))
    }
}

impl BuildMessage<field!(Machine::cpu), cpu::Print, message::Print> for Machine {
    fn build_message(&mut self, _: &cpu::Print) -> eyre::Result<message::Print> {
        let index = decode_string_id(*self.cpu.stack_top()?);
        let text = self
            .strings
            .get(index as usize)
            .ok_or_else(|| eyre::eyre!("unknown string reference"))?;
        let message = message::Print(text.clone());
        self.print_messages_built += 1;
        Ok(message)
    }
}

// The consumer uses the default M while the builders above name M explicitly.
// All CPU instructions share this Step, including the runtime input cases.
impl<I> Step<field!(Machine::cpu), I> for Machine
where
    CPU: Execute<I, Effect = StepOutcome, Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::cpu), I>,
{
    fn step(&mut self, instruction: I) -> eyre::Result<()> {
        let message =
            <Self as BuildMessage<field!(Machine::cpu), I>>::build_message(self, &instruction)?;
        let outcome = self.cpu.execute(&instruction, message)?;
        self.outcomes.push(outcome);
        Ok(())
    }
}

fn step<I>(machine: &mut Machine, instruction: I) -> eyre::Result<()>
where
    CPU: Execute<I>,
    Machine: Step<field!(Machine::cpu), I>,
{
    <Machine as Step<field!(Machine::cpu), I>>::step(machine, instruction)
}

fn machine() -> Machine {
    Machine {
        functions: vec![message::FunctionInfo {
            arity: 1,
            start_address: 20,
            local_count: 3,
        }],
        strings: vec!["hello".into()],
        ..Default::default()
    }
}

#[test]
fn blanket_no_message_builder_and_shared_step_execute_arithmetic() -> eyre::Result<()> {
    let mut machine = machine();
    step(&mut machine, cpu::ConstU64(7))?;
    step(&mut machine, cpu::ConstU64(11))?;
    step(&mut machine, cpu::AddU64)?;
    assert_eq!(*machine.cpu.stack_top()?, 18);
    assert_eq!(machine.outcomes, vec![StepOutcome::Continue; 3]);
    Ok(())
}

#[test]
fn direct_and_indirect_calls_use_resolved_metadata() -> eyre::Result<()> {
    let mut direct = machine();
    direct.cpu.stack_push(9u64);
    step(&mut direct, cpu::Call(1, 20))?;
    assert_eq!(direct.cpu.take_pending_pc(), Some(20));
    assert_eq!(direct.cpu.get_frame()?.local_count, 3);

    let mut indirect = machine();
    indirect.cpu.stack_push(9u64);
    indirect.cpu.stack_push(encode_function_ref(0));
    step(&mut indirect, cpu::IndirectCall)?;
    assert_eq!(indirect.cpu.take_pending_pc(), Some(20));
    assert_eq!(indirect.cpu.get_frame()?.local_count, 3);
    assert_eq!(indirect.cpu.get_frame()?.function, Some(0));
    Ok(())
}

#[test]
fn print_builder_resolves_text_without_consuming_the_operand() -> eyre::Result<()> {
    let mut machine = machine();
    machine.cpu.stack_push(encode_string_id(0));
    let message = <Machine as BuildMessage<field!(Machine::cpu), cpu::Print>>::build_message(
        &mut machine,
        &cpu::Print,
    )?;
    assert_eq!(message.0, "hello");
    assert_eq!(machine.print_messages_built, 1);
    assert_eq!(*machine.cpu.stack_top()?, encode_string_id(0));
    step(&mut machine, cpu::Print)?;
    assert_eq!(machine.print_messages_built, 2);
    assert!(machine.cpu.stack_top().is_err());
    Ok(())
}

#[test]
fn preparation_error_prevents_execution() {
    let mut machine = machine();
    machine.cpu.stack_push(encode_function_ref(9));
    let before = *machine.cpu.stack_top().unwrap();
    let error = step(&mut machine, cpu::IndirectCall).unwrap_err();
    assert!(error.to_string().contains("unknown function reference"));
    assert_eq!(*machine.cpu.stack_top().unwrap(), before);
    assert!(machine.cpu.get_frame().is_err());
    assert!(machine.outcomes.is_empty());
}

#[test]
fn explicit_message_parameter_is_still_the_execute_message() {
    fn require_builder<C, F, I, M>()
    where
        C: BuildMessage<F, I, M>,
        C::Child: Execute<I, Message = M>,
    {
    }
    require_builder::<Machine, field!(Machine::cpu), cpu::ConstU64, NoMessage>();
    require_builder::<Machine, field!(Machine::cpu), cpu::Call, message::FunctionInfo>();
    require_builder::<Machine, field!(Machine::cpu), cpu::IndirectCall, message::FunctionInfo>();
    require_builder::<Machine, field!(Machine::cpu), cpu::Print, message::Print>();
}
