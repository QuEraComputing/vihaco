// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A warehouse bytecode program with route-annotated print timing.

mod program;

use crate::{
    checked::{Backend, CheckedDriver, Module, Pause, StepContext},
    framework::{
        AnnotatedDriver, ChannelDriver, Describe, Receive, Route, Run, build_message_at,
        instruction_at, receive_effect_at, step_graph,
    },
    simulation::{MinExecution, SimDriver},
};
use eyre::{Result, ensure};
use program::Bytecode;
use std::collections::BTreeMap;
use vihaco_runtime::{
    BuildMessage, Execute, HasInstructionSet, NoMessage, ReusableMessageRecipe, Step, component,
    composite, dialect, field,
};
use vihaco_stdlib::{ExecutionState, Image, Loadable};

#[dialect]
#[vihaco(crate = ::vihaco_runtime)]
pub(crate) mod ops {
    #[instruction]
    pub struct PrintLabel(pub u32);
}

#[dialect]
#[vihaco(crate = ::vihaco_runtime)]
pub(crate) mod records {
    #[instruction]
    pub struct VerifyLabel(pub u32);
}

pub(crate) struct LabelPrinted {
    parcel: u32,
    barcode: String,
}

#[component]
#[vihaco(crate = ::vihaco_runtime)]
#[instructions { ops }]
#[derive(Default)]
pub(crate) struct Printer {
    submitted: usize,
}

impl Execute<ops::PrintLabel> for Printer {
    type Message = NoMessage;
    type Effect = LabelPrinted;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &ops::PrintLabel, _: NoMessage) -> Result<Self::Effect> {
        self.submitted += 1;
        let parcel = instruction.0;
        Ok(LabelPrinted {
            parcel,
            barcode: format!("BX-{parcel:04}"),
        })
    }
}

#[component]
#[vihaco(crate = ::vihaco_runtime)]
#[instructions { records }]
#[derive(Default)]
pub(crate) struct ParcelRecords {
    labels: BTreeMap<u32, String>,
    verified: Vec<u32>,
}

impl Receive<LabelPrinted> for ParcelRecords {
    type Output = ();

    fn receive(&mut self, label: LabelPrinted) -> Result<()> {
        ensure!(
            !self.labels.contains_key(&label.parcel),
            "parcel already labeled"
        );
        self.labels.insert(label.parcel, label.barcode);
        Ok(())
    }
}

impl Execute<records::VerifyLabel> for ParcelRecords {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &records::VerifyLabel, _: NoMessage) -> Result<()> {
        let parcel = instruction.0;
        ensure!(
            self.labels.contains_key(&parcel),
            "parcel {parcel} has no label"
        );
        self.verified.push(parcel);
        Ok(())
    }
}

#[composite]
#[vihaco(crate = ::vihaco_runtime)]
#[derive(Default)]
pub(crate) struct Warehouse {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState<Pause>,
    #[device(1)]
    printer: Printer,
    #[device(2)]
    parcels: ParcelRecords,
}

type Instruction = <Warehouse as HasInstructionSet>::Runtime;
type ProgramField = field!(Warehouse::execution);
type PrinterField = field!(Warehouse::printer);
type ParcelField = field!(Warehouse::parcels);

impl BuildMessage<PrinterField, ops::PrintLabel, (), NoMessage> for Warehouse {
    fn build_message(&mut self, _: &ops::PrintLabel, _: &()) -> Result<NoMessage> {
        Ok(NoMessage::new())
    }
}

impl BuildMessage<ParcelField, records::VerifyLabel, (), NoMessage> for Warehouse {
    fn build_message(&mut self, _: &records::VerifyLabel, _: &()) -> Result<NoMessage> {
        Ok(NoMessage::new())
    }
}

fn print_route<D>()
-> Route<ops::PrintLabel, impl Run<Warehouse, ops::PrintLabel, D, Output = ()> + Describe>
where
    D: AnnotatedDriver<Warehouse>,
{
    step_graph::<ops::PrintLabel>()
        .begin(build_message_at::<PrinterField>())
        // Simulation delays delivery of the printed label until 12 ticks after issue.
        .then_execute(instruction_at::<PrinterField>().annotate::<MinExecution<12>>())
        .then(receive_effect_at::<ParcelField>())
        .finish()
}

fn verify_route<D>()
-> Route<records::VerifyLabel, impl Run<Warehouse, records::VerifyLabel, D, Output = ()>> {
    step_graph::<records::VerifyLabel>()
        .begin(build_message_at::<ParcelField>())
        .then_execute(instruction_at::<ParcelField>())
        .finish()
}

impl<D: AnnotatedDriver<Self>> Step<PrinterField, ops::PrintLabel, StepContext<'_, Instruction, D>>
    for Warehouse
{
    fn step(
        &mut self,
        instruction: &ops::PrintLabel,
        context: &StepContext<'_, Instruction, D>,
    ) -> Result<()> {
        context.start(self, &print_route(), ops::PrintLabel(instruction.0))
    }
}

impl<D> Step<ParcelField, records::VerifyLabel, StepContext<'_, Instruction, D>> for Warehouse {
    fn step(
        &mut self,
        instruction: &records::VerifyLabel,
        context: &StepContext<'_, Instruction, D>,
    ) -> Result<()> {
        context.start(self, &verify_route(), records::VerifyLabel(instruction.0))
    }
}

impl<I, D, Message> Step<ProgramField, I, StepContext<'_, Instruction, D>> for Warehouse
where
    ExecutionState<Pause>: Execute<I, Message = Message, Effect = (), Error = eyre::Report>,
    Message: ReusableMessageRecipe<Self, ProgramField, I, Module<Instruction>>,
{
    fn step(&mut self, instruction: &I, context: &StepContext<'_, Instruction, D>) -> Result<()> {
        let message = Message::prepare(self, instruction, context.module)?;
        self.execution.execute(instruction, message)
    }
}

impl Backend<Warehouse> for ChannelDriver<Warehouse> {
    type Delivery = ();

    fn deliver(&mut self, machine: &mut Warehouse, (): ()) -> Result<()> {
        ensure!(self.poll_ready(machine) > 0, "no channel delivery is ready");
        Ok(())
    }
}

fn loaded_warehouse() -> Result<Loadable<Image<Instruction>, Warehouse>> {
    let module = program::load(&[
        Bytecode::PrintLabel(42),
        Bytecode::VerifyLabel(42),
        Bytecode::Halt,
    ])?;
    Ok(Loadable::new(Image::new(module), Warehouse::default()))
}

pub(super) fn demo() -> Result<()> {
    let simulated_route = print_route::<SimDriver<Warehouse>>();
    assert_eq!(std::mem::size_of_val(&simulated_route), 0);
    simulated_route.topology().print("Warehouse / Print Label");

    let mut vm = CheckedDriver::new(loaded_warehouse()?, SimDriver::starting_at(5));
    vm.execute(&[])?;
    assert_eq!(vm.machine().parcels.verified, [42]);
    assert_eq!(vm.retirements(), 3);
    assert_eq!(vm.backend().now(), 17);
    println!(
        "Warehouse verified parcel 42 at tick {}",
        vm.backend().now()
    );

    // The same bytecode completes inline with the channel driver's default timing policy.
    let mut vm = CheckedDriver::new(loaded_warehouse()?, ChannelDriver::new());
    vm.execute(&[])?;
    assert_eq!(vm.machine().parcels.verified, [42]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vihaco_stdlib::program_driver::{ProgramDriver, ProgramStop, RunCommand};

    #[test]
    fn pending_print_holds_the_pc_and_blocks_verification() -> Result<()> {
        let mut vm = CheckedDriver::new(loaded_warehouse()?, SimDriver::starting_at(5));
        vm.start((), &[])?;
        let ProgramStop::Pending { pc: 0, work } = vm.run((), RunCommand::Continue)? else {
            panic!("print did not park at PC 0");
        };
        assert_eq!(vm.machine().execution.pc(), 0);
        assert_eq!(vm.machine().printer.submitted, 1);
        assert!(vm.machine().parcels.labels.is_empty());
        assert_eq!(vm.retirements(), 0);

        assert_eq!(vm.deliver((), work, ())?, ProgramStop::Halted { pc: 2 });
        assert_eq!(vm.machine().parcels.verified, [42]);
        assert_eq!(vm.backend().now(), 17);
        Ok(())
    }

    #[test]
    fn route_failure_keeps_the_pc_and_stops_subsequent_dispatch() -> Result<()> {
        let module = program::load(&[
            Bytecode::VerifyLabel(42),
            Bytecode::PrintLabel(42),
            Bytecode::Halt,
        ])?;
        let loaded = Loadable::new(Image::new(module), Warehouse::default());
        let mut vm = CheckedDriver::new(loaded, ChannelDriver::new());
        let error = vm.execute(&[]).unwrap_err();
        assert_eq!(error.to_string(), "parcel 42 has no label");
        assert_eq!(vm.machine().execution.pc(), 0);
        assert_eq!(vm.machine().printer.submitted, 0);
        assert!(vm.run((), RunCommand::Continue).is_err());
        Ok(())
    }
}
