// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Parsed-syntax data shapes. See module docs in [`super`].

/// Parsed `.sst` module — pre-resolution. `H` is the consumer's device-header
/// enum (typically derives `Parse` via Item 5's `DeviceHeader`).
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedModule<I, H> {
    pub headers: Vec<H>,
    pub functions: Vec<ParsedFunction<I>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFunction<I> {
    /// Function name with the leading `@` stripped (`@main` → `"main"`).
    pub name: String,
    /// Empty for the moment — `.sst` examples don't exercise parameters.
    /// Non-empty parameter syntax errors during parsing.
    pub params: Vec<Param>,
    /// Return type as a bare token (`i64`, `f64`, …). Resolver converts to
    /// `vihaco::Type`.
    pub return_ty: Option<RawType>,
    pub body: Vec<BodyItem<I>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: RawType,
}

/// Bare type token — `"i64"`, `"f64"`, …. The resolver translates to
/// [`crate::program::Type`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RawType(pub String);

/// One source-level body item. Resolved into zero-or-more `I` values.
#[derive(Debug, Clone, PartialEq)]
pub enum BodyItem<I> {
    /// Canonical, derive-parsed instruction — `I::parser()` succeeded.
    Direct(I),
    /// Untyped source form — sugar (`poly <addr> 1.0`) or symbolic operand
    /// (`br @label`, `const.str "hi"`) that needs interner / symbol-table
    /// access. The consumer's [`super::Resolve`] impl expands it.
    Raw(RawForm),
}

/// Untyped source form: a mnemonic followed by some operands. The shape is
/// intentionally lossless — the resolver decides what each form means based
/// on the mnemonic.
#[derive(Debug, Clone, PartialEq)]
pub struct RawForm {
    pub mnemonic: String,
    pub operands: Vec<RawOperand>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RawOperand {
    /// Bare token: `AOD0:T1:A`, `DIGI:0`, `i64`, dotted device names.
    Ident(String),
    Int(i64),
    UInt(u64),
    Float(f64),
    Bool(bool),
    /// Contents of `"…"` with escape sequences decoded.
    StringLit(String),
    /// `@name` form — the `@` is consumed; the name remains.
    Symbol(String),
}
