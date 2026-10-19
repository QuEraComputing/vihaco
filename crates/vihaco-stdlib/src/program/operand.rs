// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Retained syntax operands for program lowering.

use vihaco_parser::{Ident, QuotedString};

/// An executable address or function index, or a symbol to resolve during loading.
#[derive(vihaco_parser_derive::Parse, Clone, Debug, PartialEq)]
#[syntax_class(value)]
pub enum Target {
    #[pattern = "$0"]
    Resolved(u32),
    #[pattern = "`@` $0"]
    Symbol(Ident),
}

/// An existing string-table index or a string literal to intern during loading.
#[derive(vihaco_parser_derive::Parse, Clone, Debug, PartialEq)]
#[syntax_class(value)]
pub enum StringOperand {
    #[pattern = "$0"]
    Literal(QuotedString),
    #[pattern = "$0"]
    Resolved(u32),
}
