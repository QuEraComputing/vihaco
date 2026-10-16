// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::word::{decode_heap_ref, decode_u64, encode_heap_ref, encode_u64};
use crate::{CPU, StepOutcome, Word, cpu};
use vihaco::Execute;
use vihaco::traits::StackMemory;

impl Execute<cpu::HeapAlloc> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::HeapAlloc, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let n_elements = instruction.0;
        let n: usize = n_elements as usize;
        self.require_operands(n)?;
        let start = self.stack.len() - n;
        let values: Box<[Word]> = self.stack.drain(start..).collect();
        let heap_id = self.push_heap_object(values);
        self.stack_push(encode_heap_ref(heap_id));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GetItem> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GetItem, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let index = Self::heap_index(self.stack_pop()?)?;
        let heap_id = decode_heap_ref(self.stack_pop()?);
        let value = *self
            .heap_object(heap_id)?
            .get(index)
            .ok_or_else(|| eyre::eyre!("heap index {} out of bounds", index))?;
        self.stack_push(value);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::HeapDealloc> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::HeapDealloc, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let id = decode_heap_ref(self.stack_pop()?);
        self.dealloc_heap_object(id)?;
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::HeapReserve> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::HeapReserve, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let capacity = decode_u64(self.stack_pop()?);
        let capacity = usize::try_from(capacity)
            .map_err(|_| eyre::eyre!("heap capacity {} does not fit in usize", capacity))?;
        let heap_id = self.reserve_heap_object(capacity)?;
        self.stack_push(encode_heap_ref(heap_id));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::HeapPush> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::HeapPush, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        self.require_operands(2)?;
        let value = self.stack_pop()?;
        let heap_ref = self.stack_pop()?;
        self.append_heap_object(decode_heap_ref(heap_ref), value)?;
        self.stack_push(heap_ref);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::HeapLen> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::HeapLen, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let heap_id = decode_heap_ref(self.stack_pop()?);
        let len = u64::try_from(self.heap_object(heap_id)?.len())?;
        self.stack_push(encode_u64(len));
        Ok(StepOutcome::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heap_reserve_rejects_unrepresentable_allocation_without_allocating() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_u64(u64::MAX));
        assert!(cpu.execute(&cpu::HeapReserve, ()).is_err());
        assert!(cpu.heap_is_empty());
    }
}
