// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Retained syntax operands for CPU lowering.

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

/// A call target address, as `@name` when a function starts there.
impl OperandText<u32> for Target {
    fn write_operand(
        address: &u32,
        names: &dyn SymbolNames,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        names.write_call_target(*address, f)
    }
}

/// A function reference (`const_fn_ref`), as `@name` when the function has one.
impl OperandText<vihaco_abi::word::Word> for Target {
    fn write_operand(
        value: &vihaco_abi::word::Word,
        names: &dyn SymbolNames,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        names.write_function(vihaco_abi::word::decode_function_ref(*value), f)
    }
}

/// A branch target: an executable address, or a label to resolve during
/// loading. It parses like [`Target`], but resolves names as labels only, and
/// its text names a label (never a function, which a jump cannot name).
#[derive(vihaco_parser_derive::Parse, Clone, Debug, PartialEq)]
#[syntax_class(value)]
pub enum JumpTarget {
    #[pattern = "$0"]
    Resolved(u32),
    #[pattern = "`@` $0"]
    Symbol(Ident),
}

/// A jump target address, as `@name` when a label is there.
impl OperandText<u32> for JumpTarget {
    fn write_operand(
        address: &u32,
        names: &dyn SymbolNames,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        names.write_jump_target(*address, f)
    }
}

/// A string-table entry, as the quoted string when the table has it.
impl OperandText<crate::Word> for StringOperand {
    fn write_operand(
        value: &crate::Word,
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
