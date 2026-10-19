// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

#[rt::dialect]
mod local {
    #[instruction]
    pub struct Halt;
}

#[rt::component]
#[instructions { local }]
struct Cpu;

impl rt::Execute<local::Halt> for Cpu {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;

    fn execute(&mut self, _: &local::Halt, _: ()) -> Result<(), Self::Error> { Ok(()) }
}

#[test]
fn indirect_feature_enables_complete_contract() {
    use rt::{Parse, chumsky::Parser};
    assert!(local::Halt::parser().parse("halt").has_output());
    rt::__private::require_available::<Cpu, local::Halt>();
    assert_eq!(
        <producer::exported::Add as rt::__private::InstructionSpec>::MNEMONIC,
        "add"
    );
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
        std::marker::PhantomData::<<MachineAlias as rt::Field<rt::field!(Machine::r#type)>>::Child>,
        std::marker::PhantomData::<Cpu>,
    );
    assert_eq!(<MachineAlias as rt::Field<rt::field!(Machine::r#type)>>::SOURCE, ["type", "host"]);
    let rows: Vec<_> = rt::__private::capabilities::<MachineAlias>().collect();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|row| row.target == ["type"]));
}

impl<I> rt::Step<rt::field!(Machine::r#type), I> for Machine
where Cpu: rt::Execute<I, Message = (), Error = std::convert::Infallible> {
    fn step(&mut self, instruction: &I, _: &()) -> rt::eyre::Result<()> { rt::Execute::execute(&mut self.r#type, instruction, ())?; Ok(()) }
}
