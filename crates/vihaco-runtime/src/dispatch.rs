// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::Composite;

/// Dispatch a composite instruction to its destination's [`crate::Step`].
///
/// Available with the experimental `typed-declarations` feature. Implementations
/// select a field route and leaf instruction type from the aggregate instruction.
pub trait Dispatch: Composite {
    /// The aggregate instruction type accepted by this composite.
    type Instruction;

    /// Select and run the step for an instruction.
    fn dispatch(&mut self, instruction: Self::Instruction);
}
