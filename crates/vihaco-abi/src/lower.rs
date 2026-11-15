// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction phases, capability-based lowering, and its inverse, raising.

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

/// Raise a runtime instruction back to its source syntax: the inverse of
/// [`Lower`].
///
/// A disassembler raises each loaded instruction with a context that names
/// what lowering resolved: symbols for addresses, functions for indices, and
/// text for string-table indices. Instruction authors express the lookups they
/// need as bounds on `C`, mirroring their `Lower` bounds. A raise only looks
/// names up; it never invents one, and it fails if the runtime form has no
/// source spelling that lowers back to it. Unchanged dialect instructions
/// receive identity raising.
///
/// For every instruction `i` that raising accepts, lowering
/// `i.raise(context)?` in a matching loader yields `i` again.
pub trait Raise<C: ?Sized> {
    type Syntax;

    fn raise(&self, context: &C) -> eyre::Result<Self::Syntax>;
}
