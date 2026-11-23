// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A label printer that completes on the simulation clock.

use super::simulation::{Scheduled, SimDriver};
use crate::framework::{
    Await, Describe, Receive, Route, Run, await_work, build_message_at, instruction_at,
    receive_effect_at, step_graph,
};
use eyre::{Result, ensure};
use std::collections::BTreeMap;
use vihaco_runtime::{
    BuildMessage, Execute, Field, NoMessage, component, composite, dialect, field,
};

#[dialect]
#[vihaco(crate = ::vihaco_runtime)]
pub(crate) mod ops {
    #[instruction]
    pub struct PrintLabel(pub u32);
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
    type Effect = Scheduled<LabelPrinted>;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &ops::PrintLabel, _: NoMessage) -> Result<Self::Effect> {
        self.submitted += 1;
        let parcel = instruction.0;
        Ok(Scheduled {
            tick: 12,
            result: Ok(LabelPrinted {
                parcel,
                barcode: format!("BX-{parcel:04}"),
            }),
        })
    }
}

#[derive(Default)]
struct ParcelRecords(BTreeMap<u32, String>);

impl Receive<LabelPrinted> for ParcelRecords {
    fn receive(&mut self, label: LabelPrinted) -> Result<()> {
        ensure!(
            !self.0.contains_key(&label.parcel),
            "parcel already labeled"
        );
        self.0.insert(label.parcel, label.barcode);
        Ok(())
    }
}

#[composite]
#[vihaco(crate = ::vihaco_runtime)]
#[derive(Default)]
pub(crate) struct Warehouse {
    #[device(1)]
    printer: Printer,
    parcels: ParcelRecords,
    completed_at: Option<u64>,
}

type PrinterField = field!(Warehouse::printer);

impl BuildMessage<PrinterField, ops::PrintLabel, (), NoMessage> for Warehouse {
    fn build_message(&mut self, _: &ops::PrintLabel, _: &()) -> Result<NoMessage> {
        Ok(NoMessage::new())
    }
}

struct ParcelField;
impl Field<ParcelField> for Warehouse {
    type Child = ParcelRecords;
    const SOURCE: &'static [&'static str] = &["parcels"];

    fn get(&self) -> &Self::Child {
        &self.parcels
    }

    fn get_mut(&mut self) -> &mut Self::Child {
        &mut self.parcels
    }
}

fn route<D>()
-> Route<ops::PrintLabel, impl Run<Warehouse, ops::PrintLabel, D, Output = ()> + Describe>
where
    D: Await<Warehouse, Scheduled<LabelPrinted>, Output = LabelPrinted>,
{
    step_graph::<ops::PrintLabel>()
        .begin(build_message_at::<PrinterField>())
        .then_execute(instruction_at::<PrinterField>())
        .then(await_work())
        .then(receive_effect_at::<ParcelField>())
        .finish()
}

pub(crate) fn demo() -> Result<()> {
    let route = route::<SimDriver<Warehouse>>();
    assert_eq!(std::mem::size_of_val(&route), 0);
    route.topology().print("Warehouse / Print Label");

    let mut machine = Warehouse::default();
    let driver = SimDriver::new();
    let record_completion =
        |machine: &mut Warehouse, driver: &SimDriver<Warehouse>, result: Result<()>| {
            result.expect("label route should complete");
            assert!(machine.completed_at.replace(driver.now()).is_none());
        };
    route.start_with(
        &mut machine,
        &driver,
        ops::PrintLabel(42),
        record_completion,
    );
    driver.advance_to(&mut machine, 11)?;
    assert!(machine.parcels.0.is_empty());
    assert_eq!(machine.completed_at, None);

    driver.advance_to(&mut machine, 14)?;
    assert_eq!(machine.parcels.0[&42], "BX-0042");
    assert_eq!(machine.completed_at, Some(12));

    driver.advance_to(&mut machine, 20)?;
    assert_eq!(machine.printer.submitted, 1);
    assert_eq!(machine.parcels.0.len(), 1);
    Ok(())
}
