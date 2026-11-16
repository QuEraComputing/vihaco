// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::{Execute, HasOperandStack, PopStack, ReusableMessageRecipe, eyre};

#[vihaco_runtime::dialect]
pub mod operations {
    #[instruction]
    pub struct Batch(pub usize);
}

pub trait BatchCount {
    fn count(&self) -> usize;
}

impl BatchCount for operations::Batch {
    fn count(&self) -> usize {
        self.0
    }
}

#[derive(Debug, PartialEq)]
pub struct QubitBatch(pub Vec<u64>);

impl<C, F, I, Context: ?Sized> ReusableMessageRecipe<C, F, I, Context> for QubitBatch
where
    C: HasOperandStack<I>,
    C::Source: PopStack<u64>,
    I: BatchCount,
{
    fn prepare(machine: &mut C, instruction: &I, _: &Context) -> eyre::Result<Self> {
        let count = instruction.count();
        let mut qubits = Vec::new();
        qubits.try_reserve_exact(count)?;
        if count != 0 {
            let source = machine.operand_stack(count);
            for _ in 0..count {
                qubits.push(source.pop()?);
            }
            qubits.reverse();
        }
        Ok(Self(qubits))
    }
}

#[vihaco_runtime::component]
#[instructions { operations }]
pub struct Device;

impl Execute<operations::Batch> for Device {
    type Message = QubitBatch;
    type Effect = Vec<u64>;
    type Error = eyre::Report;

    fn execute(&mut self, _: &operations::Batch, message: QubitBatch) -> eyre::Result<Vec<u64>> {
        Ok(message.0)
    }
}
