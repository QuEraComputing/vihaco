// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Parsed-syntax data shapes. See module docs in [`super`].

use vihaco_bytecode::SstHeader;
use vihaco_parser::{Ident, SurfaceInstruction};

/// The complete source dialect for one SST module.
pub trait ModuleSyntax {
    /// Surface instruction syntax accepted by this module.
    type Instruction: SurfaceInstruction + std::fmt::Debug + Clone + PartialEq;
    /// Source value syntax accepted by this module.
    type Value;
    /// Source type syntax accepted by this module.
    type Type: std::fmt::Debug + Clone + PartialEq;
    /// Parsed source syntax for this module's section header.
    type Header: SstHeader + std::fmt::Debug + Clone + PartialEq;
}

/// Parsed `.sst` module before resolution.
///
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedModule<S>
where
    S: ModuleSyntax,
{
    /// The parsed source header. This is distinct from installed runtime metadata.
    pub header: S::Header,
    pub functions: Vec<ParsedFunction<S>>,
    pub labels: Vec<ParsedLabel>,
    pub constants: Vec<vihaco_abi::Value>,
    pub strings: Vec<String>,
    pub source_symbols: Vec<ParsedSourceSymbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLabel {
    pub name: Ident,
    pub function: Ident,
    pub instruction: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSourceSymbol {
    pub name: Ident,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFunction<S>
where
    S: ModuleSyntax,
{
    /// Function name with the leading `@` stripped (`@main` → `"main"`).
    pub name: Ident,
    /// Empty for the moment — `.sst` examples don't exercise parameters.
    /// Non-empty parameter syntax errors during parsing.
    pub params: Vec<Param<S>>,
    /// Return type parsed with the module's source type syntax.
    pub return_ty: Option<S::Type>,
    pub body: Vec<S::Instruction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param<S>
where
    S: ModuleSyntax,
{
    pub name: Ident,
    pub ty: S::Type,
}
