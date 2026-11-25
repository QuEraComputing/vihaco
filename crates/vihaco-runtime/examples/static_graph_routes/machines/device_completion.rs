// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A capture route awaits acquisition and then validates the sample asynchronously.

use crate::framework::{
    AsyncApply, ChannelDriver, Describe, Entry, Route, Run, Wait, async_apply_effect_at,
    await_work, build_message_at, execute_instruction_at, step_graph,
};
use eyre::{Result, WrapErr, ensure, eyre};
use std::{cell::Cell, future::Future, rc::Rc};
use tokio::sync::oneshot;
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
    sample: Rc<Cell<Option<u16>>>,
    next_validation: Option<oneshot::Receiver<bool>>,
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
        // The composite picks the handle from its external device connection.
        self.next_completion
            .take()
            .ok_or_else(|| eyre!("capture has no completion handle"))
    }
}

impl AsyncApply<u16> for Digitizer {
    type Effect = ();
    type Error = eyre::Report;

    fn apply(&mut self, sample: u16) -> impl Future<Output = Result<()>> + 'static {
        let validation = self.next_validation.take();
        let stored_sample = Rc::clone(&self.sample);
        async move {
            let validation = validation.ok_or_else(|| eyre!("sample has no validation handle"))?;
            ensure!(
                validation.await.wrap_err("sample validator closed")?,
                "sample rejected"
            );
            stored_sample.set(Some(sample));
            Ok(())
        }
    }
}

fn route() -> Route<
    ops::Capture,
    impl for<'a> Run<Machine, Entry<'a, ops::Capture, ()>, ChannelDriver<Machine>, Output = ()>
    + Describe,
> {
    // Acquisition and validation can each complete on their own schedule.
    let completion = await_work().then(async_apply_effect_at::<DigitizerField>());
    step_graph::<ops::Capture>()
        .begin(build_message_at::<DigitizerField, Wait<u16>>())
        .then(execute_instruction_at::<DigitizerField>())
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
        let (validator, validation) = oneshot::channel();
        machine.next_completion = Some(completion);
        machine.digitizer.next_validation = Some(validation);
        driver.start(&mut machine, &route, ops::Capture);
        let issued = machine.digitizer.issued;
        assert!(driver.take_completion().is_none());

        let previous_sample = machine.digitizer.sample.get();
        device.complete(Ok(sample));
        driver.poll_ready(&mut machine);
        // The capture is ready. The async handler still needs validation.
        assert_eq!(machine.digitizer.sample.get(), previous_sample);
        assert!(driver.take_completion().is_none());

        validator
            .send(true)
            .map_err(|_| eyre!("effect handler stopped waiting for validation"))?;
        driver.run_until_complete(&mut machine)?;
        assert_eq!(machine.digitizer.sample.get(), Some(sample));
        assert_eq!(machine.digitizer.issued, issued);
    }
    assert_eq!(machine.digitizer.issued, 2);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn async_validation_failure_stops_effect_delivery() {
        let route = route();
        let driver = ChannelDriver::new();
        let (device, completion) = driver.signal();
        let (validator, validation) = oneshot::channel();
        let mut machine = Machine::default();
        machine.next_completion = Some(completion);
        machine.digitizer.next_validation = Some(validation);

        driver.start(&mut machine, &route, ops::Capture);
        device.complete(Ok(12));
        driver.poll_ready(&mut machine);
        assert!(driver.take_completion().is_none());
        validator.send(false).expect("validation receiver is live");

        let error = driver.run_until_complete(&mut machine).unwrap_err();
        assert_eq!(error.to_string(), "sample rejected");
        assert_eq!(machine.digitizer.sample.get(), None);
        assert_eq!(machine.digitizer.issued, 1);
    }
}
