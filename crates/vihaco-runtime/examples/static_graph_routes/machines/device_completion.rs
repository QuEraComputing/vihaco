// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A synchronous instruction returns a handle that the route waits on.

use crate::framework::{
    ChannelDriver, Describe, Route, Run, Wait, apply_effect_at, await_work, build_message_at,
    instruction_at, step_graph,
};
use eyre::{Result, eyre};
use std::convert::Infallible;
use vihaco_runtime::{Apply, BuildMessage, Execute, component, composite, dialect, field};

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
    sample: Option<u16>,
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
}

type DigitizerField = field!(Machine::digitizer);

impl BuildMessage<DigitizerField, ops::Capture, (), Wait<u16>> for Machine {
    fn build_message(&mut self, _: &ops::Capture, _: &()) -> Result<Wait<u16>> {
        self.next_completion
            .take()
            .ok_or_else(|| eyre!("capture has no completion handle"))
    }
}

impl Apply<u16> for Digitizer {
    type Effect = ();
    type Error = Infallible;

    fn apply(&mut self, sample: u16) -> Result<(), Self::Error> {
        self.sample = Some(sample);
        Ok(())
    }
}

fn route() -> Route<
    ops::Capture,
    impl Run<Machine, ops::Capture, ChannelDriver<Machine>, Output = ()> + Describe,
> {
    // The completion path parks on the handle returned by Execute.
    let completion = await_work().then(apply_effect_at::<DigitizerField>());
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
        assert_eq!(machine.digitizer.sample, Some(sample));
        assert_eq!(machine.digitizer.issued, issued);
    }
    assert_eq!(machine.digitizer.issued, 2);
    Ok(())
}
