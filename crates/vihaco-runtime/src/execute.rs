// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::Component;

/// Execute one instruction type with runtime-supplied input.
///
/// Available with the experimental `typed-declarations` feature. Each impl
/// chooses its message, result, and error types independently.
/// Implementing this trait alone does not declare dialect availability.
///
/// # Examples
///
/// ```
/// use vihaco_runtime::{Component, Effects, Execute, NoMessage};
///
/// struct Add(i64);
/// struct Counter(i64);
/// impl Component for Counter {}
/// impl Execute<Add> for Counter {
///     type Message = NoMessage;
///     type Effect = Effects<i64>;
///     type Error = eyre::Report;
///
///     fn execute(&mut self, instruction: &Add, _: NoMessage) -> eyre::Result<Self::Effect> {
///         self.0 += instruction.0;
///         Ok(Effects::one(self.0))
///     }
/// }
///
/// let mut counter = Counter(1);
/// assert_eq!(counter.execute(&Add(2), NoMessage::new()).unwrap(), Effects::one(3));
/// ```
pub trait Execute<I>: Component {
    /// Input supplied by the runtime, rather than encoded in the instruction.
    type Message;
    /// The successful result.
    type Effect;
    /// The component's execution error, usually `eyre::Report`.
    type Error;

    /// Execute a borrowed instruction without consuming its payload.
    ///
    /// # Errors
    ///
    /// Returns the error chosen by this implementation if execution fails.
    fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> Result<Self::Effect, Self::Error>;
}

/// Execute one instruction asynchronously with runtime-supplied input.
///
/// The returned future may borrow the component and instruction. A driver
/// that stores the future must arrange for it to own its inputs, for example
/// by moving the component, instruction, and message into an outer future.
pub trait AsyncExecute<I>: Component {
    /// Input supplied by the runtime.
    type Message;
    /// The successful result.
    type Effect;
    /// The component's execution error.
    type Error;

    /// Start execution of a borrowed instruction.
    ///
    /// # Errors
    ///
    /// The future resolves to the implementation's error if execution fails.
    fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> impl Future<Output = Result<Self::Effect, Self::Error>>;
}

/// Evidence that a component declares execution support for instruction `I`.
///
/// [`crate::component`] checks each leaf of its declared dialects for
/// [`Execute`], [`AsyncExecute`], or both. This bound follows from that checked
/// dialect membership. Use the corresponding execution trait to call a method.
pub trait ExecutionCapability<I>: Component {}

impl<C, I> ExecutionCapability<I> for C
where
    I: crate::__private::InstructionSpec,
    C: crate::__private::Supports<I::Dialect>,
{
}
