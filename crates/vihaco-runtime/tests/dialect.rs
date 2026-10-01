// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]
#![allow(dead_code)]

use vihaco_runtime::__private::{Coverage, DialectSpec, InstructionSpec, require_coverage};
use vihaco_runtime::{Component, Execute, Parse, chumsky::Parser, dialect};

#[dialect]
pub mod arithmetic {
    use super::Operand;

    pub struct AddSurface;

    /// Preserved runtime documentation and derives.
    #[instruction]
    #[derive(Clone, Debug, PartialEq)]
    pub struct Add(pub i64);

    #[instruction]
    #[pattern = "'stop"]
    pub struct Halt;

    #[instruction]
    #[pattern = "'swap $1 `,` $0"]
    pub struct Swap(pub u32, pub i64);

    #[instruction]
    pub struct Local(pub Operand);

    #[derive(vihaco_runtime::Parse, Clone, Debug, PartialEq)]
    #[syntax_class(value)]
    pub struct Helper(u32);
    pub mod nested {
        pub struct Unmarked;
    }
}

pub type Operand = u32;
use arithmetic as renamed;

struct Cpu;
impl Component for Cpu {}
macro_rules! execute {
    ($($instruction:ty),*) => {$(
        impl Execute<$instruction> for Cpu {
            type Message = ();
            type Effect = ();
            type Error = std::convert::Infallible;
            fn execute(&mut self, _: &$instruction, _: ()) -> Result<(), Self::Error> { Ok(()) }
        }
    )*};
}
execute!(renamed::Add, renamed::Halt, renamed::Swap, renamed::Local);

#[test]
fn pairing_parses_values_and_retains_runtime_types() {
    let surface: <renamed::Add as InstructionSpec>::Surface = renamed::syntax::Add::parser()
        .parse("add -42")
        .into_result()
        .unwrap();
    assert_eq!(surface, renamed::syntax::Add(-42));
    assert_eq!(
        renamed::syntax::Swap::parser()
            .parse("swap -7, 3")
            .into_result()
            .unwrap(),
        renamed::syntax::Swap(3, -7)
    );
    assert_eq!(
        renamed::syntax::Halt::parser()
            .parse("stop")
            .into_result()
            .unwrap(),
        renamed::syntax::Halt
    );
    assert_eq!(
        renamed::syntax::Local::parser()
            .parse("local 5")
            .into_result()
            .unwrap(),
        renamed::syntax::Local(5)
    );
    let runtime = renamed::Add(-42);
    assert_eq!(runtime.clone(), runtime);
}

#[test]
fn description_is_complete_ordered_and_alias_independent() {
    type Identity = <renamed::Add as InstructionSpec>::Dialect;
    fn same_identity<I: InstructionSpec<Dialect = Identity>>() {}
    same_identity::<renamed::Halt>();
    require_coverage::<Cpu, Identity>();
    fn coverage<D: Coverage<Cpu>>() {}
    coverage::<Identity>();
    let leaves = Identity::LEAVES;
    assert_eq!(
        leaves
            .iter()
            .map(|leaf| (leaf.dialect, leaf.mnemonic))
            .collect::<Vec<_>>(),
        [
            ("arithmetic", "add"),
            ("arithmetic", "stop"),
            ("arithmetic", "swap"),
            ("arithmetic", "local")
        ]
    );
}

mod left {
    #[vihaco_runtime::dialect]
    pub mod shared {
        #[instruction]
        pub struct Halt;
    }
}
mod right {
    #[vihaco_runtime::dialect]
    pub mod shared {
        #[instruction]
        pub struct Halt;
    }
}

#[dialect]
mod r#type {
    #[instruction]
    pub struct Halt;
}

mod custom {
    pub use vihaco_runtime::*;
}
#[dialect]
#[vihaco(crate = self::custom)]
mod overridden {
    #[instruction]
    pub struct Halt;
}

mod nested_root {
    #[vihaco_runtime::dialect]
    #[vihaco(crate = super::custom)]
    pub mod overridden {
        #[instruction]
        pub struct Halt;
    }
}

#[dialect]
mod scoped {
    use super::{Config, Configuration};
    // These names must not be captured by generated parser bindings.
    #[allow(non_camel_case_types)]
    pub type __vihaco_field_0 = u32;
    #[allow(non_camel_case_types)]
    pub type __vihaco_pattern_for_capture = i64;
    pub type CaptureSurfaceOperand = u64;
    #[instruction]
    pub struct Capture(
        pub __vihaco_field_0,
        pub __vihaco_pattern_for_capture,
        pub CaptureSurfaceOperand,
    );
    #[instruction]
    pub struct Qualified(pub <Config as Configuration>::Operand);
    #[instruction]
    pub struct Relative(
        pub super::Operand,
        pub crate::Operand,
        pub ::std::primitive::u64,
        pub self::CaptureSurfaceOperand,
    );

    #[instruction]
    pub struct Empty();

    #[instruction]
    pub struct Private(u32);
    impl syntax::Private {
        pub fn value(&self) -> u32 {
            self.0
        }
    }
}
trait Configuration {
    type Operand;
}
struct Config;
impl Configuration for Config {
    type Operand = u32;
}

#[dialect]
mod generic_operand {
    use super::Wrapper;
    #[instruction]
    pub struct Wrapped(pub Wrapper<u32>);
}
#[derive(Clone, Debug, PartialEq)]
pub struct Wrapper<T>(pub T);
impl<'src, T: Parse<'src>> Parse<'src> for Wrapper<T> {
    fn parser() -> impl Parser<
        'src,
        &'src str,
        Self,
        vihaco_runtime::chumsky::extra::Err<vihaco_runtime::chumsky::error::Simple<'src, char>>,
    > {
        T::parser().map(Self)
    }
}

#[dialect]
#[cfg_attr(all(), allow(dead_code))]
mod gated {
    #[instruction]
    pub struct Halt;
}
#[dialect]
#[cfg(any())]
mod disabled {
    #[instruction]
    pub struct Invalid<T>(T);
}

#[test]
fn declaration_scopes_roots_and_operand_names_parse_without_capture() {
    assert_eq!(
        left::shared::syntax::Halt::parser()
            .parse("halt")
            .into_result()
            .unwrap(),
        left::shared::syntax::Halt
    );
    assert_eq!(
        right::shared::syntax::Halt::parser()
            .parse("halt")
            .into_result()
            .unwrap(),
        right::shared::syntax::Halt
    );
    assert_eq!(
        <<r#type::Halt as InstructionSpec>::Dialect as DialectSpec>::LEAVES[0].dialect,
        "type"
    );
    assert!(
        overridden::syntax::Halt::parser()
            .parse("halt")
            .has_output()
    );
    assert!(
        nested_root::overridden::syntax::Halt::parser()
            .parse("halt")
            .has_output()
    );
    assert_eq!(
        scoped::syntax::Capture::parser()
            .parse("capture 1, -2, 3")
            .into_result()
            .unwrap(),
        scoped::syntax::Capture(1, -2, 3)
    );
    assert_eq!(
        scoped::syntax::Qualified::parser()
            .parse("qualified 7")
            .into_result()
            .unwrap()
            .0,
        7
    );
    assert_eq!(
        scoped::syntax::Relative::parser()
            .parse("relative 1, 2, 3, 4")
            .into_result()
            .unwrap(),
        scoped::syntax::Relative(1, 2, 3, 4)
    );
    assert_eq!(
        generic_operand::syntax::Wrapped::parser()
            .parse("wrapped 8")
            .into_result()
            .unwrap()
            .0,
        Wrapper(8)
    );
    assert_eq!(
        scoped::syntax::Empty::parser()
            .parse("empty")
            .into_result()
            .unwrap(),
        scoped::syntax::Empty()
    );
    assert_eq!(
        scoped::syntax::Private::parser()
            .parse("private 8")
            .into_result()
            .unwrap()
            .value(),
        8
    );
    fn local() {
        #[vihaco_runtime::dialect]
        mod local {
            #[instruction]
            pub struct Halt;
        }
        assert!(local::syntax::Halt::parser().parse("halt").has_output());
    }
    local();
}

#[test]
fn invalid_dialects_have_spanned_diagnostics() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/dialect/*.rs");
}

#[dialect]
mod parameter_names {
    #[instruction]
    pub struct C;
    #[instruction]
    pub struct __VihacoComponent;
    #[instruction]
    pub struct ConstI64(pub i64);
}
execute!(
    parameter_names::C,
    parameter_names::__VihacoComponent,
    parameter_names::ConstI64
);

#[test]
fn coverage_parameters_do_not_capture_instruction_names() {
    require_coverage::<Cpu, <parameter_names::C as InstructionSpec>::Dialect>();
    let value = parameter_names::syntax::ConstI64::parser()
        .parse("consti64 -9")
        .into_result()
        .unwrap();
    assert_eq!(value.0, -9);
}

#[dialect]
mod operand_identity {
    #[instruction]
    pub struct Value(pub u32);
    impl<'src> vihaco_runtime::Parse<'src> for Value {
        fn parser() -> impl vihaco_runtime::chumsky::Parser<
            'src,
            &'src str,
            Self,
            vihaco_runtime::chumsky::extra::Err<vihaco_runtime::chumsky::error::Simple<'src, char>>,
        > {
            use vihaco_runtime::chumsky::Parser as _;
            <u32 as vihaco_runtime::Parse>::parser().map(Self)
        }
    }
    #[instruction]
    pub struct Read(pub self::Value, pub super::Operand);
}

#[test]
fn syntax_reexports_preserve_runtime_operand_identity() {
    let value: <operand_identity::Read as InstructionSpec>::Surface =
        operand_identity::syntax::Read::parser()
            .parse("read 4, 5")
            .into_result()
            .unwrap();
    assert_eq!(
        value,
        operand_identity::syntax::Read(operand_identity::Value(4), 5)
    );
    let surface_operand = operand_identity::syntax::Value(4);
    assert_eq!(surface_operand.0, value.0.0);
}
