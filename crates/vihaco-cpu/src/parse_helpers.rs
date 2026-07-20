// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Field-level parser helpers wired into `#[derive(vihaco_parser::Parse)]` via
//! `#[parse_with = "..."]` on the `vihaco_cpu::Instruction` enum.
//!
//! Scope intentionally narrow: the helpers cover only the subset that
//! `#[derive(Parse)]` can model cleanly. Three families of variants are
//! **deferred to the Module orchestrator** (Item 4 of the migration plan):
//!
//! - `Const(Value::String/FunctionRef/HeapRef)` — needs a shared string
//!   interner / symbol table that `Parse` has no way to thread through.
//!   `cpu_const_value()` parses only the numeric/bool flavours.
//! - `Branch`, `ConditionalBranch`, `Call` — use symbolic `@label` targets in
//!   real `.sst` source; the symbol table lives in the orchestrator. The
//!   `never_u32()` helper guarantees `Instruction::parser()` errors out on these
//!   mnemonics so the orchestrator can dispatch first.
//! - Conversion from the parse-time numeric form to the orchestrator-resolved
//!   form is tracked in `~/.claude/plans/vihaco-future-rawvalue-refactor.md`.

use chumsky::error::Simple;
use chumsky::extra;
use chumsky::prelude::*;
use vihaco::program::{CPUType, CPUValue};
use vihaco_parser_core::Parse;

type E<'src> = extra::Err<Simple<'src, char>>;

/// Parses `.<typename>` and returns the matching [`Type`]. Used for the typed
/// arithmetic/comparison variants — `add.i64`, `lt.u64`, etc.
pub fn cpu_type<'src>() -> impl Parser<'src, &'src str, CPUType, E<'src>> {
    just('.').ignore_then(choice((
        just("i64").to(CPUType::I64),
        just("u64").to(CPUType::U64),
        just("u32").to(CPUType::U32),
        just("f64").to(CPUType::F64),
        just("bool").to(CPUType::Bool),
    )))
}

/// Parses the body of `const.<type> <literal>` — without the leading `const`
/// keyword (the derive macro emits that). Numeric and bool variants only.
///
/// String, FunctionRef, and HeapRef variants of `Value` are intentionally
/// excluded: they require the orchestrator's interner/symbol tables.
pub fn cpu_const_value<'src>() -> impl Parser<'src, &'src str, CPUValue, E<'src>> {
    choice((
        just(".i64")
            .ignore_then(text::whitespace())
            .ignore_then(i64::parser())
            .map(CPUValue::I64),
        just(".u64")
            .ignore_then(text::whitespace())
            .ignore_then(u64::parser())
            .map(CPUValue::U64),
        just(".u32")
            .ignore_then(text::whitespace())
            .ignore_then(u32::parser())
            .map(CPUValue::U32),
        just(".f64")
            .ignore_then(text::whitespace())
            .ignore_then(f64::parser())
            .map(CPUValue::F64),
        just(".bool")
            .ignore_then(text::whitespace())
            .ignore_then(bool::parser())
            .map(CPUValue::Bool),
    ))
}

/// A parser that always fails. Used for variant operands whose real form is a
/// symbolic `@label` (`Branch`, `ConditionalBranch`, `Call`). Letting these
/// fall through here means `Instruction::parser()` errors on their mnemonics,
/// which is correct — the orchestrator must intercept first.
pub fn never_u32<'src>() -> impl Parser<'src, &'src str, u32, E<'src>> {
    empty().try_map(|_, span| Err(Simple::new(None, span)))
}
