// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A warehouse SST program with route-annotated print timing.

mod program;

use crate::{
    framework::{
        AnnotatedDriver, Describe, Route, Run, apply_effect_at, execute_instruction_at,
        prepare_message_at, step_graph,
    },
    scheduled::{
        backends::{
            sim::{Sim, SimSubmitter},
            threaded::TokioWorker,
        },
        driver::{Module, Pause, RouteDriver, StepContext},
        runtime::Runtime,
        tasks::{Submit, TimedSubmit},
    },
    timing::MinExecution,
};
use eyre::{Result, ensure};
use std::collections::BTreeMap;
use vihaco_runtime::{
    Apply, Execute, HasInstructionSet, HasProgram, NoMessage, ReusableMessageRecipe, Step,
    component, composite, dialect, field,
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
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ParcelRecords {
    labels: BTreeMap<u32, String>,
    verified: Vec<u32>,
}

impl Apply<LabelPrinted> for ParcelRecords {
    type Effect = ();
    type Error = eyre::Report;

    fn apply(&mut self, label: LabelPrinted) -> Result<()> {
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

fn print_route<D>()
-> Route<ops::PrintLabel, impl Run<Warehouse, ops::PrintLabel, D, Output = ()> + Describe>
where
    D: AnnotatedDriver<Warehouse>,
{
    step_graph::<ops::PrintLabel>()
        .begin(prepare_message_at::<PrinterField, NoMessage>())
        // Each backend delivers the label after its interpretation of 12 ticks.
        .then(execute_instruction_at::<PrinterField>().annotate::<MinExecution<12>>())
        .then(apply_effect_at::<ParcelField>())
        .finish()
}

fn verify_route<D>()
-> Route<records::VerifyLabel, impl Run<Warehouse, records::VerifyLabel, D, Output = ()>> {
    step_graph::<records::VerifyLabel>()
        .begin(prepare_message_at::<ParcelField, NoMessage>())
        .then(execute_instruction_at::<ParcelField>())
        .finish()
}

impl<B: TimedSubmit> Step<PrinterField, ops::PrintLabel, StepContext<'_, Self, B>> for Warehouse {
    fn step(
        &mut self,
        instruction: &ops::PrintLabel,
        context: &StepContext<'_, Self, B>,
    ) -> Result<()> {
        context.start(self, &print_route(), ops::PrintLabel(instruction.0))
    }
}

impl<B: Submit> Step<ParcelField, records::VerifyLabel, StepContext<'_, Self, B>> for Warehouse {
    fn step(
        &mut self,
        instruction: &records::VerifyLabel,
        context: &StepContext<'_, Self, B>,
    ) -> Result<()> {
        context.start(self, &verify_route(), records::VerifyLabel(instruction.0))
    }
}

impl<I, B, Message> Step<ProgramField, I, StepContext<'_, Self, B>> for Warehouse
where
    ExecutionState<Pause>: Execute<I, Message = Message, Effect = (), Error = eyre::Report>,
    Message: ReusableMessageRecipe<Self, ProgramField, I, Module<Self>>,
{
    fn step(&mut self, instruction: &I, context: &StepContext<'_, Self, B>) -> Result<()> {
        let message = Message::prepare(self, instruction, context.module)?;
        self.execution.execute(instruction, message)
    }
}

fn new_warehouse(image: Image<Instruction>) -> Loadable<Image<Instruction>, Warehouse> {
    Loadable::new(image, Warehouse::default())
}

pub(super) fn demo() -> Result<()> {
    let image = program::load()?;
    let route = print_route::<RouteDriver<Warehouse, SimSubmitter>>();
    assert_eq!(std::mem::size_of_val(&route), 0);
    route.topology().print("Warehouse / Print Label");

    // Run the same image with two fresh machines and two timing policies.
    let mut simulated = Runtime::new(Sim::starting_at(5));
    let simulated_id = simulated.execute(new_warehouse(image.clone()), vec![])?;
    let mut realtime = Runtime::new(TokioWorker::new()?);
    let realtime_id = realtime.execute(new_warehouse(image), vec![])?;

    let simulated_machine = simulated.machine(simulated_id);
    let realtime_machine = realtime.machine(realtime_id);
    assert_eq!(simulated_machine.parcels.verified, [42]);
    assert_eq!(
        simulated_machine.parcels.labels,
        BTreeMap::from([(42, "BX-0042".into())])
    );
    assert_eq!(realtime_machine.parcels, simulated_machine.parcels);
    assert_eq!(simulated_machine.printer.submitted, 1);
    assert_eq!(realtime_machine.printer.submitted, 1);
    assert!(simulated_machine.program().is_halted() && realtime_machine.program().is_halted());
    assert_eq!(simulated_machine.program().pc(), 2);
    assert_eq!(
        realtime_machine.program().pc(),
        simulated_machine.program().pc()
    );
    assert_eq!(simulated.backend().now(), 17);
    println!("Warehouse verified parcel 42 at tick 17 and on Tokio");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduled::backends::{Drive, threaded::TokioSubmitter};
    use crate::timing::minimum_execution;
    use vihaco_stdlib::RunState;

    fn print_parks_then_resumes<D: Drive>(backend: D) -> Result<Runtime<Warehouse, D>>
    where
        D::Submitter: TimedSubmit,
    {
        let mut runtime = Runtime::new(backend);
        let id = runtime.load(new_warehouse(program::load()?));
        runtime.schedule_entry(id, vec![])?;

        runtime.advance_one()?; // Enter at PC 0.
        runtime.advance_one()?; // Issue the print and park.
        let machine = runtime.machine(id);
        assert!(matches!(
            machine.program().run_state(),
            RunState::Other(Pause::Route)
        ));
        assert_eq!(machine.program().pc(), 0);
        assert_eq!(machine.printer.submitted, 1);
        assert!(machine.parcels.labels.is_empty());

        runtime.advance_one()?; // Deliver the label without fetching verification yet.
        let machine = runtime.machine(id);
        assert_eq!(machine.program().pc(), 1);
        assert!(machine.parcels.labels.contains_key(&42));
        assert!(machine.parcels.verified.is_empty());

        assert_eq!(runtime.run()?, [id]);
        assert_eq!(runtime.machine(id).parcels.verified, [42]);
        assert_eq!(runtime.machine(id).printer.submitted, 1);
        Ok(runtime)
    }

    #[test]
    fn simulated_delivery_is_a_separate_runtime_turn() -> Result<()> {
        let runtime = print_parks_then_resumes(Sim::starting_at(5))?;
        assert_eq!(runtime.backend().now(), 17);
        Ok(())
    }

    #[test]
    fn two_warehouses_share_one_simulation_timeline() -> Result<()> {
        let image = program::load()?;
        let mut runtime = Runtime::new(Sim::starting_at(5));
        let first = runtime.load(new_warehouse(image.clone()));
        let second = runtime.load(new_warehouse(image));
        runtime.schedule_entry(first, vec![])?;
        runtime.schedule_entry(second, vec![])?;

        assert_eq!(runtime.run()?, [first, second]);
        assert_eq!(runtime.backend().now(), 17);
        for id in [first, second] {
            assert_eq!(runtime.machine(id).parcels.verified, [42]);
            assert_eq!(runtime.machine(id).printer.submitted, 1);
        }
        Ok(())
    }

    #[test]
    fn tokio_uses_the_same_topology_and_resumes_the_parked_print() -> Result<()> {
        let simulated = print_route::<RouteDriver<Warehouse, SimSubmitter>>().topology();
        let realtime = print_route::<RouteDriver<Warehouse, TokioSubmitter>>().topology();
        assert!(simulated.nodes.iter().zip(&realtime.nodes).all(|(a, b)| (
            a.name,
            a.field,
            a.source_field,
            minimum_execution(&a.metadata)
        ) == (
            b.name,
            b.field,
            b.source_field,
            minimum_execution(&b.metadata)
        )));
        assert_eq!(simulated.nodes.len(), 3);
        assert_eq!(simulated.nodes[2].source_field, None);
        assert_eq!(
            simulated.nodes[2].field,
            Some(std::any::type_name::<ParcelField>())
        );
        assert_eq!(simulated.nodes.len(), realtime.nodes.len());
        assert!(
            simulated
                .edges
                .iter()
                .map(|edge| (edge.from, edge.to))
                .eq(realtime.edges.iter().map(|edge| (edge.from, edge.to)))
        );

        print_parks_then_resumes(TokioWorker::new()?)?;
        Ok(())
    }

    #[test]
    fn route_failure_keeps_the_pc_and_stops_subsequent_dispatch() -> Result<()> {
        let image = program::load_failure("verify-first")?;
        let mut runtime = Runtime::new(TokioWorker::new()?);
        let id = runtime.load(new_warehouse(image));
        runtime.schedule_entry(id, vec![])?;
        let error = runtime.run().unwrap_err();
        assert_eq!(error.to_string(), "parcel 42 has no label");
        assert_eq!(runtime.machine(id).program().pc(), 0);
        assert_eq!(runtime.machine(id).printer.submitted, 0);
        assert!(matches!(
            runtime.machine(id).program().run_state(),
            RunState::Other(Pause::Faulted)
        ));
        Ok(())
    }

    #[test]
    fn deferred_application_error_keeps_the_prior_effect_and_faults_at_the_origin() -> Result<()> {
        let image = program::load_failure("duplicate-print")?;
        let mut runtime = Runtime::new(Sim::new());
        let id = runtime.load(new_warehouse(image));
        runtime.schedule_entry(id, vec![])?;
        let error = runtime.run().unwrap_err();
        assert_eq!(error.to_string(), "parcel already labeled");
        assert_eq!(runtime.machine(id).program().pc(), 1);
        assert_eq!(runtime.machine(id).printer.submitted, 2);
        assert_eq!(
            runtime
                .machine(id)
                .parcels
                .labels
                .get(&42)
                .map(String::as_str),
            Some("BX-0042")
        );
        assert!(runtime.machine(id).parcels.verified.is_empty());
        Ok(())
    }
}
