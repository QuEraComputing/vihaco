// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Parsed-syntax data shapes. See module docs in [`super`].

pub trait SurfaceInstruction {}

#[derive(vihaco_parser::Parse)]
#[syntax_class(value)]
pub struct SurfaceValue {
    pub value: String,
}

#[derive(Debug, Clone, Eq, PartialEq, vihaco_parser::Parse)]
#[syntax_class(type)]
#[pattern = "$ty"]
pub struct SurfaceType {
    pub ty: String,
}

/// Parsed `.sst` module — pre-resolution. `H` is the consumer's device-header
/// enum (typically derives `Parse` via Item 5's `DeviceHeader`).
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedModule<I, H>
where
    I: SurfaceInstruction,
{
    pub header: H,
    pub functions: Vec<ParsedFunction<I>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFunction<I>
where
    I: SurfaceInstruction,
{
    /// Function name with the leading `@` stripped (`@main` → `"main"`).
    pub name: String,
    /// Empty for the moment — `.sst` examples don't exercise parameters.
    /// Non-empty parameter syntax errors during parsing.
    pub params: Vec<Param>,
    /// Return type as a bare token (`i64`, `f64`, …). Resolver converts to
    /// `vihaco::Type`.
    pub return_ty: Option<SurfaceType>,
    pub body: Vec<I>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: SurfaceType,
}
