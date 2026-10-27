// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// Access to the composite's execution-state device, supplied by `#[program]`.
/// This selects mutable invocation state; the runner borrows program data separately.
/// Instruction selection and PC management remain internal to stdlib execution.
pub trait HasProgram {
    type Program;
    fn program(&self) -> &Self::Program;
    fn program_mut(&mut self) -> &mut Self::Program;
}
