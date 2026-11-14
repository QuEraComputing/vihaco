// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! `wrap` and `unwrap`: named types exist only for verification, so both pass
//! the top word through unchanged.

use crate::{CPU, Word, cpu};
use vihaco_runtime::{Execute, PopFromStack, PushToStack};

impl Execute<cpu::Wrap> for CPU {
    type Message = PopFromStack<(Word,)>;
    type Effect = PushToStack<(Word,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::Wrap,
        PopFromStack((word,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((word,)))
    }
}

impl Execute<cpu::Unwrap> for CPU {
    type Message = PopFromStack<(Word,)>;
    type Effect = PushToStack<(Word,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::Unwrap,
        PopFromStack((word,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((word,)))
    }
}
