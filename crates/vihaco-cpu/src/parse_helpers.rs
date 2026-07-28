// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Low-level chumsky parsers for CPU-specific source spellings.
//!
//! `SurfaceInstruction` uses declarative patterns. These combinators remain
//! useful to callers implementing `Parse` manually for lower-level CPU syntax
//! types.

use chumsky::error::Simple;
use chumsky::extra;
use chumsky::prelude::*;
use vihaco::program::{Type, Value};
use vihaco_parser_core::Parse;

type E<'src> = extra::Err<Simple<'src, char>>;

/// Parses `.<typename>` and returns the matching [`Type`]. Used for the typed
/// arithmetic/comparison variants — `add.i64`, `lt.u64`, etc.
pub fn cpu_type<'src>() -> impl Parser<'src, &'src str, Type, E<'src>> {
    just('.').ignore_then(choice((
        just("i64").to(Type::I64),
        just("u64").to(Type::U64),
        just("u32").to(Type::U32),
        just("f64").to(Type::F64),
        just("bool").to(Type::Bool),
    )))
}

/// Parses the body of `const.<type> <literal>` without the leading `const`
/// keyword. Numeric and bool variants only.
///
/// String, FunctionRef, and HeapRef variants of `Value` are intentionally
/// excluded: they require the orchestrator's interner/symbol tables.
pub fn cpu_const_value<'src>() -> impl Parser<'src, &'src str, Value, E<'src>> {
    choice((
        just(".i64")
            .ignore_then(text::whitespace())
            .ignore_then(i64::parser())
            .map(Value::I64),
        just(".u64")
            .ignore_then(text::whitespace())
            .ignore_then(u64::parser())
            .map(Value::U64),
        just(".u32")
            .ignore_then(text::whitespace())
            .ignore_then(u32::parser())
            .map(Value::U32),
        just(".f64")
            .ignore_then(text::whitespace())
            .ignore_then(f64::parser())
            .map(Value::F64),
        just(".bool")
            .ignore_then(text::whitespace())
            .ignore_then(bool::parser())
            .map(Value::Bool),
    ))
}

/// Returns a parser that always fails.
///
/// This is useful when a hand-written parser needs to reject a branch
/// deliberately while preserving an expected `u32` output type.
pub fn never_u32<'src>() -> impl Parser<'src, &'src str, u32, E<'src>> {
    empty().try_map(|_, span| Err(Simple::new(None, span)))
}
