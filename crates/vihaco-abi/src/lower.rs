// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction phases and capability-based lowering.

/// Retained source operands, before instruction lowering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Syntax;

/// Executable operands, after instruction lowering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Runtime;

/// Lower an instruction using the capabilities supplied by a program loader.
///
/// Instruction authors express required capabilities as bounds on `C` in their
/// implementation. Unchanged dialect instructions receive identity lowering.
pub trait Lower<C: ?Sized>: Sized {
    type Runtime;

    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime>;
}
