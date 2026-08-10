// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::convert::Infallible;

use vihaco::{Execute, NoMessage, StepResult};

struct Target;
#[derive(Clone)]
struct Instruction;
struct Effect;
enum Fault {}

impl Execute<Instruction> for Target {
    type Message = NoMessage;
    type Effect = Effect;
    type Fault = Infallible;

    fn execute(
        &mut self,
        _instruction: &Instruction,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        unreachable!()
    }
}

impl From<Infallible> for Fault {
    fn from(fault: Infallible) -> Self {
        match fault {}
    }
}

vihaco::composite! {
    pub composite Machine {
        error = Fault;
        target: Target,
    }

    runtime {
        Queue(Instruction) => target {
            message none;
        }
    }
}

fn main() {}
