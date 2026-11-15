// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Literal parser values converted into encoded runtime words.

use std::fmt;

use crate::Word;
use vihaco_abi::names::{OperandText, SymbolNames};

#[derive(vihaco_parser_derive::Parse, Clone, Debug, PartialEq)]
#[syntax_class(value)]
pub struct I32Literal(i32);

impl From<I32Literal> for Word {
    fn from(value: I32Literal) -> Self {
        crate::word::encode_i32(value.0)
    }
}

#[derive(vihaco_parser_derive::Parse, Clone, Debug, PartialEq)]
#[syntax_class(value)]
pub struct I64Literal(i64);

impl From<I64Literal> for Word {
    fn from(value: I64Literal) -> Self {
        crate::word::encode_i64(value.0)
    }
}

#[derive(vihaco_parser_derive::Parse, Clone, Debug, PartialEq)]
#[syntax_class(value)]
pub struct F32Literal(f32);

impl From<F32Literal> for Word {
    fn from(value: F32Literal) -> Self {
        crate::word::encode_f32(value.0)
    }
}

#[derive(vihaco_parser_derive::Parse, Clone, Debug, PartialEq)]
#[syntax_class(value)]
pub struct F64Literal(f64);

impl From<F64Literal> for Word {
    fn from(value: F64Literal) -> Self {
        crate::word::encode_f64(value.0)
    }
}

impl OperandText<Word> for I32Literal {
    fn write_operand(value: &Word, _: &dyn SymbolNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", vihaco_abi::word::decode_i32(*value))
    }
}

impl OperandText<Word> for I64Literal {
    fn write_operand(value: &Word, _: &dyn SymbolNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", vihaco_abi::word::decode_i64(*value))
    }
}

impl OperandText<Word> for F32Literal {
    fn write_operand(value: &Word, _: &dyn SymbolNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            vihaco_abi::names::FloatLiteral(vihaco_abi::word::decode_f32(*value))
        )
    }
}

impl OperandText<Word> for F64Literal {
    fn write_operand(value: &Word, _: &dyn SymbolNames, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            vihaco_abi::names::FloatLiteral(vihaco_abi::word::decode_f64(*value))
        )
    }
}
