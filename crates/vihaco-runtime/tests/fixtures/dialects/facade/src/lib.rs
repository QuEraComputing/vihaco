// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

#[cfg(test)]
use vm::{Parse, chumsky::Parser};

#[vm::dialect]
pub mod arithmetic {
    #[instruction]
    pub struct Add(pub i64);
}

mod custom {
    pub use vm::*;
}

#[vm::dialect]
#[vihaco(crate = crate::custom)]
mod custom_root {
    #[instruction]
    pub struct Halt;
}

#[test]
fn facade_has_a_complete_parser_stack() {
    let value: <arithmetic::Add as vm::__private::InstructionSpec>::Surface =
        arithmetic::syntax::Add::parser()
            .parse("add 9")
            .into_result()
            .unwrap();
    assert_eq!(value.0, 9);
    assert!(
        custom_root::syntax::Halt::parser()
            .parse("halt")
            .has_output()
    );
}

#[vm::component]
#[vihaco(crate = self::custom)]
#[instructions { crate::custom_root, self::arithmetic }]
struct Cpu<T = u8>(T);

impl<T, I> vm::Execute<I> for Cpu<T> {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;

    fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> { Ok(()) }
}

#[test]
fn facade_component_has_complete_support() {
    vm::__private::require_available::<Cpu<u8>, arithmetic::Add>();
    let rows: Vec<_> = vm::__private::capabilities::<Cpu<u8>>().collect();
    assert_eq!(rows.iter().map(|row| (row.dialect, row.instruction)).collect::<Vec<_>>(),
        [("custom_root", "halt"), ("arithmetic", "add")]);
}

#[vm::composite]
struct Machine {
    #[device(7, alias = "host")]
    r#type: Cpu<u8>,
}

type MachineAlias = Machine;

#[test]
fn composite_preserves_actual_child_and_source_identity() {
    fn same<T>(_: std::marker::PhantomData<T>, _: std::marker::PhantomData<T>) {}
    same(
        std::marker::PhantomData::<<MachineAlias as vm::Field<vm::field!(Machine::r#type)>>::Child>,
        std::marker::PhantomData::<Cpu<u8>>,
    );
    assert_eq!(<MachineAlias as vm::Field<vm::field!(Machine::r#type)>>::SOURCE, ["type", "host"]);
    let rows: Vec<_> = vm::__private::capabilities::<MachineAlias>().collect();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|row| row.target == ["type"]));
}

impl<I> vm::Step<vm::field!(Machine::r#type), I> for Machine
where Cpu<u8>: vm::Execute<I, Message = ()> {
    fn step(&mut self, instruction: I) { assert!(vm::Execute::execute(&mut self.r#type, &instruction, ()).is_ok()); }
}
