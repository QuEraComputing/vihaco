// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{CPU, Word, cpu};
use vihaco_runtime::{Execute, InputCount, PopFromStack, PushToStack, StackBatch, StackInputCount};

impl StackInputCount<cpu::ArrayNew> for CPU {
    fn input_count(instruction: &cpu::ArrayNew) -> eyre::Result<InputCount> {
        Ok(InputCount::Immediate(usize::try_from(instruction.1)?))
    }
}

impl Execute<cpu::ArrayNew> for CPU {
    type Message = PopFromStack<StackBatch<Word>>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        instruction: &cpu::ArrayNew,
        PopFromStack(values): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        // The heap object outlives this message and owns a separate allocation.
        let values: Vec<Word> = values.into_iter().collect();
        Ok(PushToStack((self
            .heap
            .alloc_array(instruction.0.clone(), values),)))
    }
}

impl Execute<cpu::ArrayGet> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(Word,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        instruction: &cpu::ArrayGet,
        PopFromStack((id, index)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((self.heap.get(id, &instruction.0, index)?,)))
    }
}

impl Execute<cpu::ArraySet> for CPU {
    type Message = PopFromStack<(u64, u64, Word)>;
    type Effect = PushToStack<()>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        instruction: &cpu::ArraySet,
        PopFromStack((id, index, value)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        self.heap.set(id, &instruction.0, index, value)?;
        Ok(PushToStack(()))
    }
}

impl Execute<cpu::HeapDealloc> for CPU {
    type Message = PopFromStack<(u64,)>;
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

impl Execute<cpu::ArrayReserve> for CPU {
    type Message = PopFromStack<(u64,)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        instruction: &cpu::ArrayReserve,
        PopFromStack((capacity,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let capacity = usize::try_from(capacity)?;
        Ok(PushToStack((self
            .heap
            .reserve_array(instruction.0.clone(), capacity)?,)))
    }
}

impl Execute<cpu::ArrayPush> for CPU {
    type Message = PopFromStack<(Word, Word)>;
    type Effect = PushToStack<(Word,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        instruction: &cpu::ArrayPush,
        PopFromStack((id, value)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        self.heap
            .push(crate::word::decode_heap_ref(id), &instruction.0, value)?;
        Ok(PushToStack((id,)))
    }
}

impl Execute<cpu::ArrayLen> for CPU {
    type Message = PopFromStack<(u64,)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        _: &cpu::ArrayLen,
        PopFromStack((id,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((u64::try_from(self.heap.len(id)?)?,)))
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
    type Effect = PushToStack<(u64,)>;
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
    type Message = PopFromStack<(u64,)>;
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
    type Message = PopFromStack<(u64, Word)>;
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
        assert!(cpu.execute(&cpu::ArrayLen, PopFromStack((id,))).is_err());
        let array = cpu.heap.alloc_array(vihaco_abi::Type::U64, vec![1, 2]);
        assert!(
            cpu.execute(&cpu::GetField(slot(0, 0)), PopFromStack((array,)))
                .is_err()
        );
    }

    #[test]
    fn array_elements_are_checked_against_the_object_element_type() {
        use vihaco_abi::Type;
        let mut cpu = CPU::default();
        let id = cpu.heap.alloc_array(Type::F64, vec![1, 2]);
        cpu.execute(&cpu::ArraySet(Type::F64), PopFromStack((id, 1, 9)))
            .unwrap();
        let PushToStack((value,)) = cpu
            .execute(&cpu::ArrayGet(Type::F64), PopFromStack((id, 1)))
            .unwrap();
        assert_eq!(value, 9);
        let PushToStack((len,)) = cpu.execute(&cpu::ArrayLen, PopFromStack((id,))).unwrap();
        assert_eq!(len, 2);
        let err = cpu
            .execute(&cpu::ArrayGet(Type::U64), PopFromStack((id, 0)))
            .unwrap_err();
        assert!(
            err.to_string().contains("is array f64, expected array u64"),
            "{err}"
        );
        assert!(
            cpu.execute(&cpu::ArraySet(Type::F64), PopFromStack((id, 2, 0)))
                .is_err()
        );
        assert!(
            cpu.execute(&cpu::ArrayPush(Type::F64), PopFromStack((id, 0)))
                .is_err(),
            "full arrays reject appends"
        );
    }
}
