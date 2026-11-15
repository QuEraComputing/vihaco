// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::Composite;

/// Dispatch a composite instruction to its destination's [`crate::Step`].
///
/// Available with the experimental `typed-declarations` feature. Implementations
/// select a field route and leaf instruction type from the aggregate instruction
/// and forward the shared context to its authored Step. `Context` defaults to `()`.
pub trait Dispatch<Context: ?Sized = ()>: Composite {
    /// The aggregate instruction type accepted by this composite.
    type Instruction;

    /// Select and run the step, borrowing the instruction without consuming it.
    ///
    /// # Errors
    ///
    /// Returns the selected step's error unchanged. State changes and effects
    /// already applied by the step are not rolled back; recovery or termination
    /// is the caller's responsibility.
    fn dispatch(&mut self, instruction: &Self::Instruction, context: &Context) -> eyre::Result<()>;

    /// `instruction` in source syntax with operands named through `names`,
    /// for runtime errors that locate a failing instruction.
    ///
    /// Composites implement this from their instructions'
    /// [`InstructionText`](crate::InstructionText). The default has no text,
    /// so a hand-written machine's errors show the location alone.
    fn instruction_text(
        instruction: &Self::Instruction,
        names: &dyn crate::SymbolNames,
    ) -> Option<String> {
        let _ = (instruction, names);
        None
    }
}
