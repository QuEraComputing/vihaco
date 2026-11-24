// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A waveform route with different metadata on the two delivery paths.

mod metadata;

use crate::framework::{
    ChannelDriver, Compute, Describe, Receive, Route, Run, build_message_at, compute, edge,
    fork_join, instruction_at, receive_effect_at, step_graph,
};
use eyre::{Result, ensure};
use metadata::{Bus, Critical, DacBus};
use vihaco_runtime::{
    BuildMessage, Execute, Field, NoMessage, component, composite, dialect, field,
};

#[dialect]
#[vihaco(crate = ::vihaco_runtime)]
pub(crate) mod ops {
    #[instruction]
    pub struct Fire(pub i32);
}

#[component]
#[vihaco(crate = ::vihaco_runtime)]
#[instructions { ops }]
#[derive(Default)]
pub(crate) struct Source;
impl Execute<ops::Fire> for Source {
    type Message = NoMessage;
    type Effect = Vec<i32>;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &ops::Fire, _: NoMessage) -> Result<Vec<i32>> {
        Ok(vec![instruction.0.saturating_neg(), 0, instruction.0])
    }
}

#[derive(Default)]
struct Sink(Vec<Vec<i32>>);
impl Receive<Vec<i32>> for Sink {
    fn receive(&mut self, samples: Vec<i32>) -> Result<()> {
        self.0.push(samples);
        Ok(())
    }
}

#[composite]
#[vihaco(crate = ::vihaco_runtime)]
#[derive(Default)]
pub(crate) struct Machine {
    #[device(1)]
    source: Source,
    dac: Sink,
    monitor: Sink,
    delivered: usize,
}
type SourceField = field!(Machine::source);
struct Dac;
struct Monitor;
impl Field<Dac> for Machine {
    type Child = Sink;
    const SOURCE: &'static [&'static str] = &["dac"];
    fn get(&self) -> &Sink {
        &self.dac
    }
    fn get_mut(&mut self) -> &mut Sink {
        &mut self.dac
    }
}
impl Field<Monitor> for Machine {
    type Child = Sink;
    const SOURCE: &'static [&'static str] = &["monitor"];
    fn get(&self) -> &Sink {
        &self.monitor
    }
    fn get_mut(&mut self) -> &mut Sink {
        &mut self.monitor
    }
}

struct Duplicate;
impl Compute for Duplicate {
    type Machine = Machine;
    type Input = Vec<i32>;
    type Output = (Vec<i32>, Vec<i32>);
    const NAME: &'static str = "duplicate waveform";
    fn apply(_: &mut Machine, samples: Vec<i32>) -> Result<Self::Output> {
        Ok((samples.clone(), samples))
    }
}

struct Clamp<const LIMIT: i32>;
impl<const LIMIT: i32> Compute for Clamp<LIMIT> {
    type Machine = Machine;
    type Input = Vec<i32>;
    type Output = Vec<i32>;
    const NAME: &'static str = "clamp waveform";
    fn apply(_: &mut Machine, mut samples: Vec<i32>) -> Result<Vec<i32>> {
        ensure!(LIMIT >= 0, "clamp limit must be nonnegative");
        for sample in &mut samples {
            *sample = (*sample).clamp(-LIMIT, LIMIT);
        }
        Ok(samples)
    }
}

struct RecordDelivery;
impl Compute for RecordDelivery {
    type Machine = Machine;
    type Input = ((), ());
    type Output = ();
    const NAME: &'static str = "record delivery";
    fn apply(machine: &mut Machine, _: ((), ())) -> Result<()> {
        machine.delivered += 1;
        Ok(())
    }
}

impl BuildMessage<SourceField, ops::Fire, (), NoMessage> for Machine {
    fn build_message(&mut self, _: &ops::Fire, _: &()) -> Result<NoMessage> {
        Ok(NoMessage::new())
    }
}

fn route()
-> Route<ops::Fire, impl Run<Machine, ops::Fire, ChannelDriver<Machine>, Output = ()> + Describe> {
    step_graph::<ops::Fire>()
        .begin(build_message_at::<SourceField>())
        .then_execute(instruction_at::<SourceField>())
        .then(fork_join(
            compute::<Duplicate>(),
            // The DAC gets the clamped copy. The monitor gets the original.
            compute::<Clamp<4>>().then_via(
                edge().annotate::<DacBus>().annotate::<Critical>(),
                receive_effect_at::<Dac>(),
            ),
            receive_effect_at::<Monitor>(),
        ))
        // The join releases this node after both receivers have accepted their samples.
        .then(compute::<RecordDelivery>())
        .finish()
}

pub(crate) fn demo() -> Result<()> {
    let route = route();
    assert_eq!(std::mem::size_of_val(&route), 0);
    let topology = route.topology();
    // Inspect the transfer before constructing the machine or running the route.
    let dac = &topology.edges[3];
    assert_eq!(topology.nodes[dac.from].name, "clamp waveform");
    assert_eq!(
        topology.nodes[dac.to].field,
        Some(std::any::type_name::<Dac>())
    );
    assert_eq!(
        dac.metadata.get::<DacBus>(),
        Some(&Bus {
            name: "pulse",
            lanes: 4
        })
    );
    assert_eq!(dac.metadata.get::<Critical>(), Some(&true));
    topology.print("Waveform / Fire");

    let mut machine = Machine::default();
    let driver = ChannelDriver::new();
    driver.start(&mut machine, &route, ops::Fire(10));
    driver.run_until_complete(&mut machine)?;
    assert_eq!(machine.dac.0, [[-4, 0, 4]]);
    assert_eq!(machine.monitor.0, [[-10, 0, 10]]);
    assert_eq!(machine.delivered, 1);
    Ok(())
}
