// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::Instruction;
use vihaco::program::value_cpu::CPUValueSyntax;
use vihaco::program::{CPUType, CPUValue};

/// `#[derive(Parse)]` notes:
///
/// - Real `.sst` syntax uses **dot-suffixed** types (`add.i64`, `load.i64 0`).
///   The `parse_helpers::cpu_type` / `cpu_const_value` helpers consume the
///   leading `.`; that's why the typed variants set `delimiters(open = "",
///   close = "", separator = "")` and use `#[parse_with]` on the `Type` field.
/// - `Const(Value::String/FunctionRef/HeapRef)`, `Branch(_)`,
///   `ConditionalBranch(_, _)`, `Call(_, _)`, and bare `ret` use symbolic
///   operands that need a shared interner / symbol table not available to a
///   stateless `Parse` impl. Their `parse_with` helpers return `never_u32` so
///   `Instruction::parser()` errors on those mnemonics — the Module
///   orchestrator (Item 4 of the migration plan) intercepts them first.
/// - Variant order is preserved from the pre-migration layout so derived
///   opcodes stay stable. The single exception: `IndirectCall` is moved
///   ahead of `Call` so the prefix-ordering check (`call` ⊂ `call_indirect`)
///   passes.
#[derive(Debug, Clone, PartialEq, Instruction)]
#[instruction(width = 16)]
pub enum Instruction {
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
    Load(CPUType, u32),

    /// `store.<type> <address>`.
    Store(CPUType, u32),

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
    Const(CPUValue),

    // arithmetic operations
    Add(CPUType),
    Sub(CPUType),
    Mul(CPUType),
    Div(CPUType),
    Rem(CPUType),
    Neg(CPUType),

    // integer / bitwise operations
    Shl(CPUType),
    Shr(CPUType),
    Rol(CPUType),
    Ror(CPUType),
    BitAnd(CPUType),
    BitOr(CPUType),
    BitXor(CPUType),

    // boolean operations
    Not,
    And,
    Or,
    Xor,

    // comparison operations
    Eq(CPUType),
    Ne(CPUType),
    Lt(CPUType),
    Gt(CPUType),
    Le(CPUType),
    Ge(CPUType),
}

#[derive(Debug, PartialEq, vihaco_parser::Parse)]
#[syntax_class(instruction, head = "cpu")]
pub enum RawInstruction {
    #[pattern = "'span $0 $1 $2"]
    Span(u32, u32, u32),

    #[pattern = "'br `@` $0"]
    Branch(u32),

    #[pattern = "'cond_br `@` $0 `,` `@` $1"]
    ConditionalBranch(CPUValueSyntax, CPUValueSyntax),

    #[pattern = "'ret $0"]
    Return(u32),

    #[pattern = "'call $0 `,` $1"]
    Call(u32, CPUValueSyntax),

    #[pattern = "'load $0 $1"]
    Load(CPUType, u32),

    #[pattern = "'store $0 $1"]
    Store(CPUType, u32),

    #[pattern = "'heap_alloc $0"]
    HeapAlloc(u32),

    #[pattern = "'const $0"]
    Const(u32),

    // arithmetic operations
    #[pattern = "'add $0"]
    Add(CPUType),
    Sub(CPUType),
    Mul(CPUType),
    Div(CPUType),
    Rem(CPUType),
    Neg(CPUType),

    Bar(u32, u32, u32),

    // integer / bitwise operations
    Shl(CPUType),
    Shr(CPUType),
    Rol(CPUType),
    Ror(CPUType),
    BitAnd(CPUType),
    BitOr(CPUType),
    BitXor(CPUType),

    // boolean operations
    // comparison operations
    Eq(CPUType),
    Ne(CPUType),
    Lt(CPUType),
    Gt(CPUType),
    Le(CPUType),
    Ge(CPUType),
}

impl<T: Into<CPUValue>> From<T> for Instruction {
    fn from(value: T) -> Self {
        Instruction::Const(value.into())
    }
}

impl vihaco::CanonicalInstructionSyntax for Instruction {
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
mod parse_tests {
    use super::RawInstruction;
    use chumsky::Parser as _;
    use vihaco::program::CPUType;
    use vihaco_parser_core::Parse;

    fn parse(input: &str) -> RawInstruction {
        RawInstruction::parser()
            .parse(input)
            .into_result()
            .unwrap_or_else(|e| panic!("parse({input:?}) failed: {e:?}"))
    }

    #[test]
    fn parses_explicit_pattern() {
        assert_eq!(parse("cpu::span 0 1 2"), RawInstruction::Span(0, 1, 2));
    }

    #[test]
    fn parses_generated_instruction_pattern() {
        assert_eq!(parse("cpu::sub f64"), RawInstruction::Sub(CPUType::F64));
        assert_eq!(
            parse("cpu::bitand i64"),
            RawInstruction::BitAnd(CPUType::I64)
        );
    }

    #[test]
    fn rejects_missing_operands() {
        assert!(RawInstruction::parser().parse("cpu::span 0 1").has_errors());
        assert!(RawInstruction::parser().parse("cpu::add").has_errors());
    }
}
