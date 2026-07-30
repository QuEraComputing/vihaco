// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::Instruction;
use vihaco::program::{Type, Value};
use vihaco_parser::{BareToken, Ident};

/// Runtime bytecode instructions.
///
/// Source text parses into the separate [`SurfaceInstruction`] enum below.
/// Keeping the source and runtime forms separate lets patterns carry symbolic
/// names and surface types until a resolver converts them to runtime values.
/// Runtime variant order remains stable because it determines derived opcodes.
#[derive(Debug, Clone, PartialEq, Instruction)]
#[instruction(width = 16)]
pub enum RuntimeInstruction {
    // no-ops
    /// span <file:file_id> <start:u32> <end:u32>
    /// `span 0 1 2` — three space-separated u32s.
    Span(u32, u32, u32),

    /// Label definition.
    Label,

    /// `func_start <name>` — marks function entry. `<name>` is symbolic and
    /// orchestrator-resolved; the unit variant carries no payload.
    FunctionStart,
    /// `func_end <name>` — marks function exit (debug only).
    FunctionEnd,

    /// `breakpoint`. Must precede `Branch` (whose token `br` would be a
    /// prefix of `breakpoint`).
    Breakpoint,

    // control flows
    /// `br <target>` — symbolic. Deferred to orchestrator.
    Branch(u32),

    /// `cond_br <true_target>, <false_target>` — symbolic. Deferred.
    ConditionalBranch(u32, u32),

    /// `ret` (bare) is the form real `.sst` uses; numeric `ret <n>` has no
    /// precedent so we defer. Orchestrator emits `Return(0)` for bare `ret`.
    Return(u32),

    /// `call_indirect`. **Must precede `Call`** for the prefix check.
    IndirectCall,

    /// `call <arity>, <addr>` — symbolic addr. Deferred.
    Call(u32, u32),

    /// `halt` — stop execution.
    Halt,

    // traps / IO
    /// `print` — write top-of-stack to stdout.
    Print,

    // memory operations
    /// `load.<type> <address>` — two fields with single-space separator.
    Load(Type, u32),
    /// `store.<type> <address>`.
    Store(Type, u32),

    /// `dup`.
    Dup,

    /// `heap_alloc <n>`.
    HeapAlloc(u32),

    /// `get_item`. Must precede `Ge` (token `ge` ⊂ `get_item`).
    GetItem,

    /// `heap_dealloc` — pops a HeapRef and marks the slot dead, returning it
    /// to the free list for reuse by the next `heap_alloc`.
    HeapDealloc,

    /// `const.<type> <literal>` — numeric/bool only here. `.str`/`.fn_ref`/
    /// `.heap_ref` are orchestrator-handled.
    Const(Value),

    // arithmetic operations
    Add(Type),
    Sub(Type),
    Mul(Type),
    Div(Type),
    Rem(Type),
    Neg(Type),

    // integer / bitwise operations
    Shl(Type),
    Shr(Type),
    Rol(Type),
    Ror(Type),
    BitAnd(Type),
    BitOr(Type),
    BitXor(Type),

    // boolean operations
    Not,
    And,
    Or,
    Xor,

    // comparison operations
    Eq(Type),
    Ne(Type),
    Lt(Type),
    Gt(Type),
    Le(Type),
    Ge(Type),
}

#[derive(Debug, Clone, Copy, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(type)]
pub enum SurfaceType {
    #[pattern = "`undef`"]
    Undefined,
    #[pattern = "`str`"]
    String,
    #[pattern = "`bool`"]
    Bool,
    #[pattern = "`i64`"]
    I64,
    #[pattern = "`u32`"]
    U32,
    #[pattern = "`u64`"]
    U64,
    #[pattern = "`f64`"]
    F64,
    #[pattern = "`fn_ref`"]
    FunctionRef,
    #[pattern = "`heap_ref`"]
    HeapRef,
}

#[derive(Debug, Clone, PartialEq, Eq, vihaco_parser_derive::Parse)]
#[syntax_class(value)]
pub enum SurfaceValue {
    #[pattern = "$0"]
    Quoted(vihaco_parser::QuotedString),
    #[pattern = "$0"]
    Bare(BareToken),
}

#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(instruction, head = "cpu")]
pub enum SurfaceInstruction {
    // no-ops
    /// span <file:file_id> <start:u32> <end:u32>
    /// `span 0 1 2` — three space-separated u32s.
    #[pattern = "'span $0 $1 $2"]
    Span(u32, u32, u32),

    /// Label definition.
    #[pattern = "'label `@` $0"]
    Label(Ident),

    /// `func_start <name>` — marks function entry. `<name>` is symbolic and
    /// orchestrator-resolved; the unit variant carries no payload.
    #[pattern = "'func_start"]
    FunctionStart,
    /// `func_end <name>` — marks function exit (debug only).
    #[pattern = "'func_end"]
    FunctionEnd,

    /// `breakpoint`. Must precede `Branch` (whose token `br` would be a
    /// prefix of `breakpoint`).
    Breakpoint,

    // control flows
    /// `br <target>` — symbolic. Deferred to orchestrator.
    #[pattern = "'br `@` $0"]
    Branch(Ident),

    /// `cond_br <true_target>, <false_target>` — symbolic. Deferred.
    #[pattern = "'cond_br `@` $0 `,` `@` $1"]
    ConditionalBranch(Ident, Ident),

    /// `ret` (bare) is the form real `.sst` uses; numeric `ret <n>` has no
    /// precedent so we defer. Orchestrator emits `Return(0)` for bare `ret`.
    #[pattern = "'ret"]
    Return,

    /// `call_indirect`. **Must precede `Call`** for the prefix check.
    #[pattern = "'call_indirect"]
    IndirectCall,

    /// `call <arity>, <addr>` — symbolic addr. Deferred.
    Call(u32, Ident),

    /// `halt` — stop execution.
    Halt,

    // traps / IO
    /// `print` — write top-of-stack to stdout.
    Print,

    // memory operations
    /// `load.<type> <address>` — two fields with single-space separator.
    Load(SurfaceType, u32),

    /// `store.<type> <address>`.
    Store(SurfaceType, u32),

    /// `dup`.
    Dup,

    /// `heap_alloc <n>`.
    #[pattern = "'heap_alloc $0"]
    HeapAlloc(u32),

    /// `get_item`. Must precede `Ge` (token `ge` ⊂ `get_item`).
    #[pattern = "'get_item"]
    GetItem,

    /// `heap_dealloc` — pops a HeapRef and marks the slot dead, returning it
    /// to the free list for reuse by the next `heap_alloc`.
    #[pattern = "'heap_dealloc"]
    HeapDealloc,

    /// `const.<type> <literal>` — numeric/bool only here. `.str`/`.fn_ref`/
    /// `.heap_ref` are orchestrator-handled.
    Const(SurfaceType, SurfaceValue),

    // arithmetic operations
    Add(SurfaceType),
    Sub(SurfaceType),
    Mul(SurfaceType),
    Div(SurfaceType),
    Rem(SurfaceType),
    Neg(SurfaceType),

    // integer / bitwise operations
    Shl(SurfaceType),
    Shr(SurfaceType),
    Rol(SurfaceType),
    Ror(SurfaceType),
    #[pattern = "'bitand $0"]
    BitAnd(SurfaceType),
    #[pattern = "'bitor $0"]
    BitOr(SurfaceType),
    #[pattern = "'bitxor $0"]
    BitXor(SurfaceType),

    // boolean operations
    Not,
    And,
    Or,
    Xor,

    // comparison operations
    Eq(SurfaceType),
    Ne(SurfaceType),
    Lt(SurfaceType),
    Gt(SurfaceType),
    Le(SurfaceType),
    Ge(SurfaceType),
}

impl<T: Into<Value>> From<T> for RuntimeInstruction {
    fn from(value: T) -> Self {
        RuntimeInstruction::Const(value.into())
    }
}

impl vihaco::CanonicalInstructionSyntax for RuntimeInstruction {
    fn variants() -> &'static [vihaco::CanonicalInstructionVariantSyntax] {
        &[
            vihaco::CanonicalInstructionVariantSyntax {
                mnemonic: "cpu::const_i64",
                operands: &[vihaco::OperandKind::I64],
            },
            vihaco::CanonicalInstructionVariantSyntax {
                mnemonic: "cpu::const_f64",
                operands: &[vihaco::OperandKind::F64],
            },
            vihaco::CanonicalInstructionVariantSyntax {
                mnemonic: "cpu::const_bool",
                operands: &[vihaco::OperandKind::Bool],
            },
            vihaco::CanonicalInstructionVariantSyntax {
                mnemonic: "cpu::const_u64",
                operands: &[vihaco::OperandKind::NonNegativeU64],
            },
            vihaco::CanonicalInstructionVariantSyntax {
                mnemonic: "cpu::fn_ref",
                operands: &[vihaco::OperandKind::Symbol],
            },
            vihaco::CanonicalInstructionVariantSyntax {
                mnemonic: "cpu::call_direct",
                operands: &[vihaco::OperandKind::Symbol],
            },
        ]
    }
}

#[cfg(test)]
#[allow(clippy::approx_constant)]
mod parse_tests {
    use super::{BareToken, SurfaceInstruction, SurfaceType, SurfaceValue};
    use chumsky::Parser as _;
    use vihaco_parser::Parse;

    fn parse(input: &str) -> SurfaceInstruction {
        SurfaceInstruction::parser()
            .parse(input)
            .into_result()
            .unwrap_or_else(|e| panic!("parse({input:?}) failed: {e:?}"))
    }

    fn parse_type(input: &str) -> SurfaceType {
        SurfaceType::parser()
            .parse(input)
            .into_result()
            .unwrap_or_else(|e| panic!("parse_type({input:?}) failed: {e:?}"))
    }

    macro_rules! assert_parses {
        ($input:literal, $pattern:pat $(if $guard:expr)?) => {
            assert!(
                matches!(parse($input), $pattern $(if $guard)?),
                "input {:?} parsed to the wrong variant or operands",
                $input
            );
        };
    }

    #[test]
    fn parses_unit_variants() {
        assert_parses!("cpu::halt", SurfaceInstruction::Halt);
        assert_parses!("cpu::print", SurfaceInstruction::Print);
        assert_parses!("cpu::dup", SurfaceInstruction::Dup);
        assert_parses!("cpu::breakpoint", SurfaceInstruction::Breakpoint);
        assert_parses!(
            "cpu::label @loop",
            SurfaceInstruction::Label(name) if name.as_str() == "loop"
        );
        assert_parses!("cpu::func_start", SurfaceInstruction::FunctionStart);
        assert_parses!("cpu::func_end", SurfaceInstruction::FunctionEnd);
        assert_parses!("cpu::get_item", SurfaceInstruction::GetItem);
        assert_parses!("cpu::not", SurfaceInstruction::Not);
        assert_parses!("cpu::and", SurfaceInstruction::And);
        assert_parses!("cpu::or", SurfaceInstruction::Or);
        assert_parses!("cpu::xor", SurfaceInstruction::Xor);
        assert_parses!("cpu::call_indirect", SurfaceInstruction::IndirectCall);
        assert_parses!("cpu::ret", SurfaceInstruction::Return);
    }

    #[test]
    fn parses_surface_types() {
        for (input, expected) in [
            ("undef", SurfaceType::Undefined),
            ("str", SurfaceType::String),
            ("bool", SurfaceType::Bool),
            ("i64", SurfaceType::I64),
            ("u32", SurfaceType::U32),
            ("u64", SurfaceType::U64),
            ("f64", SurfaceType::F64),
            ("fn_ref", SurfaceType::FunctionRef),
            ("heap_ref", SurfaceType::HeapRef),
        ] {
            assert_eq!(parse_type(input), expected, "input {input:?}");
        }
    }

    #[test]
    fn parses_typed_operations() {
        assert_parses!("cpu::add i64", SurfaceInstruction::Add(SurfaceType::I64));
        assert_parses!("cpu::sub f64", SurfaceInstruction::Sub(SurfaceType::F64));
        assert_parses!("cpu::mul u32", SurfaceInstruction::Mul(SurfaceType::U32));
        assert_parses!("cpu::div u64", SurfaceInstruction::Div(SurfaceType::U64));
        assert_parses!("cpu::rem i64", SurfaceInstruction::Rem(SurfaceType::I64));
        assert_parses!("cpu::neg f64", SurfaceInstruction::Neg(SurfaceType::F64));
        assert_parses!("cpu::lt i64", SurfaceInstruction::Lt(SurfaceType::I64));
        assert_parses!("cpu::eq i64", SurfaceInstruction::Eq(SurfaceType::I64));
        assert_parses!("cpu::ne u64", SurfaceInstruction::Ne(SurfaceType::U64));
        assert_parses!("cpu::gt u32", SurfaceInstruction::Gt(SurfaceType::U32));
        assert_parses!("cpu::le f64", SurfaceInstruction::Le(SurfaceType::F64));
        assert_parses!("cpu::ge f64", SurfaceInstruction::Ge(SurfaceType::F64));
        assert_parses!(
            "cpu::bitand i64",
            SurfaceInstruction::BitAnd(SurfaceType::I64)
        );
        assert_parses!(
            "cpu::bitor u64",
            SurfaceInstruction::BitOr(SurfaceType::U64)
        );
        assert_parses!(
            "cpu::bitxor u32",
            SurfaceInstruction::BitXor(SurfaceType::U32)
        );
        assert_parses!("cpu::shl u64", SurfaceInstruction::Shl(SurfaceType::U64));
        assert_parses!("cpu::shr i64", SurfaceInstruction::Shr(SurfaceType::I64));
        assert_parses!("cpu::rol u32", SurfaceInstruction::Rol(SurfaceType::U32));
        assert_parses!("cpu::ror u64", SurfaceInstruction::Ror(SurfaceType::U64));
    }

    #[test]
    fn parses_load_store() {
        assert_parses!(
            "cpu::load i64, 7",
            SurfaceInstruction::Load(SurfaceType::I64, 7)
        );
        assert_parses!(
            "cpu::store f64, 42",
            SurfaceInstruction::Store(SurfaceType::F64, 42)
        );
    }

    #[test]
    fn parses_heap_alloc() {
        assert_parses!("cpu::heap_alloc 5", SurfaceInstruction::HeapAlloc(5));
    }

    #[test]
    fn parses_span() {
        assert_parses!("cpu::span 0 1 2", SurfaceInstruction::Span(0, 1, 2));
    }

    #[test]
    fn parses_const_numeric_flavors() {
        assert_parses!(
            "cpu::const i64, 42",
            SurfaceInstruction::Const(SurfaceType::I64, value)
                if value == SurfaceValue::Bare(BareToken("42".to_owned()))
        );
        assert_parses!(
            "cpu::const u64, 7",
            SurfaceInstruction::Const(SurfaceType::U64, value)
                if value == SurfaceValue::Bare(BareToken("7".to_owned()))
        );
        assert_parses!(
            "cpu::const u32, 3",
            SurfaceInstruction::Const(SurfaceType::U32, value)
                if value == SurfaceValue::Bare(BareToken("3".to_owned()))
        );
        assert_parses!(
            "cpu::const f64, 3.14",
            SurfaceInstruction::Const(SurfaceType::F64, value)
                if value == SurfaceValue::Bare(BareToken("3.14".to_owned()))
        );
        assert_parses!(
            "cpu::const bool, true",
            SurfaceInstruction::Const(SurfaceType::Bool, value)
                if value == SurfaceValue::Bare(BareToken("true".to_owned()))
        );
    }

    #[test]
    fn parses_const_quoted_string() {
        assert_parses!(
            "cpu::const str, \"hello world\"",
            SurfaceInstruction::Const(SurfaceType::String, SurfaceValue::Quoted(value))
                if value.as_str() == "hello world"
        );
    }

    #[test]
    fn parses_symbolic_control_flow() {
        assert_parses!(
            "cpu::br @body",
            SurfaceInstruction::Branch(target) if target.as_str() == "body"
        );
        assert_parses!(
            "cpu::cond_br @then, @else",
            SurfaceInstruction::ConditionalBranch(then_target, else_target)
                if then_target.as_str() == "then" && else_target.as_str() == "else"
        );
        assert_parses!(
            "cpu::call 2, main",
            SurfaceInstruction::Call(2, target) if target.as_str() == "main"
        );
    }

    #[test]
    fn rejects_malformed_quoted_value_instead_of_treating_it_as_bare() {
        assert!(
            SurfaceInstruction::parser()
                .parse("cpu::const str, \"unterminated")
                .has_errors()
        );
    }

    #[test]
    fn rejects_legacy_runtime_instruction_syntax() {
        assert!(
            SurfaceInstruction::parser()
                .parse("const.i64 42")
                .has_errors()
        );
        assert!(SurfaceInstruction::parser().parse("br @body").has_errors());
    }
}
