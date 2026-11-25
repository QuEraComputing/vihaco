// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Delivery policies used by the shared sample route.

use crate::{
    checked::Backend,
    framework::{Await, ChannelDriver},
    machine::{Machine, SampleEffect, SampleRequest},
    simulation::{Scheduled, SimDriver},
};
use eyre::{Result, eyre};
use vihaco_abi::word::Word;
use vihaco_runtime::PushToStack;

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
