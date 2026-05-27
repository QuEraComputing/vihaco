use vihaco::Instruction;
use vihaco::value::{Type, Value};

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
#[derive(Debug, Clone, PartialEq, Instruction, vihaco_parser::Parse)]
#[instruction(width = 16)]
pub enum Instruction {
    // no-ops
    /// span <file:file_id> <start:u32> <end:u32>
    /// `span 0 1 2` — three space-separated u32s.
    #[delimiters(open = "", close = "", separator = " ")]
    Span(u32, u32, u32),

    /// Label definition.
    Label,

    /// `func_start <name>` — marks function entry. `<name>` is symbolic and
    /// orchestrator-resolved; the unit variant carries no payload.
    #[token = "func_start"]
    FunctionStart,
    /// `func_end <name>` — marks function exit (debug only).
    #[token = "func_end"]
    FunctionEnd,

    /// `breakpoint`. Must precede `Branch` (whose token `br` would be a
    /// prefix of `breakpoint`).
    Breakpoint,

    // control flows
    /// `br <target>` — symbolic. Deferred to orchestrator.
    #[token = "br"]
    #[delimiters(open = "", close = "", separator = "")]
    Branch(#[parse_with = "crate::parse_helpers::never_u32"] u32),

    /// `cond_br <true_target>, <false_target>` — symbolic. Deferred.
    #[token = "cond_br"]
    #[delimiters(open = "", close = "", separator = ",")]
    ConditionalBranch(
        #[parse_with = "crate::parse_helpers::never_u32"] u32,
        #[parse_with = "crate::parse_helpers::never_u32"] u32,
    ),

    /// `ret` (bare) is the form real `.sst` uses; numeric `ret <n>` has no
    /// precedent so we defer. Orchestrator emits `Return(0)` for bare `ret`.
    #[token = "ret"]
    #[delimiters(open = "", close = "", separator = "")]
    Return(#[parse_with = "crate::parse_helpers::never_u32"] u32),

    /// `call_indirect`. **Must precede `Call`** for the prefix check.
    #[token = "call_indirect"]
    IndirectCall,

    /// `call <arity>, <addr>` — symbolic addr. Deferred.
    #[token = "call"]
    #[delimiters(open = "", close = "", separator = ",")]
    Call(
        #[parse_with = "crate::parse_helpers::never_u32"] u32,
        #[parse_with = "crate::parse_helpers::never_u32"] u32,
    ),

    /// `halt` — stop execution.
    Halt,

    // traps / IO
    /// `print` — write top-of-stack to stdout.
    Print,

    // memory operations
    /// `load.<type> <address>` — two fields with single-space separator.
    #[delimiters(open = "", close = "", separator = " ")]
    Load(#[parse_with = "crate::parse_helpers::cpu_type"] Type, u32),
    /// `store.<type> <address>`.
    #[delimiters(open = "", close = "", separator = " ")]
    Store(#[parse_with = "crate::parse_helpers::cpu_type"] Type, u32),

    /// `dup`.
    Dup,

    /// `heap_alloc <n>`.
    #[token = "heap_alloc"]
    #[delimiters(open = "", close = "", separator = "")]
    HeapAlloc(u32),

    /// `get_item`. Must precede `Ge` (token `ge` ⊂ `get_item`).
    #[token = "get_item"]
    GetItem,

    /// `const.<type> <literal>` — numeric/bool only here. `.str`/`.fn_ref`/
    /// `.heap_ref` are orchestrator-handled.
    #[token = "const"]
    #[delimiters(open = "", close = "", separator = "")]
    Const(#[parse_with = "crate::parse_helpers::cpu_const_value"] Value),

    // arithmetic operations
    #[delimiters(open = "", close = "", separator = "")]
    Add(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Sub(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Mul(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Div(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Rem(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Neg(#[parse_with = "crate::parse_helpers::cpu_type"] Type),

    // integer / bitwise operations
    #[delimiters(open = "", close = "", separator = "")]
    Shl(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Shr(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Rol(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Ror(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[token = "bitand"]
    #[delimiters(open = "", close = "", separator = "")]
    BitAnd(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[token = "bitor"]
    #[delimiters(open = "", close = "", separator = "")]
    BitOr(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[token = "bitxor"]
    #[delimiters(open = "", close = "", separator = "")]
    BitXor(#[parse_with = "crate::parse_helpers::cpu_type"] Type),

    // boolean operations
    Not,
    And,
    Or,
    Xor,

    // comparison operations
    #[delimiters(open = "", close = "", separator = "")]
    Eq(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Ne(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Lt(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Gt(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Le(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
    #[delimiters(open = "", close = "", separator = "")]
    Ge(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
}

impl<T: Into<Value>> From<T> for Instruction {
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
#[allow(clippy::approx_constant)]
mod parse_tests {
    use super::Instruction;
    use chumsky::Parser as _;
    use vihaco::value::{Type, Value};
    use vihaco_parser_core::Parse;

    fn parse(input: &str) -> Instruction {
        Instruction::parser()
            .parse(input)
            .into_result()
            .unwrap_or_else(|e| panic!("parse({input:?}) failed: {e:?}"))
    }

    #[test]
    fn parses_unit_variants() {
        for (input, expected) in [
            ("halt", Instruction::Halt),
            ("print", Instruction::Print),
            ("dup", Instruction::Dup),
            ("breakpoint", Instruction::Breakpoint),
            ("label", Instruction::Label),
            ("func_start", Instruction::FunctionStart),
            ("func_end", Instruction::FunctionEnd),
            ("get_item", Instruction::GetItem),
            ("not", Instruction::Not),
            ("and", Instruction::And),
            ("or", Instruction::Or),
            ("xor", Instruction::Xor),
            ("call_indirect", Instruction::IndirectCall),
        ] {
            assert_eq!(parse(input), expected, "input {input:?}");
        }
    }

    #[test]
    fn parses_typed_arith() {
        assert_eq!(parse("add.i64"), Instruction::Add(Type::I64));
        assert_eq!(parse("sub.f64"), Instruction::Sub(Type::F64));
        assert_eq!(parse("mul.u32"), Instruction::Mul(Type::U32));
        assert_eq!(parse("div.u64"), Instruction::Div(Type::U64));
        assert_eq!(parse("lt.i64"), Instruction::Lt(Type::I64));
        assert_eq!(parse("ge.f64"), Instruction::Ge(Type::F64));
        assert_eq!(parse("bitand.i64"), Instruction::BitAnd(Type::I64));
        assert_eq!(parse("shl.u64"), Instruction::Shl(Type::U64));
    }

    #[test]
    fn parses_load_store() {
        assert_eq!(parse("load.i64 7"), Instruction::Load(Type::I64, 7));
        assert_eq!(parse("store.f64 42"), Instruction::Store(Type::F64, 42));
    }

    #[test]
    fn parses_heap_alloc() {
        assert_eq!(parse("heap_alloc 5"), Instruction::HeapAlloc(5));
    }

    #[test]
    fn parses_span() {
        assert_eq!(parse("span 0 1 2"), Instruction::Span(0, 1, 2));
    }

    #[test]
    fn parses_const_numeric_flavors() {
        assert_eq!(parse("const.i64 42"), Instruction::Const(Value::I64(42)));
        assert_eq!(parse("const.u64 7"), Instruction::Const(Value::U64(7)));
        assert_eq!(parse("const.u32 3"), Instruction::Const(Value::U32(3)));
        assert_eq!(
            parse("const.f64 3.14"),
            Instruction::Const(Value::F64(3.14))
        );
        assert_eq!(
            parse("const.bool true"),
            Instruction::Const(Value::Bool(true))
        );
    }

    #[test]
    fn defers_symbolic_branch_to_orchestrator() {
        // `br @body` cannot be parsed by the derive — `never_u32` ensures this.
        // The Module orchestrator (Item 4) handles the symbolic form.
        assert!(Instruction::parser().parse("br @body").has_errors());
        assert!(Instruction::parser().parse("cond_br @a, @b").has_errors());
        assert!(Instruction::parser().parse("call @main, 0").has_errors());
        assert!(Instruction::parser().parse("ret").has_errors());
    }

    #[test]
    fn defers_const_string_to_orchestrator() {
        // `const.str "hello"` requires interner state — handled by the orchestrator.
        assert!(
            Instruction::parser()
                .parse("const.str \"hello\"")
                .has_errors()
        );
    }
}
