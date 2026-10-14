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
    let value: imported::Add = imported::Add::parser()
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
        std::marker::PhantomData::<<MachineAlias as rt::Field<rt::field!(Machine::r#type)>>::Child>,
        std::marker::PhantomData::<Cpu>,
    );
    assert_eq!(<MachineAlias as rt::Field<rt::field!(Machine::r#type)>>::SOURCE, ["type", "host"]);
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
    same(std::marker::PhantomData::<<ExternalRoot as rt::Field<Main>>::Child>, std::marker::PhantomData::<producer::Cpu>);
    assert_eq!(<ExternalRoot as rt::Field<Main>>::SOURCE, ["cpu", "host"]);
    assert_ne!(std::any::TypeId::of::<Main>(), std::any::TypeId::of::<Spare>());
}

use producer::Cpu as ImportedCpu;
pub use producer::__vihaco_fields_Cpu as __vihaco_fields_ImportedCpu;
pub use producer::__vihaco_inventory_Cpu as __vihaco_inventory_ImportedCpu;

#[rt::composite]
struct FlatMachine {
    #[device(42, alias = "host")] primary: ImportedCpu,
    #[device(43)] spare: producer::Cpu,
    total: i64,
}

impl rt::Step<rt::field!(FlatMachine::primary), imported::Add> for FlatMachine {
    fn step(&mut self, instruction: imported::Add) { self.total += instruction.0; }
}

impl rt::Step<rt::field!(FlatMachine::spare), imported::Add> for FlatMachine {
    fn step(&mut self, instruction: imported::Add) { self.total += 10 * instruction.0; }
}

impl rt::Step<rt::field!(FlatMachine::primary), imported::Halt> for FlatMachine {
    fn step(&mut self, _: imported::Halt) { self.total = -1; }
}

impl rt::Step<rt::field!(FlatMachine::spare), imported::Halt> for FlatMachine {
    fn step(&mut self, _: imported::Halt) { self.total = -2; }
}

#[test]
fn generated_flat_enum_and_dispatch_cross_crate_and_renamed_framework() {
    use rt::{Dispatch, HasInstructionSet};
    let mut machine = FlatMachine { primary: producer::Cpu, spare: producer::Cpu, total: 0 };
    machine.dispatch(__vihaco_FlatMachineInstruction::V_primary__arithmetic__Add(imported::Add(2)));
    machine.dispatch(__vihaco_FlatMachineInstruction::V_spare__arithmetic__Add(imported::Add(3)));
    assert_eq!(machine.total, 32);
    let source = <FlatMachine as HasInstructionSet>::Runtime::parser().parse("host::arithmetic::add -8").into_result().unwrap();
    assert_eq!(source, __vihaco_FlatMachineInstruction::V_primary__arithmetic__Add(imported::Add(-8)));
    assert_eq!(<__vihaco_inventory_FlatMachine::Provider as rt::__private::LeafAt<0>>::DEVICE_CODES, [42]);
}

#[rt::composite]
struct PrivateMachine {
    #[device(99)] secret: producer::PrivateDevice,
    stopped: bool,
}

impl rt::Step<rt::field!(PrivateMachine::secret), producer::PrivateStop> for PrivateMachine {
    fn step(&mut self, _: producer::PrivateStop) { self.stopped = true; }
}

#[test]
fn private_dialect_types_cross_crate_through_generated_provider_projections() {
    use rt::{Dispatch, HasInstructionSet};
    let mut machine = PrivateMachine { secret: producer::PrivateDevice, stopped: false };
    machine.dispatch(__vihaco_PrivateMachineInstruction::V_secret__private_ucontrol__Stop(producer::PrivateStop));
    assert!(machine.stopped);
    assert!(<PrivateMachine as HasInstructionSet>::Runtime::parser().parse("secret::private_control::stop").has_output());
}

use producer::Machine as ProducerMachine;
use producer::__vihaco_fields_Machine as __vihaco_fields_ProducerMachine;

impl<I> rt::Step<rt::field!(Machine::r#type), I> for Machine
where Cpu: rt::Execute<I, Message = ()> {
    fn step(&mut self, instruction: I) { assert!(rt::Execute::execute(&mut self.r#type, &instruction, ()).is_ok()); }
}

impl<I> rt::Step<rt::field!(ExternalRoot::main::cpu), I> for ExternalRoot
where producer::Machine: rt::Step<rt::field!(ProducerMachine::cpu), I>, producer::Cpu: rt::Execute<I> {
    fn step(&mut self, instruction: I) { <producer::Machine as rt::Step<rt::field!(ProducerMachine::cpu), I>>::step(&mut self.main, instruction); }
}

impl<I> rt::Step<rt::field!(ExternalRoot::spare::cpu), I> for ExternalRoot
where producer::Machine: rt::Step<rt::field!(ProducerMachine::cpu), I>, producer::Cpu: rt::Execute<I> {
    fn step(&mut self, instruction: I) { <producer::Machine as rt::Step<rt::field!(ProducerMachine::cpu), I>>::step(&mut self.spare, instruction); }
}
