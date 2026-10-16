// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]
#![allow(dead_code)]

use vihaco_runtime::__private::{capabilities, require_available};
use vihaco_runtime::{Execute, component, dialect};

#[dialect]
mod control {
    #[instruction]
    pub struct Halt;
}
mod left {
    #[vihaco_runtime::dialect]
    pub mod shared {
        #[instruction]
        pub struct Add(pub i64);
    }
}
mod right {
    #[vihaco_runtime::dialect]
    pub mod shared {
        #[instruction]
        pub struct Sub(pub i64);
    }
}
use control as renamed;

trait State {}
impl State for u8 {}

/// Authored documentation, defaults, fields and bounds survive expansion.
#[component]
#[instructions { right::shared, renamed, crate::left::shared, }]
#[derive(Default)]
#[allow(clippy::multiple_bound_locations)]
struct Cpu<'a, T: self::State = u8, const N: usize = 2>
where
    T: Default,
{
    state: T,
    data: &'a [u8],
}

macro_rules! execute {
    ($instruction:ty, $message:ty, $effect:ty, $value:expr) => {
        impl<T: State + Default, const N: usize> Execute<$instruction> for Cpu<'_, T, N> {
            type Message = $message;
            type Effect = $effect;
            type Error = std::convert::Infallible;
            fn execute(
                &mut self,
                _: &$instruction,
                _: Self::Message,
            ) -> Result<Self::Effect, Self::Error> {
                Ok($value)
            }
        }
    };
}
execute!(control::Halt, (), (), ());
execute!(left::shared::Add, u8, u64, 7);
execute!(right::shared::Sub, bool, bool, true);

type Concrete<'a> = Cpu<'a>;
type Generic<'a, T, const N: usize> = Cpu<'a, T, N>;
use Cpu as Imported;

#[test]
fn complete_description_retains_order_and_type_aliases() {
    let rows: Vec<_> = capabilities::<Concrete<'_>>().collect();
    assert_eq!(
        rows.iter()
            .map(|row| (row.dialect, row.instruction))
            .collect::<Vec<_>>(),
        [("shared", "sub"), ("control", "halt"), ("shared", "add")]
    );
    assert!(rows.iter().all(|row| row.target.is_empty()));
    assert_eq!(
        capabilities::<Generic<'_, u8, 4>>().collect::<Vec<_>>(),
        rows
    );
    require_available::<Imported<'_>, control::Halt>();
    require_available::<Cpu<'_, u8, 4>, left::shared::Add>();
    let mut cpu = Cpu::<'_, u8, 2>::default();
    assert_eq!(
        Execute::<left::shared::Add>::execute(&mut cpu, &left::shared::Add(1), 2).unwrap(),
        7
    );
}

mod custom {
    pub use vihaco_runtime::*;
}
#[component]
#[vihaco(crate = self::custom)]
#[instructions { self::control }]
struct Tuple<T = u8>(T);
impl<T> Execute<control::Halt> for Tuple<T> {
    type Message = ();
    type Effect = ();
    type Error = std::convert::Infallible;
    fn execute(&mut self, _: &control::Halt, _: ()) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[component]
#[cfg(any())]
#[instructions { absent }]
struct Disabled;
#[component]
#[cfg_attr(all(), cfg(any()), derive(Default))]
#[instructions { absent }]
struct AlsoDisabled;

#[test]
fn function_local_unit_components_work_without_instruction_sets() {
    #[component]
    #[instructions { control }]
    struct Local;
    impl Execute<control::Halt> for Local {
        type Message = ();
        type Effect = ();
        type Error = std::convert::Infallible;
        fn execute(&mut self, _: &control::Halt, _: ()) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    require_available::<Local, control::Halt>();
    assert_eq!(capabilities::<Local>().next().unwrap().instruction, "halt");
    require_available::<Tuple<u64>, control::Halt>();
}

#[test]
fn invalid_components_have_structural_and_semantic_diagnostics() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/component/*.rs");
}

mod nested {
    #[vihaco_runtime::component]
    #[vihaco(crate = super::custom)]
    #[instructions { super::control }]
    #[cfg_attr(all(), derive(Default))]
    pub struct Unit;
    impl vihaco_runtime::Execute<super::control::Halt> for Unit {
        type Message = ();
        type Effect = ();
        type Error = std::convert::Infallible;
        fn execute(&mut self, _: &super::control::Halt, _: ()) -> Result<(), Self::Error> {
            Ok(())
        }
    }
}

#[component]
#[instructions { control }]
struct Blanket<T>(T);
impl<T, I> Execute<I> for Blanket<T> {
    type Message = ();
    type Effect = ();
    type Error = std::convert::Infallible;
    fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> {
        Ok(())
    }
}
#[test]
fn relative_roots_and_blanket_execution_preserve_support() {
    require_available::<nested::Unit, control::Halt>();
    require_available::<Blanket<u8>, control::Halt>();
}
