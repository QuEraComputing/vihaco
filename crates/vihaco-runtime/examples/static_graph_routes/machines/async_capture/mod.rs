// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! An async instruction keeps its component across a suspension.

#[cfg(test)]
mod tokio_driver;

use crate::framework::{
    AsyncExecute, AsyncSlot, BuildAsyncMessage, ChannelDriver, Describe, FutureDriver, Route, Run,
    apply_effect_at, async_instruction_at, build_async_message_at, step_graph,
};
use eyre::{Result, WrapErr, eyre};
use std::convert::Infallible;
use tokio::sync::oneshot;
use vihaco_runtime::{Apply, Field};

struct Capture;

#[derive(Default)]
struct Digitizer {
    issued: usize,
    sample: Option<u16>,
}

impl AsyncExecute<Capture> for Digitizer {
    type Message = oneshot::Receiver<u16>;
    type Effect = u16;
    type Error = eyre::Report;

    async fn execute(&mut self, _: &Capture, sample: Self::Message) -> Result<u16> {
        self.issued += 1;
        // The component remains leased while its own future waits for the sample.
        sample.await.wrap_err("digitizer sample channel closed")
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

impl Apply<u16> for AsyncSlot<Digitizer> {
    type Effect = ();
    type Error = eyre::Report;

    fn apply(&mut self, sample: u16) -> Result<()> {
        self.apply_to_component(sample)
    }
}

struct Machine {
    digitizer: AsyncSlot<Digitizer>,
    next_sample: Option<oneshot::Receiver<u16>>,
}

struct DigitizerField;

impl Field<DigitizerField> for Machine {
    type Child = AsyncSlot<Digitizer>;
    const SOURCE: &'static [&'static str] = &["digitizer"];

    fn get(&self) -> &Self::Child {
        &self.digitizer
    }

    fn get_mut(&mut self) -> &mut Self::Child {
        &mut self.digitizer
    }
}

impl BuildAsyncMessage<DigitizerField, Capture> for Machine {
    type Message = oneshot::Receiver<u16>;

    fn build_async_message(&mut self, _: &Capture) -> Result<Self::Message> {
        self.next_sample
            .take()
            .ok_or_else(|| eyre!("capture has no sample channel"))
    }
}

fn route<D: FutureDriver<Machine>>()
-> Route<Capture, impl Run<Machine, Capture, D, Output = ()> + Describe> {
    step_graph::<Capture>()
        .begin(build_async_message_at::<DigitizerField>())
        .then_execute_async(async_instruction_at::<DigitizerField>())
        // Execution returns the lease before this node borrows the same field.
        .then(apply_effect_at::<DigitizerField>())
        .finish()
}

fn fixture() -> (Machine, oneshot::Sender<u16>) {
    let (send, receive) = oneshot::channel();
    let machine = Machine {
        digitizer: AsyncSlot::new(Digitizer::default()),
        next_sample: Some(receive),
    };
    (machine, send)
}

pub(crate) fn demo() -> Result<()> {
    let route = route();
    route.topology().print("Digitizer / Async Capture");

    let (mut machine, device) = fixture();
    let driver = ChannelDriver::new();
    driver.start(&mut machine, &route, Capture);
    assert_eq!(
        machine.digitizer.inspect(|digitizer| digitizer.issued),
        None
    );

    device
        .send(23)
        .map_err(|_| eyre!("capture stopped waiting"))?;
    driver.run_until_complete(&mut machine)?;
    assert_eq!(
        machine
            .digitizer
            .inspect(|digitizer| (digitizer.issued, digitizer.sample)),
        Some((1, Some(23)))
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::tokio_driver::TokioDriver;
    use super::*;
    use crate::simulation::SimDriver;

    #[test]
    fn async_capture_returns_its_component_before_delivering_the_sample() -> Result<()> {
        demo()
    }

    #[test]
    fn lease_returns_on_error_or_drop() {
        let (mut machine, device) = fixture();
        let driver = ChannelDriver::new();
        driver.start(&mut machine, &route(), Capture);
        drop(device);
        assert!(driver.run_until_complete(&mut machine).is_err());
        assert_eq!(
            machine.digitizer.inspect(|digitizer| digitizer.sample),
            Some(None)
        );

        let (mut machine, _device) = fixture();
        driver.start(&mut machine, &route(), Capture);
        drop(driver);
        assert_eq!(
            machine.digitizer.inspect(|digitizer| digitizer.issued),
            Some(1)
        );
    }

    #[test]
    fn same_async_route_runs_on_tokio_and_simulation() -> Result<()> {
        let (mut machine, device) = fixture();
        let mut driver = TokioDriver::new()?;
        route().start_with(&mut machine, &driver, Capture, |machine, _, result| {
            result.expect("Tokio capture succeeds");
            assert_eq!(
                machine.digitizer.inspect(|digitizer| digitizer.sample),
                Some(Some(23))
            );
        });
        device.send(23).expect("capture owns the receiver");
        driver.resume_one(&mut machine)?;

        let (mut machine, device) = fixture();
        let driver = SimDriver::new();
        route().start_with(&mut machine, &driver, Capture, |_, _, result| {
            result.expect("simulated capture succeeds");
        });
        device.send(29).expect("capture owns the receiver");
        driver.advance_to(&mut machine, 1)?;
        assert_eq!(
            machine.digitizer.inspect(|digitizer| digitizer.sample),
            Some(Some(29))
        );
        Ok(())
    }
}
