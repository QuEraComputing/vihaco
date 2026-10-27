// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{CPU, cpu};
use vihaco_runtime::{Execute, PopFromStack, PushToStack};

impl Execute<cpu::Not> for CPU {
    type Message = PopFromStack<(bool,)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::Not,
        PopFromStack((operand,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = !operand;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::And> for CPU {
    type Message = PopFromStack<(bool, bool)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::And,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs && rhs,)))
    }
}

impl Execute<cpu::Or> for CPU {
    type Message = PopFromStack<(bool, bool)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::Or,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs || rhs,)))
    }
}

impl Execute<cpu::Xor> for CPU {
    type Message = PopFromStack<(bool, bool)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::Xor,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs ^ rhs,)))
    }
}
