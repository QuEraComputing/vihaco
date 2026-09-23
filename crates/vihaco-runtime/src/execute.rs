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
