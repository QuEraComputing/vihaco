// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

use producer::exported as imported;
#[cfg(test)]
use rt::__private::{DialectSpec, InstructionSpec, require_coverage};
#[cfg(test)]
use rt::{Parse, chumsky::Parser};
#[rt::component]
#[instructions { ::producer::exported }]
struct Cpu;
macro_rules! execute {
    ($($instruction:ty),*) => {$(
        impl rt::Execute<$instruction> for Cpu {
            type Message = (); type Effect = (); type Error = std::convert::Infallible;
            fn execute(&mut self, _: &$instruction, _: ()) -> Result<(), Self::Error> { Ok(()) }
        }
    )*};
}
execute!(imported::Add, imported::Halt);
#[test]
fn cross_crate_imports_preserve_identity_and_parser() {
    type Identity = <imported::Add as InstructionSpec>::Dialect;
    fn same<I: InstructionSpec<Dialect = Identity>>() {}
    same::<producer::arithmetic::Halt>();
    require_coverage::<Cpu, Identity>();
    assert_eq!(Identity::LEAVES[0].dialect, "arithmetic");
    assert_eq!(Identity::LEAVES[1].mnemonic, "halt");
    rt::__private::require_available::<Cpu, imported::Halt>();
    assert_eq!(rt::__private::capabilities::<Cpu>().count(), 2);
    let value: <imported::Add as InstructionSpec>::Surface = imported::syntax::Add::parser()
        .parse("add -8")
        .into_result()
        .unwrap();
    assert_eq!(value.0, -8);
}

#[rt::composite]
struct Machine {
    #[device(7, alias = "host")]
    r#type: Cpu,
}
type MachineAlias = Machine;
#[test]
fn composite_preserves_actual_child_and_source_identity() {
    fn same<T>(_: std::marker::PhantomData<T>, _: std::marker::PhantomData<T>) {}
    same(
        std::marker::PhantomData::<<MachineAlias as rt::__private::Field<rt::field!(Machine::r#type)>>::Child>,
        std::marker::PhantomData::<Cpu>,
    );
    assert_eq!(<MachineAlias as rt::__private::Field<rt::field!(Machine::r#type)>>::SOURCE, ["type", "host"]);
    let rows: Vec<_> = rt::__private::capabilities::<MachineAlias>().collect();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|row| row.target == ["type"]));
}

#[rt::composite]
struct ExternalRoot {
    #[device(1)] main: producer::Machine,
    #[device(2)] spare: producer::Machine,
}
#[test]
fn markers_resolve_across_crates() {
    type Main = rt::field!(ExternalRoot::main::cpu);
    type Spare = rt::field!(ExternalRoot::spare::cpu);
    fn same<T>(_: std::marker::PhantomData<T>, _: std::marker::PhantomData<T>) {}
    same(std::marker::PhantomData::<<ExternalRoot as rt::__private::Field<Main>>::Child>, std::marker::PhantomData::<producer::Cpu>);
    assert_eq!(<ExternalRoot as rt::__private::Field<Main>>::SOURCE, ["cpu", "host"]);
    assert_ne!(std::any::TypeId::of::<Main>(), std::any::TypeId::of::<Spare>());
}
