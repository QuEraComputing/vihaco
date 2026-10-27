// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{Execute, NoMessage, component, dialect};

#[dialect]
pub mod counter {
    #[instruction]
    pub struct Add(pub u64);
}

#[component]
#[instructions { counter }]
#[derive(Default)]
pub struct Counter(u64);

impl Execute<counter::Add> for Counter {
    type Message = NoMessage;
    type Effect = u64;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &counter::Add, _: NoMessage) -> eyre::Result<u64> {
        self.0 = self
            .0
            .checked_add(instruction.0)
            .ok_or_else(|| eyre::eyre!("counter overflow"))?;
        Ok(self.0)
    }
}
