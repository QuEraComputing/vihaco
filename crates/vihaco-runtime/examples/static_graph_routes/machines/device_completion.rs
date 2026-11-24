// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A synchronous instruction returns a handle that the route waits on.

use crate::framework::{
    ChannelDriver, Compute, Describe, Route, Run, Wait, await_work, build_message_at, compute,
    instruction_at, step_graph,
};
use eyre::{Result, eyre};
use vihaco_runtime::{BuildMessage, Execute, component, composite, dialect, field};

#[dialect]
#[vihaco(crate = ::vihaco_runtime)]
pub(crate) mod ops {
    #[instruction]
    pub struct Capture;
}

#[component]
#[vihaco(crate = ::vihaco_runtime)]
#[instructions { ops }]
#[derive(Default)]
pub(crate) struct Digitizer {
    issued: usize,
}

impl Execute<ops::Capture> for Digitizer {
    type Message = Wait<u16>;
    type Effect = Wait<u16>;
    type Error = eyre::Report;

    fn execute(&mut self, _: &ops::Capture, completion: Wait<u16>) -> Result<Wait<u16>> {
        // The fixture supplies the handle for the acquisition we start here.
        self.issued += 1;
        Ok(completion)
    }
}

#[composite]
#[vihaco(crate = ::vihaco_runtime)]
#[derive(Default)]
pub(crate) struct Machine {
    #[device(1)]
    digitizer: Digitizer,
    next_completion: Option<Wait<u16>>,
    sample: Option<u16>,
}

type DigitizerField = field!(Machine::digitizer);

impl BuildMessage<DigitizerField, ops::Capture, (), Wait<u16>> for Machine {
    fn build_message(&mut self, _: &ops::Capture, _: &()) -> Result<Wait<u16>> {
        self.next_completion
            .take()
            .ok_or_else(|| eyre!("capture has no completion handle"))
    }
}

struct StoreSample;

impl Compute for StoreSample {
    type Machine = Machine;
    type Input = u16;
    type Output = ();
    const NAME: &'static str = "store sample";

    fn apply(machine: &mut Machine, sample: u16) -> Result<()> {
        machine.sample = Some(sample);
        Ok(())
    }
}

fn route() -> Route<
    ops::Capture,
    impl Run<Machine, ops::Capture, ChannelDriver<Machine>, Output = ()> + Describe,
> {
    // The completion path parks on the handle returned by Execute.
    let completion = await_work().then(compute::<StoreSample>());
    step_graph::<ops::Capture>()
        .begin(build_message_at::<DigitizerField>())
        .then_execute(instruction_at::<DigitizerField>())
        .then(completion)
        .finish()
}

pub(crate) fn demo() -> Result<()> {
    let route = route();
    route.topology().print("Digitizer / Device Completion");

    let driver = ChannelDriver::new();
    let mut machine = Machine::default();
    for sample in [12, 29] {
        // Reuse the route with a fresh handle for each acquisition.
        let (device, completion) = driver.signal();
        machine.next_completion = Some(completion);
        driver.start(&mut machine, &route, ops::Capture);
        let issued = machine.digitizer.issued;
        assert!(driver.take_completion().is_none());

        device.complete(Ok(sample));
        driver.run_until_complete(&mut machine)?;
        assert_eq!(machine.sample, Some(sample));
        assert_eq!(machine.digitizer.issued, issued);
    }
    assert_eq!(machine.digitizer.issued, 2);
    Ok(())
}
