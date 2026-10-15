// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{CPU, StepOutcome, cpu};
use vihaco::{Execute, NoMessage};

impl Execute<cpu::ConstI32> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstI32, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstI64> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstI64, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstU32> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstU32, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstU64> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstU64, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstF32> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstF32, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstF64> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstF64, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstBool> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstBool, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstString> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        instruction: &cpu::ConstString,
        _: NoMessage,
    ) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstFunctionRef> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        instruction: &cpu::ConstFunctionRef,
        _: NoMessage,
    ) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstHeapRef> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        instruction: &cpu::ConstHeapRef,
        _: NoMessage,
    ) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}
