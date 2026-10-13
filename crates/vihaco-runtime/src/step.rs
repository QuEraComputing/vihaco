// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{Composite, Execute, Field};

/// Execution behavior for one composite field route and instruction type.
///
/// Available with the experimental `typed-declarations` feature. Use
/// [`crate::field!`] to select a direct field or a nested route. The final child
/// must implement [`Execute<I>`], while the step implementation supplies the
/// composite's behavior, including runtime input and effect handling.
pub trait Step<F, I>
where
    Self: Composite + Field<F>,
    <Self as Field<F>>::Child: Execute<I>,
{
    /// Perform a step for the selected destination and instruction.
    fn step(&mut self, instruction: I);
}
