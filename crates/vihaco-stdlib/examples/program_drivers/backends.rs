// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Delivery policies used by the shared sample route.

use crate::{
    checked::Backend,
    framework::{AnnotatedDriver, Await, ChannelDriver, FutureDriver},
    machine::{Machine, SampleEffect, SampleRequest},
    simulation::{Scheduled, SimDriver},
    tokio_driver::TokioDriver,
};
use eyre::{Result, ensure, eyre};
use std::{thread, time::Duration};
use vihaco_abi::word::Word;
use vihaco_runtime::PushToStack;

impl AnnotatedDriver<Machine> for TokioDriver<Machine> {}

impl Await<Machine, SampleRequest> for ChannelDriver<Machine> {
    type Output = SampleEffect;

    fn wait(
        &self,
        machine: &mut Machine,
        _: SampleRequest,
        done: impl FnOnce(&mut Machine, &Self, Result<SampleEffect>) + 'static,
    ) {
        let (sender, handle) = self.signal();
        machine.sender = Some(sender);
        self.wait(machine, handle, move |machine, driver, sample| {
            done(machine, driver, sample.map(|word| PushToStack([word])));
        });
    }
}

impl Backend<Machine> for ChannelDriver<Machine> {
    type Delivery = Result<Word>;

    fn deliver(&mut self, machine: &mut Machine, delivery: Self::Delivery) -> Result<()> {
        let sender = machine
            .sender
            .take()
            .ok_or_else(|| eyre!("no device reply expected"))?;
        sender.complete(delivery);
        self.poll_ready(machine);
        Ok(())
    }
}

impl Await<Machine, SampleRequest> for SimDriver<Machine> {
    type Output = SampleEffect;

    fn wait(
        &self,
        machine: &mut Machine,
        request: SampleRequest,
        done: impl FnOnce(&mut Machine, &Self, Result<SampleEffect>) + 'static,
    ) {
        let Some(tick) = self.now().checked_add(12) else {
            return done(machine, self, Err(eyre!("simulation tick overflow")));
        };
        self.wait(
            machine,
            Scheduled {
                tick,
                result: Ok(PushToStack([request.0])),
            },
            done,
        );
    }
}

impl<M> Backend<M> for SimDriver<M> {
    type Delivery = ();

    fn deliver(&mut self, machine: &mut M, (): ()) -> Result<()> {
        self.advance_next(machine)?;
        Ok(())
    }
}

impl Await<Machine, SampleRequest> for TokioDriver<Machine> {
    type Output = SampleEffect;

    fn wait(
        &self,
        machine: &mut Machine,
        request: SampleRequest,
        done: impl FnOnce(&mut Machine, &Self, Result<SampleEffect>) + 'static,
    ) {
        let owner = thread::current().id();
        let gate = machine.tokio_gate.clone();
        self.run_future(
            machine,
            async move {
                if let Some(gate) = gate {
                    gate.wait().await;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
                ensure!(
                    thread::current().id() == owner,
                    "route future left its owner thread"
                );
                Ok(PushToStack([request.0]))
            },
            done,
        );
    }
}

impl Backend<Machine> for TokioDriver<Machine> {
    type Delivery = ();

    fn deliver(&mut self, machine: &mut Machine, (): ()) -> Result<()> {
        self.resume_one(machine)
    }
}
