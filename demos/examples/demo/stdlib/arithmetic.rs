// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::stack::{Stack, StackFault};

use super::{
    Effects,
    execute::{Execute, Execution, StepResult},
    handle::Absorb,
    supply::Supply,
};

vihaco::component! {
    component ArithmeticUnit {}

    instruction {
        #[derive(Debug, Clone, Copy)]
        Add,
        #[derive(Debug, Clone, Copy)]
        Sub,
        #[derive(Debug, Clone, Copy)]
        Mul
    }
}

pub use arithmetic_unit::ArithmeticUnit;
pub use arithmetic_unit::instruction::{Add, Mul, Sub};

impl ArithmeticUnit {
    pub fn new() -> Self {
        Self {}
    }
}

/// The message the composite resolves for an arithmetic op (its two operands).
pub struct BinaryOperands {
    lhs: i64,
    rhs: i64,
}

/// The semantic effect arithmetic produces. It carries a value and names no destination, which is
/// why several routes can share it.
#[derive(Debug)]
pub struct ValueResult(i64);

impl Execute<Add> for ArithmeticUnit {
    type Message = BinaryOperands;
    type Effect = ValueResult;
    type Fault = std::convert::Infallible;

    fn execute(
        &mut self,
        _instruction: &Add,
        message: BinaryOperands,
    ) -> Result<StepResult<ValueResult>, Self::Fault> {
        Ok(StepResult {
            effects: Effects::one(ValueResult(message.lhs.wrapping_add(message.rhs))),
            execution: Execution::Complete,
        })
    }
}

impl Execute<Sub> for ArithmeticUnit {
    type Message = BinaryOperands;
    type Effect = ValueResult;
    type Fault = std::convert::Infallible;

    fn execute(
        &mut self,
        _instruction: &Sub,
        message: BinaryOperands,
    ) -> Result<StepResult<ValueResult>, Self::Fault> {
        Ok(StepResult {
            effects: Effects::one(ValueResult(message.lhs.wrapping_sub(message.rhs))),
            execution: Execution::Complete,
        })
    }
}

impl Execute<Mul> for ArithmeticUnit {
    type Message = BinaryOperands;
    type Effect = ValueResult;
    type Fault = std::convert::Infallible;

    fn execute(
        &mut self,
        _instruction: &Mul,
        message: BinaryOperands,
    ) -> Result<StepResult<ValueResult>, Self::Fault> {
        Ok(StepResult {
            effects: Effects::one(ValueResult(message.lhs.wrapping_mul(message.rhs))),
            execution: Execution::Complete,
        })
    }
}

/// How a stack swallows an arithmetic result, written once and reused by every `effects to <stack>`
/// route. The macro never synthesizes handler behavior; it names a field.
impl Absorb<ValueResult> for Stack {
    type Fault = StackFault;

    fn absorb(&mut self, effect: ValueResult) -> Result<(), StackFault> {
        self.push(effect.0);
        Ok(())
    }
}

/// How a stack yields a pair of operands, written once and reused by every `message from <stack>`
/// route. Pops rhs then lhs to preserve stack order.
impl Supply<BinaryOperands> for Stack {
    type Fault = StackFault;

    fn supply(&mut self) -> Result<BinaryOperands, StackFault> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        Ok(BinaryOperands { lhs, rhs })
    }
}
