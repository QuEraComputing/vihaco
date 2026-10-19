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
            .get(id)?
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
        Ok(PushToStack((u64::try_from(self.heap.get(id)?.len())?,)))
    }
}
