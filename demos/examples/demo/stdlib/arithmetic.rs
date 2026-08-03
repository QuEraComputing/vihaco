// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// A reusable, stateless `i64` arithmetic component implementing `add`, `sub`, and `mul`. It is pure
/// behavior: it does not know which CPU contains it, which stack supplied the values, or how long an
/// operation takes. Because it carries no timing, the same component works unchanged whether
/// execution is driven by a clock or by real time.
struct ArithmeticUnit;

impl ArithmeticUnit {
    fn new() -> Self {
        Self
    }
}

/*
component! {
    component ArithmeticUnit;

    #[namespace("arith")]
    instruction Arithmetic {
        #[pattern = "'add"]
        Add,
        #[pattern = "'sub"]
        Sub,
        #[pattern = "'mul"]
        Mul,
    }
}
*/

enum Arithmetic {
    Add(Add),
    Sub(Sub),
    Mul(Mul),
}

/// The three arithmetic runtime instructions. Each is a distinct payload type so `Execute` can
/// select the operation, while the surrounding route ZST selects where the result lands.
#[derive(Debug, Clone, Copy)]
struct Add;
#[derive(Debug, Clone, Copy)]
struct Sub;
#[derive(Debug, Clone, Copy)]
struct Mul;

/// The message the composite resolves for an arithmetic op (its two operands).
struct BinaryOperands {
    lhs: i64,
    rhs: i64,
}

/// The semantic effect arithmetic produces. It carries a value and names no destination, which is
/// why several routes can share it.
#[derive(Debug)]
struct ValueResult(i64);

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
