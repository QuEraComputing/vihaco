// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{CPU, Word, cpu};
use vihaco_runtime::{Execute, InputCount, PopFromStack, PushToStack, StackBatch, StackInputCount};

impl StackInputCount<cpu::HeapAlloc> for CPU {
    fn input_count(instruction: &cpu::HeapAlloc) -> eyre::Result<InputCount> {
        Ok(InputCount::Immediate(usize::try_from(instruction.0)?))
    }
}

impl Execute<cpu::HeapAlloc> for CPU {
    type Message = PopFromStack<StackBatch<Word>>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        _: &cpu::HeapAlloc,
        PopFromStack(values): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        // The heap object outlives this message and owns a separate allocation.
        Ok(PushToStack((self
            .heap
            .alloc(values.into_iter().collect::<Vec<_>>()),)))
    }
}

impl Execute<cpu::GetItem> for CPU {
    type Message = PopFromStack<(u32, u64)>;
    type Effect = PushToStack<(Word,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        _: &cpu::GetItem,
        PopFromStack((id, index)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let index = usize::try_from(index)?;
        let value = *self
            .heap
            .array(id)?
            .get(index)
            .ok_or_else(|| eyre::eyre!("heap index {} out of bounds", index))?;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::HeapDealloc> for CPU {
    type Message = PopFromStack<(u32,)>;
    type Effect = PushToStack<()>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        _: &cpu::HeapDealloc,
        PopFromStack((id,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        self.heap.dealloc(id)?;
        Ok(PushToStack(()))
    }
}

impl Execute<cpu::HeapReserve> for CPU {
    type Message = PopFromStack<(u64,)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        _: &cpu::HeapReserve,
        PopFromStack((capacity,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((self
            .heap
            .reserve(usize::try_from(capacity)?)?,)))
    }
}

impl Execute<cpu::HeapPush> for CPU {
    type Message = PopFromStack<(Word, Word)>;
    type Effect = PushToStack<(Word,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        _: &cpu::HeapPush,
        PopFromStack((id, value)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        self.heap.push(crate::word::decode_heap_ref(id), value)?;
        Ok(PushToStack((id,)))
    }
}

impl Execute<cpu::HeapLen> for CPU {
    type Message = PopFromStack<(u32,)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        _: &cpu::HeapLen,
        PopFromStack((id,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((u64::try_from(self.heap.array(id)?.len())?,)))
    }
}

impl StackInputCount<cpu::StructNew> for CPU {
    fn input_count(instruction: &cpu::StructNew) -> eyre::Result<InputCount> {
        Ok(InputCount::Immediate(usize::try_from(
            instruction.0.field_count,
        )?))
    }
}

impl Execute<cpu::StructNew> for CPU {
    type Message = PopFromStack<StackBatch<Word>>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        instruction: &cpu::StructNew,
        PopFromStack(values): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let values: Vec<Word> = values.into_iter().collect();
        Ok(PushToStack((self
            .heap
            .alloc_struct(instruction.0, values)?,)))
    }
}

impl Execute<cpu::GetField> for CPU {
    type Message = PopFromStack<(u32,)>;
    type Effect = PushToStack<(Word,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        instruction: &cpu::GetField,
        PopFromStack((id,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((self.heap.field(id, instruction.0)?,)))
    }
}

impl Execute<cpu::SetField> for CPU {
    type Message = PopFromStack<(u32, Word)>;
    type Effect = PushToStack<()>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        instruction: &cpu::SetField,
        PopFromStack((id, value)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        self.heap.set_field(id, instruction.0, value)?;
        Ok(PushToStack(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FieldSlot, StructLayout};

    const POINT: StructLayout = StructLayout {
        struct_id: 0,
        field_count: 2,
    };

    fn slot(struct_id: u32, slot: u32) -> FieldSlot {
        FieldSlot { struct_id, slot }
    }

    #[test]
    fn struct_fields_are_checked_against_the_object_struct_type() {
        let mut cpu = CPU::default();
        let id = cpu.heap.alloc_struct(POINT, vec![1, 2]).unwrap();
        cpu.execute(&cpu::SetField(slot(0, 1)), PopFromStack((id, 7)))
            .unwrap();
        let PushToStack((value,)) = cpu
            .execute(&cpu::GetField(slot(0, 1)), PopFromStack((id,)))
            .unwrap();
        assert_eq!(value, 7);
        assert!(
            cpu.execute(&cpu::GetField(slot(1, 0)), PopFromStack((id,)))
                .is_err()
        );
        assert!(cpu.execute(&cpu::HeapLen, PopFromStack((id,))).is_err());
        let array = cpu.heap.alloc(vec![1, 2]);
        assert!(
            cpu.execute(&cpu::GetField(slot(0, 0)), PopFromStack((array,)))
                .is_err()
        );
    }
}
