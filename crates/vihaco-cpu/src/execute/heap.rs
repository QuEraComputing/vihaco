// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::word::{decode_heap_ref, decode_u64, encode_heap_ref, encode_u64};
use crate::{CPU, StepOutcome, Word, cpu};
use vihaco_module::host::StackMemory;
use vihaco_runtime::{Execute, NoMessage};

impl Execute<cpu::ArrayNew> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ArrayNew, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let n = instruction.1 as usize;
        self.require_operands(n)?;
        let start = self.stack.len() - n;
        let values: Vec<Word> = self.stack.drain(start..).collect();
        let heap_id = self.alloc_heap_array(instruction.0.clone(), values);
        self.stack_push(encode_heap_ref(heap_id));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ArrayGet> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ArrayGet, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let index = decode_u64(self.stack_pop()?);
        let heap_id = decode_heap_ref(self.stack_pop()?);
        let value = self.heap_array_get(heap_id, &instruction.0, index)?;
        self.stack_push(value);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ArraySet> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ArraySet, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        self.require_operands(3)?;
        let value = self.stack_pop()?;
        let index = decode_u64(self.stack_pop()?);
        let heap_id = decode_heap_ref(self.stack_pop()?);
        self.heap_array_set(heap_id, &instruction.0, index, value)?;
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::HeapDealloc> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &cpu::HeapDealloc,
        _: NoMessage,
    ) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let id = decode_heap_ref(self.stack_pop()?);
        self.dealloc_heap_object(id)?;
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ArrayReserve> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        instruction: &cpu::ArrayReserve,
        _: NoMessage,
    ) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let capacity = decode_u64(self.stack_pop()?);
        let capacity = usize::try_from(capacity)
            .map_err(|_| eyre::eyre!("array capacity {} does not fit in usize", capacity))?;
        let heap_id = self.reserve_heap_array(instruction.0.clone(), capacity)?;
        self.stack_push(encode_heap_ref(heap_id));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ArrayPush> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ArrayPush, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        self.require_operands(2)?;
        let value = self.stack_pop()?;
        let heap_ref = self.stack_pop()?;
        self.push_heap_array(decode_heap_ref(heap_ref), &instruction.0, value)?;
        self.stack_push(heap_ref);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ArrayLen> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::ArrayLen, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let heap_id = decode_heap_ref(self.stack_pop()?);
        let len = u64::try_from(self.heap_array_len(heap_id)?)?;
        self.stack_push(encode_u64(len));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::StructNew> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::StructNew, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let n = instruction.0.field_count as usize;
        self.require_operands(n)?;
        let start = self.stack.len() - n;
        let values: Vec<Word> = self.stack.drain(start..).collect();
        let heap_id = self.push_heap_struct(instruction.0, values)?;
        self.stack_push(encode_heap_ref(heap_id));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GetField> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::GetField, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let heap_id = decode_heap_ref(self.stack_pop()?);
        let value = self.heap_field(heap_id, instruction.0)?;
        self.stack_push(value);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::SetField> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::SetField, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        self.require_operands(2)?;
        let value = self.stack_pop()?;
        let heap_id = decode_heap_ref(self.stack_pop()?);
        self.set_heap_field(heap_id, instruction.0, value)?;
        Ok(StepOutcome::Continue)
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
    fn struct_new_and_field_access_check_the_struct_type() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_u64(1));
        cpu.stack_push(encode_u64(2));
        cpu.execute(&cpu::StructNew(POINT), NoMessage::new())
            .unwrap();
        let point = *cpu.stack_top().unwrap();

        cpu.stack_push(point);
        cpu.stack_push(encode_u64(7));
        cpu.execute(&cpu::SetField(slot(0, 1)), NoMessage::new())
            .unwrap();
        assert_eq!(cpu.stack(), &[point]);
        cpu.execute(&cpu::GetField(slot(0, 1)), NoMessage::new())
            .unwrap();
        assert_eq!(cpu.stack(), &[encode_u64(7)]);

        cpu.stack_push(point);
        let err = cpu
            .execute(&cpu::GetField(slot(1, 0)), NoMessage::new())
            .unwrap_err();
        assert!(err.to_string().contains("expected struct 1"), "{err}");
        cpu.stack_push(point);
        assert!(cpu.execute(&cpu::ArrayLen, NoMessage::new()).is_err());

        cpu.stack_push(encode_u64(1));
        cpu.stack_push(encode_u64(2));
        cpu.execute(&cpu::ArrayNew(vihaco_abi::Type::U64, 2), NoMessage::new())
            .unwrap();
        let err = cpu
            .execute(&cpu::GetField(slot(0, 0)), NoMessage::new())
            .unwrap_err();
        assert!(
            err.to_string().contains("is array u64, expected struct 0"),
            "{err}"
        );
    }

    #[test]
    fn struct_new_and_set_field_require_operands() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_u64(1));
        assert!(
            cpu.execute(&cpu::StructNew(POINT), NoMessage::new())
                .is_err()
        );
        assert_eq!(cpu.stack(), &[encode_u64(1)]);
        assert!(
            cpu.execute(&cpu::SetField(slot(0, 0)), NoMessage::new())
                .is_err()
        );
    }

    #[test]
    fn heap_reserve_rejects_unrepresentable_allocation_without_allocating() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_u64(u64::MAX));
        assert!(
            cpu.execute(&cpu::ArrayReserve(vihaco_abi::Type::U64), NoMessage::new())
                .is_err()
        );
        assert!(cpu.heap_is_empty());
    }
}
