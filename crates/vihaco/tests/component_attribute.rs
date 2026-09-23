// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "component-attribute")]

use std::marker::PhantomData;
use vihaco::{Execute, attributes::component, dialect};

// Transitional coexistence coverage. Remove this legacy case with `component!`.
vihaco::component! {
    pub component Legacy {}
    instruction { Halt, }
}

dialect! {
    arith {
        Add,
        Sub,
        Halt,
    }
}

dialect! {
    boolean {
        Not,
        And,
        Halt,
    }
}

#[component]
#[instructions { arith::{Add, Sub}, boolean::Not, arith::Halt }]
#[derive(Default)]
pub struct Calculator {
    total: i64,
}

impl Execute<arith::Add> for Calculator {
    type Message = i64;
    type Effect = i64;
    type Error = ();

    fn execute(&mut self, _: &arith::Add, message: i64) -> Result<i64, ()> {
        self.total += message;
        Ok(self.total)
    }
}

impl Execute<arith::Sub> for Calculator {
    type Message = ();
    type Effect = ();
    type Error = ();

    fn execute(&mut self, _: &arith::Sub, _: ()) -> Result<(), ()> {
        self.total -= 1;
        Ok(())
    }
}

impl Execute<boolean::Not> for Calculator {
    type Message = bool;
    type Effect = bool;
    type Error = ();

    fn execute(&mut self, _: &boolean::Not, value: bool) -> Result<bool, ()> {
        Ok(!value)
    }
}

impl Execute<arith::Halt> for Calculator {
    type Message = ();
    type Effect = ();
    type Error = ();

    fn execute(&mut self, _: &arith::Halt, _: ()) -> Result<(), ()> {
        Ok(())
    }
}

#[test]
fn selects_instructions_and_keeps_the_struct() {
    let mut calculator = Calculator::default();
    assert_eq!(
        Execute::<arith::Add>::execute(&mut calculator, &arith::Add, 7),
        Ok(7)
    );
    assert_eq!(calculator.total, 7);
    assert_eq!(
        Execute::<boolean::Not>::execute(&mut calculator, &boolean::Not, true),
        Ok(false)
    );
}

use arith as arithmetic;

#[component]
#[instructions { arithmetic::*, boolean::{Halt, And} }]
struct Generic<'a, T, const N: usize>
where
    T: Copy + Default + 'a,
{
    values: &'a [T; N],
    marker: PhantomData<T>,
}

impl<'a, T: Copy + Default, const N: usize> Execute<arith::Add> for Generic<'a, T, N> {
    type Message = ();
    type Effect = usize;
    type Error = ();
    fn execute(&mut self, _: &arith::Add, _: ()) -> Result<usize, ()> {
        Ok(self.values.len())
    }
}

impl<'a, T: Copy + Default, const N: usize> Execute<arith::Sub> for Generic<'a, T, N> {
    type Message = ();
    type Effect = ();
    type Error = ();
    fn execute(&mut self, _: &arith::Sub, _: ()) -> Result<(), ()> {
        Ok(())
    }
}

impl<'a, T: Copy + Default, const N: usize> Execute<arith::Halt> for Generic<'a, T, N> {
    type Message = ();
    type Effect = ();
    type Error = ();
    fn execute(&mut self, _: &arith::Halt, _: ()) -> Result<(), ()> {
        Ok(())
    }
}

impl<'a, T: Copy + Default, const N: usize> Execute<boolean::Halt> for Generic<'a, T, N> {
    type Message = ();
    type Effect = ();
    type Error = ();
    fn execute(&mut self, _: &boolean::Halt, _: ()) -> Result<(), ()> {
        Ok(())
    }
}

impl<'a, T: Copy + Default, const N: usize> Execute<boolean::And> for Generic<'a, T, N> {
    type Message = bool;
    type Effect = bool;
    type Error = ();
    fn execute(&mut self, _: &boolean::And, value: bool) -> Result<bool, ()> {
        Ok(value)
    }
}

#[component]
#[instructions { crate::arith::Add, crate::arith::{Sub} }]
struct Qualified;

impl Execute<arith::Add> for Qualified {
    type Message = ();
    type Effect = ();
    type Error = ();
    fn execute(&mut self, _: &arith::Add, _: ()) -> Result<(), ()> {
        Ok(())
    }
}

impl Execute<arith::Sub> for Qualified {
    type Message = ();
    type Effect = ();
    type Error = ();
    fn execute(&mut self, _: &arith::Sub, _: ()) -> Result<(), ()> {
        Ok(())
    }
}

#[test]
fn supports_wildcards_aliases_qualified_paths_and_generics() {
    let values = [1_u8, 2];
    let mut generic = Generic {
        values: &values,
        marker: PhantomData,
    };
    assert_eq!(
        Execute::<arith::Add>::execute(&mut generic, &arith::Add, ()),
        Ok(2)
    );
    fn require_component<C: vihaco::Component>(_: &C) {}
    require_component(&generic);
    let _ = Qualified;
    fn require_legacy<C: vihaco::Component + vihaco::HasInstructionSet>() {}
    require_legacy::<legacy::Legacy>();
}
