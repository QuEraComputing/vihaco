// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! One reusable capture topology, with fresh signals and work on each run.

mod processing;

use crate::framework::{
    Annotation, Describe, Driver, Prepared, Receive, Route, Run, SignalInput, Wait,
    await_device_completion, await_task, build_message_at, compute, edge, fork_join,
    instruction_at, receive_effect_at, spawn_task, step_graph, wait_for_signal,
};
use eyre::{Result, eyre};
use processing::{AnalyzeCapture, CompareResults, PredictResponse, SplitCapture};
use vihaco_runtime::{BuildMessage, Execute, Field, Step, component, composite, dialect, field};

#[dialect]
#[vihaco(crate = ::vihaco_runtime)]
pub(crate) mod ops {
    #[instruction]
    pub struct Capture(pub i32);
}

pub(crate) struct Inputs {
    arm: Option<Wait<()>>,
    device: Wait<Vec<i32>>,
    expected: i64,
}
pub(crate) struct StartedCapture {
    expected: i64,
    device: Wait<Vec<i32>>,
}

#[component]
#[vihaco(crate = ::vihaco_runtime)]
#[instructions { ops }]
#[derive(Default)]
pub(crate) struct Digitizer(usize);
impl Execute<ops::Capture> for Digitizer {
    type Message = Inputs;
    type Effect = StartedCapture;
    type Error = eyre::Report;

    fn execute(&mut self, _: &ops::Capture, inputs: Inputs) -> Result<StartedCapture> {
        self.0 += 1;
        Ok(StartedCapture {
            expected: inputs.expected,
            device: inputs.device,
        })
    }
}

#[derive(Default)]
struct Results(Vec<i64>);
impl Receive<i64> for Results {
    fn receive(&mut self, value: i64) -> Result<()> {
        self.0.push(value);
        Ok(())
    }
}

#[composite]
#[vihaco(crate = ::vihaco_runtime)]
#[derive(Default)]
pub(crate) struct Machine {
    #[device(1)]
    digitizer: Digitizer,
    next_inputs: Option<(Wait<()>, Wait<Vec<i32>>)>,
    results: Results,
}
type DigitizerField = field!(Machine::digitizer);
struct ResultsField;
impl Field<ResultsField> for Machine {
    type Child = Results;
    const SOURCE: &'static [&'static str] = &["results"];
    fn get(&self) -> &Results {
        &self.results
    }
    fn get_mut(&mut self) -> &mut Results {
        &mut self.results
    }
}

impl BuildMessage<DigitizerField, ops::Capture, (), Inputs> for Machine {
    fn build_message(&mut self, instruction: &ops::Capture, _: &()) -> Result<Inputs> {
        let (arm, device) = self
            .next_inputs
            .take()
            .ok_or_else(|| eyre!("capture has no input bindings"))?;
        Ok(Inputs {
            arm: Some(arm),
            device,
            expected: i64::from(instruction.0).abs(),
        })
    }
}

struct ArmSignal;
impl SignalInput for ArmSignal {
    type Input = Prepared<ops::Capture, Inputs>;
    const NAME: &'static str = "wait for arm";

    fn take_signal(input: &mut Self::Input) -> Result<Wait<()>> {
        input
            .message
            .arm
            .take()
            .ok_or_else(|| eyre!("arm signal already taken"))
    }
}

struct CaptureResultLink;
impl Annotation for CaptureResultLink {
    type Value = &'static str;
    const VALUE: &'static &str = &"control backplane";
}
struct ResultSink;
impl Annotation for ResultSink {
    type Value = bool;
    const VALUE: &'static bool = &true;
}

fn route() -> Route<ops::Capture, impl Run<Machine, ops::Capture, Output = ()> + Describe> {
    step_graph::<ops::Capture>()
        .begin(build_message_at::<DigitizerField>())
        .then(wait_for_signal::<ArmSignal>())
        .then_execute(instruction_at::<DigitizerField>())
        .then(fork_join(
            compute::<SplitCapture>(),
            spawn_task::<PredictResponse>().then(await_task::<PredictResponse>()),
            await_device_completion()
                .then(spawn_task::<AnalyzeCapture>())
                .then(await_task::<AnalyzeCapture>()),
        ))
        .then(compute::<CompareResults>())
        .then_via(
            edge().annotate::<CaptureResultLink>(),
            receive_effect_at::<ResultsField>().annotate::<ResultSink>(),
        )
        .finish()
}

impl Step<DigitizerField, ops::Capture, Driver<Self>> for Machine {
    fn step(&mut self, instruction: &ops::Capture, driver: &Driver<Self>) -> Result<()> {
        route().start(self, driver, ops::Capture(instruction.0));
        Ok(())
    }
}

pub(crate) fn demo() -> Result<()> {
    let route = route();
    assert_eq!(std::mem::size_of_val(&route), 0);
    let topology = route.topology();
    assert_eq!(topology.nodes.len(), 12);
    assert_eq!(topology.edges.len(), 12);
    assert_eq!(
        topology.edges[11].metadata.get::<CaptureResultLink>(),
        Some(&"control backplane")
    );
    assert_eq!(topology.nodes[11].metadata.get::<ResultSink>(), Some(&true));
    topology.print("Digitizer / Capture");

    let mut machine = Machine::default();
    let driver = Driver::new();
    for (index, (expected, measured)) in [(8, 10), (30, 27)].into_iter().enumerate() {
        let (arm, armed) = driver.signal();
        let (device, finished) = driver.signal();
        machine.next_inputs = Some((armed, finished));
        route.start(&mut machine, &driver, ops::Capture(expected));
        assert_eq!(machine.digitizer.0, index);
        assert_eq!(driver.poll_ready(&mut machine), 0);

        arm.complete(Ok(()));
        assert!(driver.poll_ready(&mut machine) > 0);
        assert_eq!(machine.digitizer.0, index + 1);
        assert_eq!(machine.results.0.len(), index);

        device.complete(Ok(vec![-measured, 1]));
        driver.run_until_complete(&mut machine)?;
        assert_eq!(machine.digitizer.0, index + 1);
    }
    assert_eq!(machine.results.0, [2, -3]);
    Ok(())
}
