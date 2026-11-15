// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Retained syntax operands for program lowering.

use std::fmt;

use vihaco_abi::names::{OperandText, SymbolNames};
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

/// A jump or call target, as `@name` when a function or label starts there.
impl OperandText<u32> for Target {
    fn write_operand(
        address: &u32,
        names: &dyn SymbolNames,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        names.write_target(*address, f)
    }
}

/// A string-table entry, as the quoted string when the table has it.
impl OperandText<vihaco_abi::word::Word> for StringOperand {
    fn write_operand(
        value: &vihaco_abi::word::Word,
        names: &dyn SymbolNames,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let index = vihaco_abi::word::decode_string_id(*value);
        match names.string(index) {
            Some(text) => write!(f, "{text:?}"),
            None => write!(f, "{index}"),
        }
    }
}
