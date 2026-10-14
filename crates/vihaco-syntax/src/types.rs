// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Parsed-syntax data shapes. See module docs in [`super`].

use vihaco_parser::Ident;

/// Parsed SST module containing consumer instruction values.
///
/// `I` is the consumer's instruction type (runtime instructions in the typed
/// declaration mode), `Ty` is its source type
/// syntax, and `H` is its section-header type.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedModule<I, Ty, H> {
    pub header: H,
    pub functions: Vec<ParsedFunction<I, Ty>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFunction<I, Ty> {
    /// Function name with the leading `@` stripped (`@main` → `"main"`).
    pub name: Ident,
    /// Function parameters parsed from `name: type` declarations.
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
