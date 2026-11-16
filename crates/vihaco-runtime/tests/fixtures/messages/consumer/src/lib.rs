// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(test)]

use message_author::{Device, QubitBatch, operations};
use vihaco::{
    BuildMessage, Execute, ExecutionState, PopStack, PushStack, ReusableMessageRecipe, composite,
    eyre, field,
};

fn running_execution() -> ExecutionState {
    use vihaco::module::{FunctionInfo, LocalModule, Signature};
    let program: LocalModule<(), vihaco::Value, vihaco::Type> = LocalModule {
        code: vec![()],
        main_function: Some(0),
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![],
                ret: vec![],
            },
            local_count: 0,
            start_address: 0,
            end_address: 1,
            file: 0,
        }],
        ..Default::default()
    };
    let mut execution = ExecutionState::new();
    execution.start(&program, &[]).unwrap();
    execution
}

#[composite]
struct DelegatingMachine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    device: message_author::Device,
}

impl<I> BuildMessage<field!(DelegatingMachine::device), I, str, QubitBatch> for DelegatingMachine
where
    Device: Execute<I, Message = QubitBatch>,
    QubitBatch: ReusableMessageRecipe<Self, field!(DelegatingMachine::device), I, str>,
{
    fn build_message(&mut self, instruction: &I, context: &str) -> eyre::Result<QubitBatch> {
        <QubitBatch as ReusableMessageRecipe<
            Self, field!(DelegatingMachine::device), I, str,
        >>::prepare(self, instruction, context)
    }
}

#[composite]
struct CustomMachine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    device: message_author::Device,
    selected: Vec<u64>,
}

// The same upstream message has a matching recipe, but this machine chooses
// its own construction. A generic I must remain legal across the crate boundary.
impl<I> BuildMessage<field!(CustomMachine::device), I, str, QubitBatch> for CustomMachine
where
    Device: Execute<I, Message = QubitBatch>,
{
    fn build_message(&mut self, _: &I, _: &str) -> eyre::Result<QubitBatch> {
        Ok(QubitBatch(self.selected.clone()))
    }
}

#[test]
fn composite_explicitly_uses_an_upstream_recipe_with_unsized_context() -> eyre::Result<()> {
    let mut machine = DelegatingMachine {
        execution: running_execution(),
        device: Device,
    };
    for value in [7_u64, 11, 13] {
        machine.execution.push(value);
    }
    let message = machine.build_message(&operations::Batch(2), "context")?;
    assert_eq!(
        machine.device.execute(&operations::Batch(2), message)?,
        [11, 13]
    );
    assert_eq!(machine.execution.stack(), &[7]);
    let empty = machine.build_message(&operations::Batch(0), "context")?;
    assert!(empty.0.is_empty());
    assert_eq!(machine.execution.stack(), &[7]);
    Ok(())
}

#[test]
fn generic_custom_builder_can_replace_an_applicable_upstream_recipe() -> eyre::Result<()> {
    let mut machine = CustomMachine {
        execution: running_execution(),
        device: Device,
        selected: vec![21, 34],
    };
    machine.execution.push(99_u64);
    let message = machine.build_message(&operations::Batch(2), "context")?;
    assert_eq!(message.0, [21, 34]);
    assert_eq!(machine.execution.stack(), &[99]);
    // The recipe is also applicable to this machine; using it is a choice.
    let message = <QubitBatch as ReusableMessageRecipe<
        CustomMachine,
        field!(CustomMachine::device),
        operations::Batch,
        str,
    >>::prepare(&mut machine, &operations::Batch(1), "context")?;
    assert_eq!(message.0, [99]);
    Ok(())
}

#[test]
fn failed_recipe_retains_successful_operand_consumption() {
    let mut machine = DelegatingMachine {
        execution: running_execution(),
        device: Device,
    };
    machine.execution.push(7_u64);
    assert!(
        machine
            .build_message(&operations::Batch(2), "context")
            .is_err()
    );
    assert!(machine.execution.stack().is_empty());
    assert!(PopStack::<u64>::pop(&mut machine.execution).is_err());
}
