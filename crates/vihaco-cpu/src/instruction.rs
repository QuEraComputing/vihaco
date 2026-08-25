// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser::BareToken;

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
    #[pattern = "`i32`"]
    I32,
    #[pattern = "`u32`"]
    U32,
    #[pattern = "`u64`"]
    U64,
    #[pattern = "`f64`"]
    F64,
    #[pattern = "`f32`"]
    F32,
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

#[cfg(test)]
#[allow(clippy::approx_constant)]
mod parse_tests {
    use super::SurfaceType;
    use crate::SurfaceInstruction;
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
        assert_parses!("cpu.halt", SurfaceInstruction::Halt);
        assert_parses!("cpu.print", SurfaceInstruction::Print);
        assert_parses!("cpu.dup", SurfaceInstruction::Dup);
        assert_parses!("cpu.breakpoint", SurfaceInstruction::Breakpoint);
        assert_parses!(
            "cpu.label @loop",
            SurfaceInstruction::Label(name) if name.as_str() == "loop"
        );
        assert_parses!("cpu.func_start", SurfaceInstruction::FunctionStart);
        assert_parses!("cpu.func_end", SurfaceInstruction::FunctionEnd);
        assert_parses!("cpu.get_item", SurfaceInstruction::GetItem);
        assert_parses!("cpu.not", SurfaceInstruction::Not);
        assert_parses!("cpu.and", SurfaceInstruction::And);
        assert_parses!("cpu.or", SurfaceInstruction::Or);
        assert_parses!("cpu.xor", SurfaceInstruction::Xor);
        assert_parses!("cpu.call_indirect", SurfaceInstruction::IndirectCall);
        assert_parses!("cpu.ret 0", SurfaceInstruction::Return(0));
    }

    #[test]
    fn parses_explicit_typed_variants() {
        assert_parses!("cpu.add_i32", SurfaceInstruction::AddI32);
        assert_parses!("cpu.add_f32", SurfaceInstruction::AddF32);
        assert_parses!("cpu.load_i64 7", SurfaceInstruction::LoadI64(7));
        assert_parses!("cpu.const_i64 42", SurfaceInstruction::ConstI64(_));
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

    #[cfg(any())]
    #[test]
    fn parses_typed_operations() {
        assert_parses!("cpu.add i64", SurfaceInstruction::Add(SurfaceType::I64));
        assert_parses!("cpu.sub f64", SurfaceInstruction::Sub(SurfaceType::F64));
        assert_parses!("cpu.mul u32", SurfaceInstruction::Mul(SurfaceType::U32));
        assert_parses!("cpu.div u64", SurfaceInstruction::Div(SurfaceType::U64));
        assert_parses!("cpu.rem i64", SurfaceInstruction::Rem(SurfaceType::I64));
        assert_parses!("cpu.neg f64", SurfaceInstruction::Neg(SurfaceType::F64));
        assert_parses!("cpu.lt i64", SurfaceInstruction::Lt(SurfaceType::I64));
        assert_parses!("cpu.eq i64", SurfaceInstruction::Eq(SurfaceType::I64));
        assert_parses!("cpu.ne u64", SurfaceInstruction::Ne(SurfaceType::U64));
        assert_parses!("cpu.gt u32", SurfaceInstruction::Gt(SurfaceType::U32));
        assert_parses!("cpu.le f64", SurfaceInstruction::Le(SurfaceType::F64));
        assert_parses!("cpu.ge f64", SurfaceInstruction::Ge(SurfaceType::F64));
        assert_parses!(
            "cpu.bitand i64",
            SurfaceInstruction::BitAnd(SurfaceType::I64)
        );
        assert_parses!("cpu.bitor u64", SurfaceInstruction::BitOr(SurfaceType::U64));
        assert_parses!(
            "cpu.bitxor u32",
            SurfaceInstruction::BitXor(SurfaceType::U32)
        );
        assert_parses!("cpu.shl u64", SurfaceInstruction::Shl(SurfaceType::U64));
        assert_parses!("cpu.shr i64", SurfaceInstruction::Shr(SurfaceType::I64));
        assert_parses!("cpu.rol u32", SurfaceInstruction::Rol(SurfaceType::U32));
        assert_parses!("cpu.ror u64", SurfaceInstruction::Ror(SurfaceType::U64));
    }

    #[cfg(any())]
    #[test]
    fn parses_load_store() {
        assert_parses!(
            "cpu.load i64, 7",
            SurfaceInstruction::Load(SurfaceType::I64, 7)
        );
        assert_parses!(
            "cpu.store f64, 42",
            SurfaceInstruction::Store(SurfaceType::F64, 42)
        );
    }

    #[test]
    fn parses_heap_alloc() {
        assert_parses!("cpu.heap_alloc 5", SurfaceInstruction::HeapAlloc(5));
    }

    #[test]
    fn parses_span() {
        assert_parses!("cpu.span 0 1 2", SurfaceInstruction::Span(0, 1, 2));
    }

    #[cfg(any())]
    #[test]
    fn parses_const_numeric_flavors() {
        assert_parses!(
            "cpu.const i64, 42",
            SurfaceInstruction::Const(SurfaceType::I64, value)
                if value == SurfaceValue::Bare(BareToken("42".to_owned()))
        );
        assert_parses!(
            "cpu.const u64, 7",
            SurfaceInstruction::Const(SurfaceType::U64, value)
                if value == SurfaceValue::Bare(BareToken("7".to_owned()))
        );
        assert_parses!(
            "cpu.const u32, 3",
            SurfaceInstruction::Const(SurfaceType::U32, value)
                if value == SurfaceValue::Bare(BareToken("3".to_owned()))
        );
        assert_parses!(
            "cpu.const f64, 3.14",
            SurfaceInstruction::Const(SurfaceType::F64, value)
                if value == SurfaceValue::Bare(BareToken("3.14".to_owned()))
        );
        assert_parses!(
            "cpu.const bool, true",
            SurfaceInstruction::Const(SurfaceType::Bool, value)
                if value == SurfaceValue::Bare(BareToken("true".to_owned()))
        );
    }

    #[cfg(any())]
    #[test]
    fn parses_const_quoted_string() {
        assert_parses!(
            "cpu.const str, \"hello world\"",
            SurfaceInstruction::Const(SurfaceType::String, SurfaceValue::Quoted(value))
                if value.as_str() == "hello world"
        );
    }

    #[test]
    fn parses_symbolic_control_flow() {
        assert_parses!(
            "cpu.br @body",
            SurfaceInstruction::Branch(target) if target.as_str() == "body"
        );
        assert_parses!(
            "cpu.cond_br @then, @else",
            SurfaceInstruction::ConditionalBranch(then_target, else_target)
                if then_target.as_str() == "then" && else_target.as_str() == "else"
        );
        assert_parses!(
            "cpu.call 2, main",
            SurfaceInstruction::Call(2, target) if target.as_str() == "main"
        );
    }

    #[test]
    fn rejects_malformed_quoted_value_instead_of_treating_it_as_bare() {
        assert!(
            SurfaceInstruction::parser()
                .parse("cpu.const str, \"unterminated")
                .has_errors()
        );
    }
}
