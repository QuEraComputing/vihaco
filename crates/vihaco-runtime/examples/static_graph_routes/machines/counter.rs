// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A linear control step with no effect receivers or asynchronous work.

use crate::framework::{
    Annotation, Describe, Driver, Route, Run, build_message_at, edge, instruction_at, step_graph,
};
use eyre::{Result, eyre};
use vihaco_runtime::{Dispatch, Execute, NoMessage, Step, component, composite, dialect, field};

#[dialect]
#[vihaco(crate = ::vihaco_runtime)]
pub(crate) mod ops {
    #[instruction]
    pub struct Add(pub i32);
}

#[component]
#[vihaco(crate = ::vihaco_runtime)]
#[instructions { ops }]
#[derive(Default)]
pub(crate) struct Counter(i32);
impl Execute<ops::Add> for Counter {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &ops::Add, _: NoMessage) -> Result<()> {
        self.0 = self
            .0
            .checked_add(instruction.0)
            .ok_or_else(|| eyre!("counter overflow"))?;
        Ok(())
    }
}

#[composite]
#[vihaco(crate = ::vihaco_runtime)]
#[derive(Default)]
pub(crate) struct Machine {
    #[device(1)]
    counter: Counter,
}
type CounterField = field!(Machine::counter);

struct RegisterBank;
impl Annotation for RegisterBank {
    type Value = u8;
    const VALUE: &'static u8 = &3;
}
struct ConnectionKind;
impl Annotation for ConnectionKind {
    type Value = &'static str;
    const VALUE: &'static &str = &"local register bus";
}

fn route() -> Route<ops::Add, impl Run<Machine, ops::Add, Output = ()> + Describe> {
    step_graph::<ops::Add>()
        .begin(build_message_at::<CounterField>())
        .then_execute_via(
            edge().annotate::<ConnectionKind>(),
            instruction_at::<CounterField>().annotate::<RegisterBank>(),
        )
        .finish()
}

impl Step<CounterField, ops::Add, Driver<Self>> for Machine {
    fn step(&mut self, instruction: &ops::Add, driver: &Driver<Self>) -> Result<()> {
        route().start(self, driver, ops::Add(instruction.0));
        Ok(())
    }
}

pub(crate) fn demo() -> Result<()> {
    let route = route();
    assert_eq!(std::mem::size_of_val(&route), 0);
    let topology = route.topology();
    assert_eq!(topology.nodes[1].metadata.get::<RegisterBank>(), Some(&3));
    assert_eq!(
        topology.edges[0].metadata.get::<ConnectionKind>(),
        Some(&"local register bus")
    );
    topology.print("Counter / Add");

    let mut machine = Machine::default();
    let driver = Driver::new();
    machine.dispatch(
        &__vihaco_MachineInstruction::V_counter__ops__Add(ops::Add(5)),
        &driver,
    )?;
    driver.run_until_complete(&mut machine)?;
    assert_eq!(machine.counter.0, 5);
    Ok(())
}
