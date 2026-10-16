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
