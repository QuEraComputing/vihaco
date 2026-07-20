// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser_core::Parse;

use crate::{
    FromBytes,
    traits::{ByteWidth, WriteBytes},
};

/// A value for a `vihaco` component.
///
/// Values must be associated with some [`Type`], connected by both
/// [`Value::Type`] and [`Value::type_of`].
pub trait Value: Sized + ByteWidth + FromBytes + WriteBytes {
    /// The type universe that this value set is contained within.
    type Type: Type;

    fn type_of(&self) -> Self::Type;
}

/// The type universe of a `vihaco` component.
///
/// See [`Value`] for the value set of a `vihaco` component.
pub trait Type: Sized + ByteWidth + FromBytes + WriteBytes {}

/// A marker trait to denote a type is a resolution context to be used
/// during the resolution of a [`crate::syntax::ParsedModule`] into a
/// [`crate::module::LocalModule`].
pub trait ResolutionContext {}

/// The syntactic representation of a value.
///
/// This is used in raw forms during module parsing.
///
/// As the author of a component, you should implement this for its
/// instruction set. The author of the architecture using the component
/// can then have its own [`ResolutionContext`] to describe
/// architecture-specific implementation of dialect requirements.
pub trait SyntacticValue<'src, Ctx: ResolutionContext>: Parse<'src> {
    type RuntimeForm: Value;

    /// Resolve a syntactic value into its runtime form.
    ///
    /// `ctx` provides any functionality or mutable state that is
    /// required for a syntactic value to be resolved. For example,
    /// `ctx` can provide a function for string interning if your
    /// component requires it.
    fn resolve(&self, ctx: &mut Ctx) -> eyre::Result<Self::RuntimeForm>;
}
