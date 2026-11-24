// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A warehouse bytecode program with route-annotated print timing.

mod program;
mod program_driver;

use crate::framework::{
    AnnotatedDriver, ChannelDriver, Describe, Receive, Route, Run, build_message_at,
    instruction_at, receive_effect_at, step_graph,
};
use crate::simulation::{MinExecution, SimDriver};
use eyre::{Result, ensure, eyre};
use program::Program;
use program_driver::{ProgramContext, ProgramDriver, encode};
use std::collections::BTreeMap;
use vihaco_runtime::{
    BuildMessage, Execute, NoMessage, Step, component, composite, dialect, field,
};

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
    #[device(1)]
    printer: Printer,
    #[device(2)]
    parcels: ParcelRecords,
}

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

impl<D: AnnotatedDriver<Self>> Step<PrinterField, ops::PrintLabel, ProgramContext<'_, D>>
    for Warehouse
{
    fn step(
        &mut self,
        instruction: &ops::PrintLabel,
        context: &ProgramContext<'_, D>,
    ) -> Result<()> {
        context.start(self, &print_route(), ops::PrintLabel(instruction.0));
        Ok(())
    }
}

impl<D> Step<ParcelField, records::VerifyLabel, ProgramContext<'_, D>> for Warehouse {
    fn step(
        &mut self,
        instruction: &records::VerifyLabel,
        context: &ProgramContext<'_, D>,
    ) -> Result<()> {
        context.start(self, &verify_route(), records::VerifyLabel(instruction.0));
        Ok(())
    }
}

pub(crate) fn demo() -> Result<()> {
    use program::Bytecode;

    let simulated_route = print_route::<SimDriver<Warehouse>>();
    assert_eq!(std::mem::size_of_val(&simulated_route), 0);
    simulated_route.topology().print("Warehouse / Print Label");

    let bytes = encode(&[Bytecode::PrintLabel(42), Bytecode::VerifyLabel(42)])?;
    let program = Program::load::<Bytecode>(&bytes)?;

    let mut machine = Warehouse::default();
    let backend = SimDriver::new();
    backend.advance_to(&mut machine, 5)?;
    ProgramDriver::new(&program, &backend).run(&mut machine, |machine, backend| {
        // Print has issued. Verify can run after the label reaches the records.
        assert_eq!(machine.printer.submitted, 1);
        assert!(machine.parcels.labels.is_empty());
        assert_eq!(backend.advance_next(machine)?, 17);
        Ok(())
    })?;
    assert_eq!(machine.parcels.verified, [42]);
    println!("Warehouse verified parcel 42 at tick {}", backend.now());

    // The same bytecode and routes can finish inline with the channel driver.
    let mut machine = Warehouse::default();
    let backend = ChannelDriver::new();
    ProgramDriver::new(&program, &backend).run(&mut machine, |_, _| {
        Err(eyre!(
            "channel driver should complete the warehouse routes inline"
        ))
    })?;
    assert_eq!(machine.parcels.verified, [42]);
    Ok(())
}
