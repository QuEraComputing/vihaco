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
    ///
    /// # Errors
    ///
    /// Returns failures from preparing runtime input, executing the instruction,
    /// or handling its effects. Implementations can use `?` to convert compatible
    /// execution errors into [`eyre::Report`], including `Infallible`, or to
    /// propagate existing reports unchanged. Returning an error does not roll
    /// back state changes or effects already applied.
    fn step(&mut self, instruction: I) -> eyre::Result<()>;
}
