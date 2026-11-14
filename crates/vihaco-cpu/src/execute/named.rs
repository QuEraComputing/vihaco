// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! `wrap` and `unwrap`: named types exist only for verification, so both leave
//! the top word unchanged. They still require an operand, as their stack
//! effect does.

use crate::{CPU, StepOutcome, cpu};
use vihaco_module::host::StackMemory;
use vihaco_runtime::{Execute, NoMessage};

impl Execute<cpu::Wrap> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::Wrap, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        self.stack_top()?;
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::Unwrap> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::Unwrap, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        self.stack_top()?;
        Ok(StepOutcome::Continue)
    }
}
