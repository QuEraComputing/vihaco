// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Drives local route futures with Tokio and delivers their results to the machine.

use crate::framework::FutureDriver;
use eyre::{Result, WrapErr, eyre};
use std::{future::Future, time::Duration};
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc,
    task::LocalSet,
    time::timeout,
};

type Resume<M> = Box<dyn FnOnce(&mut M, &TokioDriver<M>)>;

pub(super) struct TokioDriver<M> {
    tasks: LocalSet,
    runtime: Runtime,
    ready: mpsc::UnboundedReceiver<Resume<M>>,
    completed: mpsc::UnboundedSender<Resume<M>>,
}

impl<M> TokioDriver<M> {
    pub(super) fn new() -> Result<Self> {
        let (completed, ready) = mpsc::unbounded_channel();
        Ok(Self {
            tasks: LocalSet::new(),
            runtime: Builder::new_current_thread().enable_time().build()?,
            ready,
            completed,
        })
    }

    /// Runs Tokio until a result can be delivered on the machine's thread.
    pub(super) fn resume_one(&mut self, machine: &mut M) -> Result<()> {
        let resume = self
            .runtime
            .block_on(
                self.tasks
                    .run_until(async { timeout(Duration::from_secs(5), self.ready.recv()).await }),
            )
            .wrap_err("waiting for a Tokio route completion")?
            .ok_or_else(|| eyre!("Tokio completion queue closed"))?;
        resume(machine, self);
        Ok(())
    }
}

impl<M> crate::framework::RouteDelivery<M> for TokioDriver<M> {}

impl<M: 'static> FutureDriver<M> for TokioDriver<M> {
    fn run_future<T: 'static>(
        &self,
        _: &mut M,
        future: impl Future<Output = Result<T>> + 'static,
        done: impl FnOnce(&mut M, &Self, Result<T>) + 'static,
    ) {
        let completed = self.completed.clone();
        // The task owns the future. The route callback runs when resume_one borrows the machine.
        drop(self.tasks.spawn_local(async move {
            let result = future.await;
            let _ = completed.send(Box::new(move |machine, driver| {
                done(machine, driver, result)
            }));
        }));
    }
}
