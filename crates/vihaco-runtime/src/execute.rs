// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::Component;

/// Mark that a component can execute this instruction.
///
/// Each instruction has its own message, effect, and error type.
/// This is declared by the component that is choosing to execute
/// this instruction.
///
/// The message should be created by the composite and passed into
/// [`Execute::execute`].
///
/// # Example
///
/// ```
/// use vihaco_runtime::{Component, Execute};
/// use std::convert::Infallible;
///
/// // ALU is a stateless component
/// struct ALU;
/// impl Component for ALU {}
///
/// // Add is an instruction that takes no arguments;
/// // useful for a stack machine
/// struct Add;
///
/// struct BinaryOperands<L, R>(L, R);
/// struct ALUResult<T>(T);
///
/// impl Execute<Add> for ALU {
///     type Message = BinaryOperands<u32, u32>;
///     type Effect = ALUResult<u32>;
///     type Error = Infallible;
///
///     fn execute(
///         &mut self,
///         instruction: &Add,
///         message: BinaryOperands<u32, u32>,
///     ) -> Result<ALUResult<u32>, Infallible> {
///         Ok(message.0.wrapping_add(message.1))
///     }
/// }
/// ```
pub trait Execute<I>
where
    Self: Component,
{
    type Message;
    type Effect;
    type Error;

    fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> Result<Self::Effect, Self::Error>;
}
