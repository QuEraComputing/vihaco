// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Parsed-syntax data shapes. See module docs in [`super`].

use vihaco_parser::{Ident, SurfaceInstruction};

/// Parsed `.sst` module before resolution.
///
/// `I` is the consumer's surface instruction type, `Ty` is its source type
/// syntax, and `H` is its section-header type.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedModule<I, Ty, H>
where
    I: SurfaceInstruction,
{
    pub header: H,
    pub functions: Vec<ParsedFunction<I, Ty>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFunction<I, Ty>
where
    I: SurfaceInstruction,
{
    /// Function name with the leading `@` stripped (`@main` → `"main"`).
    pub name: Ident,
    /// Empty for the moment — `.sst` examples don't exercise parameters.
    /// Non-empty parameter syntax errors during parsing.
    pub params: Vec<Param<Ty>>,
    /// Return type parsed with the consumer-provided `Ty` syntax.
    pub return_ty: Option<Ty>,
    pub body: Vec<I>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param<Ty> {
    pub name: Ident,
    pub ty: Ty,
}
