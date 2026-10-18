// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{CPU, StepOutcome, Word, cpu};
use vihaco_module::host::{FrameMemory, StackMemory};
use vihaco_runtime::{Execute, NoMessage};

impl Execute<cpu::LoadI32> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::LoadI32, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        // addr should be local to frame.
        let value = self.get_local(addr as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LoadI64> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::LoadI64, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        // addr should be local to frame.
        let value = self.get_local(addr as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LoadU32> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::LoadU32, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        // addr should be local to frame.
        let value = self.get_local(addr as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LoadU64> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::LoadU64, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        // addr should be local to frame.
        let value = self.get_local(addr as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LoadF32> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::LoadF32, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        // addr should be local to frame.
        let value = self.get_local(addr as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LoadF64> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::LoadF64, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        // addr should be local to frame.
        let value = self.get_local(addr as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LoadBool> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::LoadBool, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        // addr should be local to frame.
        let value = self.get_local(addr as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::StoreI32> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::StoreI32, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        let address = self.local_address(addr as usize)?;
        let value: Word = self.stack_pop()?;
        *self
            .stack
            .get_mut(address)
            .ok_or_else(|| eyre::eyre!("local index out of bounds"))? = value;
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::StoreI64> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::StoreI64, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        let address = self.local_address(addr as usize)?;
        let value: Word = self.stack_pop()?;
        *self
            .stack
            .get_mut(address)
            .ok_or_else(|| eyre::eyre!("local index out of bounds"))? = value;
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::StoreU32> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::StoreU32, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        let address = self.local_address(addr as usize)?;
        let value: Word = self.stack_pop()?;
        *self
            .stack
            .get_mut(address)
            .ok_or_else(|| eyre::eyre!("local index out of bounds"))? = value;
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::StoreU64> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::StoreU64, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        let address = self.local_address(addr as usize)?;
        let value: Word = self.stack_pop()?;
        *self
            .stack
            .get_mut(address)
            .ok_or_else(|| eyre::eyre!("local index out of bounds"))? = value;
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::StoreF32> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::StoreF32, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        let address = self.local_address(addr as usize)?;
        let value: Word = self.stack_pop()?;
        *self
            .stack
            .get_mut(address)
            .ok_or_else(|| eyre::eyre!("local index out of bounds"))? = value;
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::StoreF64> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::StoreF64, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        let address = self.local_address(addr as usize)?;
        let value: Word = self.stack_pop()?;
        *self
            .stack
            .get_mut(address)
            .ok_or_else(|| eyre::eyre!("local index out of bounds"))? = value;
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::StoreBool> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::StoreBool, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let addr = instruction.0;
        let address = self.local_address(addr as usize)?;
        let value: Word = self.stack_pop()?;
        *self
            .stack
            .get_mut(address)
            .ok_or_else(|| eyre::eyre!("local index out of bounds"))? = value;
        Ok(StepOutcome::Continue)
    }
}
