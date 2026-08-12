// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::data::Heap;
use crate::instruction::cpu::runtime::instruction;
use vihaco::frame::Frame;
use vihaco::program::Value;
use vihaco_parser::{BareToken, Ident, QuotedString};

vihaco::component! {
    #[module = cpu]
    pub component CPU {
        pub(crate) frames: Vec<Frame>,
        pub(crate) heap: Heap,
        pub(crate) stack: Vec<Value>,
        pub(crate) span: (u32, u32, u32),
        pub(crate) pending_pc: Option<u32>,
        pub(crate) current_pc: u32,
        pub(crate) return_values: Vec<Value>,
    }

    runtime {
        type Type = vihaco::Type;
        value Value = vihaco::Value;

        instruction {
            Span(u32, u32, u32),
            Label,
            FunctionStart,
            FunctionEnd,
            Breakpoint,
            Branch(u32),
            ConditionalBranch(u32, u32),
            Return(u32),
            IndirectCall,
            Call(u32, u32),
            Halt,
            Print,
            Load(Type, u32),
            Store(Type, u32),
            Dup,
            HeapAlloc(u32),
            GetItem,
            HeapDealloc,
            Const(Value),
            Add(Type),
            Sub(Type),
            Mul(Type),
            Div(Type),
            Rem(Type),
            Neg(Type),
            Shl(Type),
            Shr(Type),
            Rol(Type),
            Ror(Type),
            BitAnd(Type),
            BitOr(Type),
            BitXor(Type),
            Not,
            And,
            Or,
            Xor,
            Eq(Type),
            Ne(Type),
            Lt(Type),
            Gt(Type),
            Le(Type),
            Ge(Type),
        }
    }

    syntax {
        type SurfaceType {
            Undefined = "`undef`";
            String = "`str`";
            Bool = "`bool`";
            I64 = "`i64`";
            U32 = "`u32`";
            U64 = "`u64`";
            F64 = "`f64`";
            FunctionRef = "`fn_ref`";
            HeapRef = "`heap_ref`";
        }

        value SurfaceValue {
            Quoted(QuotedString) = "$0";
            Bare(BareToken) = "$0";
        }

        instruction {
            Span(u32, u32, u32) = "'cpu::span $0 $1 $2";
            Label(Ident) = "'cpu::label `@` $0";
            FunctionStart = "'cpu::func_start";
            FunctionEnd = "'cpu::func_end";
            Breakpoint = "'cpu::breakpoint";
            Branch(Ident) = "'cpu::br `@` $0";
            ConditionalBranch(Ident, Ident) = "'cpu::cond_br `@` $0 `,` `@` $1";
            Return = "'cpu::ret";
            IndirectCall = "'cpu::call_indirect";
            Call(u32, Ident) = "'cpu::call $0 `,` $1";
            Halt = "'cpu::halt";
            Print = "'cpu::print";
            Load(SurfaceType, u32) = "'cpu::load $0 `,` $1";
            Store(SurfaceType, u32) = "'cpu::store $0 `,` $1";
            Dup = "'cpu::dup";
            HeapAlloc(u32) = "'cpu::heap_alloc $0";
            GetItem = "'cpu::get_item";
            HeapDealloc = "'cpu::heap_dealloc";
            Const(SurfaceType, SurfaceValue) = "'cpu::const $0 `,` $1";
            Add(SurfaceType) = "'cpu::add $0";
            Sub(SurfaceType) = "'cpu::sub $0";
            Mul(SurfaceType) = "'cpu::mul $0";
            Div(SurfaceType) = "'cpu::div $0";
            Rem(SurfaceType) = "'cpu::rem $0";
            Neg(SurfaceType) = "'cpu::neg $0";
            Shl(SurfaceType) = "'cpu::shl $0";
            Shr(SurfaceType) = "'cpu::shr $0";
            Rol(SurfaceType) = "'cpu::rol $0";
            Ror(SurfaceType) = "'cpu::ror $0";
            BitAnd(SurfaceType) = "'cpu::bitand $0";
            BitOr(SurfaceType) = "'cpu::bitor $0";
            BitXor(SurfaceType) = "'cpu::bitxor $0";
            Not = "'cpu::not";
            And = "'cpu::and";
            Or = "'cpu::or";
            Xor = "'cpu::xor";
            Eq(SurfaceType) = "'cpu::eq $0";
            Ne(SurfaceType) = "'cpu::ne $0";
            Lt(SurfaceType) = "'cpu::lt $0";
            Gt(SurfaceType) = "'cpu::gt $0";
            Le(SurfaceType) = "'cpu::le $0";
            Ge(SurfaceType) = "'cpu::ge $0";
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for cpu::CPU {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            heap: Heap::default(),
            stack: Vec::new(),
            span: (0, 0, 0),
            pending_pc: None,
            current_pc: 0,
            return_values: Vec::new(),
        }
    }
}

impl Clone for cpu::CPU {
    fn clone(&self) -> Self {
        Self {
            frames: self.frames.clone(),
            heap: self.heap.clone(),
            stack: self.stack.clone(),
            span: self.span,
            pending_pc: self.pending_pc,
            current_pc: self.current_pc,
            return_values: self.return_values.clone(),
        }
    }
}

impl std::fmt::Debug for cpu::CPU {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CPU")
            .field("frames", &self.frames)
            .field("heap", &self.heap)
            .field("stack", &self.stack)
            .field("span", &self.span)
            .field("pending_pc", &self.pending_pc)
            .field("current_pc", &self.current_pc)
            .field("return_values", &self.return_values)
            .finish()
    }
}

pub use cpu::syntax::{Instruction as SurfaceInstruction, SurfaceType, SurfaceValue};

impl<T: Into<Value>> From<T> for instruction::Const {
    fn from(value: T) -> Self {
        instruction::Const(value.into())
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
        assert!(SurfaceInstruction::parser().parse("br @body").has_errors());
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
