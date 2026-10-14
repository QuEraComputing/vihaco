// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Literal parser values converted into encoded runtime words.

use crate::Word;

#[derive(vihaco::Parse)]
#[syntax_class(value)]
pub(crate) struct I32Literal(i32);

impl From<I32Literal> for Word {
    fn from(value: I32Literal) -> Self {
        crate::word::encode_i32(value.0)
    }
}

#[derive(vihaco::Parse)]
#[syntax_class(value)]
pub(crate) struct I64Literal(i64);

impl From<I64Literal> for Word {
    fn from(value: I64Literal) -> Self {
        crate::word::encode_i64(value.0)
    }
}

#[derive(vihaco::Parse)]
#[syntax_class(value)]
pub(crate) struct F32Literal(f32);

impl From<F32Literal> for Word {
    fn from(value: F32Literal) -> Self {
        crate::word::encode_f32(value.0)
    }
}

#[derive(vihaco::Parse)]
#[syntax_class(value)]
pub(crate) struct F64Literal(f64);

impl From<F64Literal> for Word {
    fn from(value: F64Literal) -> Self {
        crate::word::encode_f64(value.0)
    }
}
