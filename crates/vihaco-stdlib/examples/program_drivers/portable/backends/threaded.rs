// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Tokio drives local futures on a worker's owner thread.

use super::super::tasks::{Submit, TaskId, TaskIds, Ticket, track};
use super::{DemoInput, Drive, Input};
use eyre::{Result, WrapErr, eyre};
use std::{future::Future, rc::Rc, thread, time::Duration};
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc,
    task::LocalSet,
    time::{sleep, timeout},
};

#[derive(Clone)]
pub(crate) struct TokioSubmitter {
    local: Rc<LocalSet>,
    ids: Rc<TaskIds>,
    ready: mpsc::UnboundedSender<TaskId>,
}

impl Submit for TokioSubmitter {
    fn submit<T: 'static>(
        &self,
        future: impl Future<Output = Result<T>> + 'static,
    ) -> Result<Ticket<T>> {
        let id = self.ids.next()?;
        let ready = self.ready.clone();
        let (ticket, job) = track(id, future, move |id| {
            let _ = ready.send(id);
        });
        drop(self.local.spawn_local(job));
        Ok(ticket)
    }
}

pub(crate) struct TokioWorker {
    runtime: Runtime,
    submitter: TokioSubmitter,
    ready: mpsc::UnboundedReceiver<TaskId>,
}

impl TokioWorker {
    pub(crate) fn new() -> Result<Self> {
        let (send, ready) = mpsc::unbounded_channel();
        let runtime = Builder::new_current_thread().enable_time().build()?;
        Ok(Self {
            runtime,
            submitter: TokioSubmitter {
                local: Rc::new(LocalSet::new()),
                ids: Rc::default(),
                ready: send,
            },
            ready,
        })
    }
}

impl Drive for TokioWorker {
    type Submitter = TokioSubmitter;

    fn submitter(&self) -> Self::Submitter {
        self.submitter.clone()
    }

    fn enqueue_turn(&self) -> Result<TaskId> {
        let task = self.submitter.ids.next()?;
        self.submitter.ready.send(task)?;
        Ok(task)
    }

    fn next_ready(&mut self) -> Result<TaskId> {
        self.runtime
            .block_on(self.submitter.local.run_until(async {
                tokio::task::yield_now().await;
                timeout(Duration::from_secs(5), self.ready.recv()).await
            }))
            .wrap_err("waiting for a Tokio task")?
            .ok_or_else(|| eyre!("task completion queue closed"))
    }
}

/// Assign programs before creating worker-local machines or execution contexts.
pub(crate) fn run_on_workers<P: Send, R: Send>(
    workers: usize,
    programs: Vec<P>,
    run_sections: impl Fn(TokioWorker, Vec<P>) -> Result<R> + Sync,
) -> Result<Vec<R>> {
    eyre::ensure!(workers > 0, "worker count must be positive");
    let mut assigned: Vec<Vec<P>> = (0..workers).map(|_| Vec::new()).collect();
    for (index, program) in programs.into_iter().enumerate() {
        assigned[index % workers].push(program);
    }
    thread::scope(|scope| {
        let handles = assigned
            .into_iter()
            .map(|programs| {
                let run_sections = &run_sections;
                scope.spawn(move || run_sections(TokioWorker::new()?, programs))
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().map_err(|_| eyre!("Tokio worker panicked"))?)
            .collect()
    })
}

impl DemoInput for TokioWorker {
    fn schedule_input(&mut self, input: Input) {
        drop(self.submitter.local.spawn_local(async move {
            sleep(Duration::from_millis(input.delay)).await;
            let _ = input.port.send(input.sample);
        }));
    }
}
