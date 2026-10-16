// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{CPU, StepOutcome, cpu};
use vihaco::Execute;

impl Execute<cpu::ConstI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstBool> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstBool, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstString> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstString, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstFunctionRef> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstFunctionRef, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConstHeapRef> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::ConstHeapRef, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let v = instruction.0;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}
