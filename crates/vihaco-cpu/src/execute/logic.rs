// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::data::component::canonical_bool;
use crate::word::encode_bool;
use crate::{CPU, StepOutcome, cpu};
use vihaco_module::host::StackMemory;
use vihaco_runtime::{Execute, NoMessage};

impl Execute<cpu::Not> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::Not, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let value = !canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::And> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::And, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = canonical_bool(self.stack_pop()?)?;
        let lhs = canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(lhs && rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::Or> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::Or, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = canonical_bool(self.stack_pop()?)?;
        let lhs = canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(lhs || rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::Xor> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::Xor, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = canonical_bool(self.stack_pop()?)?;
        let lhs = canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(lhs ^ rhs));
        Ok(StepOutcome::Continue)
    }
}
