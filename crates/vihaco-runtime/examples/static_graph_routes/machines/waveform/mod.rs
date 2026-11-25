// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A waveform route with different metadata on the two delivery paths.

mod metadata;

use crate::framework::{
    ChannelDriver, Describe, Route, Run, apply_effect_at, build_message_at, duplicate, edge,
    fork_join, instruction_at, step_graph,
};
use eyre::Result;
use metadata::{Bus, Critical, DacBus};
use std::convert::Infallible;
use vihaco_runtime::{
    Apply, BuildMessage, Execute, Field, NoMessage, component, composite, dialect, field,
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
impl Apply<Vec<i32>> for Sink {
    type Effect = ();
    type Error = Infallible;

    fn apply(&mut self, samples: Vec<i32>) -> Result<(), Self::Error> {
        self.0.push(samples);
        Ok(())
    }
}

#[derive(Default)]
struct Clamp;

impl Apply<Vec<i32>> for Clamp {
    type Effect = Vec<i32>;
    type Error = Infallible;

    fn apply(&mut self, mut samples: Vec<i32>) -> Result<Vec<i32>, Self::Error> {
        for sample in &mut samples {
            *sample = (*sample).clamp(-4, 4);
        }
        Ok(samples)
    }
}

#[derive(Default)]
struct DeliveryLog(usize);

impl Apply<((), ())> for DeliveryLog {
    type Effect = ();
    type Error = Infallible;

    fn apply(&mut self, _: ((), ())) -> Result<(), Self::Error> {
        self.0 += 1;
        Ok(())
    }
}

#[composite]
#[vihaco(crate = ::vihaco_runtime)]
#[derive(Default)]
pub(crate) struct Machine {
    #[device(1)]
    source: Source,
    clamp: Clamp,
    dac: Sink,
    monitor: Sink,
    deliveries: DeliveryLog,
}
type SourceField = field!(Machine::source);
struct ClampField;
struct Dac;
struct Monitor;
struct DeliveryField;

impl Field<ClampField> for Machine {
    type Child = Clamp;
    const SOURCE: &'static [&'static str] = &["clamp"];
    fn get(&self) -> &Clamp {
        &self.clamp
    }
    fn get_mut(&mut self) -> &mut Clamp {
        &mut self.clamp
    }
}
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

impl Field<DeliveryField> for Machine {
    type Child = DeliveryLog;
    const SOURCE: &'static [&'static str] = &["deliveries"];
    fn get(&self) -> &DeliveryLog {
        &self.deliveries
    }
    fn get_mut(&mut self) -> &mut DeliveryLog {
        &mut self.deliveries
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
            duplicate(),
            // The DAC gets the clamped copy. The monitor gets the original.
            apply_effect_at::<ClampField>().then_via(
                edge().annotate::<DacBus>().annotate::<Critical>(),
                apply_effect_at::<Dac>(),
            ),
            apply_effect_at::<Monitor>(),
        ))
        // The join releases this node after both receivers have accepted their samples.
        .then(apply_effect_at::<DeliveryField>())
        .finish()
}

pub(crate) fn demo() -> Result<()> {
    let route = route();
    assert_eq!(std::mem::size_of_val(&route), 0);
    let topology = route.topology();
    // Inspect the transfer before constructing the machine or running the route.
    let dac = topology
        .edges
        .iter()
        .find(|link| link.metadata.get::<DacBus>().is_some())
        .expect("the DAC transfer is annotated");
    assert_eq!(
        topology.nodes[dac.from].field,
        Some(std::any::type_name::<ClampField>())
    );
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
    assert_eq!(machine.deliveries.0, 1);
    Ok(())
}
