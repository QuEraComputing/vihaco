// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Shared submission vocabulary. No machine, route, or clock enters this API.

use eyre::{Result, WrapErr, eyre};
use std::{cell::Cell, future::Future};
use tokio::sync::oneshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct TaskId(u64);

#[derive(Default)]
pub(crate) struct TaskIds(Cell<u64>);

impl TaskIds {
    pub(crate) fn next(&self) -> Result<TaskId> {
        let id = self.0.get();
        self.0.set(
            id.checked_add(1)
                .ok_or_else(|| eyre!("task IDs exhausted"))?,
        );
        Ok(TaskId(id))
    }
}

pub(crate) trait Submit: Clone + 'static {
    fn submit<T: 'static>(
        &self,
        future: impl Future<Output = Result<T>> + 'static,
    ) -> Result<Ticket<T>>;
}

/// Optional backend capability for releasing a result at a backend-native deadline.
pub(crate) trait TimedSubmit: Submit {
    type Deadline;

    fn deadline(&self, ticks: u64) -> Result<Self::Deadline>;

    fn submit_at<T: 'static>(&self, deadline: Self::Deadline, value: T) -> Result<Ticket<T>>;
}

pub(crate) struct Ticket<T> {
    id: TaskId,
    result: oneshot::Receiver<Result<T>>,
}

impl<T> Ticket<T> {
    pub(crate) fn id(&self) -> TaskId {
        self.id
    }

    /// The owner calls this after receiving the task's ready notification.
    pub(crate) fn take_result(mut self) -> Result<T> {
        self.result.try_recv().wrap_err("task result unavailable")?
    }
}

/// Store the typed result before announcing the task. The scheduler drives only
/// the returned unit-output future. The route continuation stays with its owner.
pub(crate) fn track<T>(
    id: TaskId,
    future: impl Future<Output = Result<T>>,
    notify: impl FnOnce(TaskId),
) -> (Ticket<T>, impl Future<Output = ()>) {
    let (send, result) = oneshot::channel();
    let job = async move {
        if send.send(future.await).is_ok() {
            notify(id);
        }
    };
    (Ticket { id, result }, job)
}
