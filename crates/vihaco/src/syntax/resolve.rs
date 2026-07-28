// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The `Resolve` trait — bridge between [`super::ParsedModule`] and the
//! runtime [`crate::module::Module`]. Each consumer implements this for its
//! own instruction set + header type, holding whatever state is needed
//! (label table, string interner, sugar expansion rules).

use crate::syntax::{ParsedModule, types::SurfaceInstruction};

/// Lower a parsed module to its resolved runtime form.
///
/// Implementations own application-specific conversion such as translating
/// typed surface variants, expanding explicitly modeled sugar, or interning
/// parsed values.
pub trait Resolve<I, H>
where
    I: SurfaceInstruction,
{
    /// Resolved module type — concrete to the consumer (typically
    /// `crate::module::Module<I, Value, Type, Info>` with consumer-specific
    /// `Info`).
    type Module;

    fn resolve_module(&mut self, parsed: ParsedModule<I, H>) -> eyre::Result<Self::Module>;
}
