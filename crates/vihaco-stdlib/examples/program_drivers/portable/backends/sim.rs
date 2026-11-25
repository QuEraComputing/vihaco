// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Deterministic virtual time. Device input wakes an ordinary Rust future.

use super::super::tasks::{Submit, TaskId, TaskIds, Ticket, track};
use super::{DemoInput, Drive, Input};
use eyre::{Result, eyre};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{Arc, mpsc},
    task::{Context, Wake, Waker},
};

type Job = Pin<Box<dyn Future<Output = ()>>>;

struct WakeTask {
    task: TaskId,
    runnable: mpsc::Sender<TaskId>,
}

impl Wake for WakeTask {
    fn wake(self: Arc<Self>) {
        let _ = self.runnable.send(self.task);
    }
}

struct Queue {
    ids: TaskIds,
    jobs: RefCell<BTreeMap<TaskId, Job>>,
    ready: RefCell<VecDeque<TaskId>>,
    runnable: mpsc::Sender<TaskId>,
}

#[derive(Clone)]
pub(crate) struct SimSubmitter(Rc<Queue>);

impl Submit for SimSubmitter {
    fn submit<T: 'static>(
        &self,
        future: impl Future<Output = Result<T>> + 'static,
    ) -> Result<Ticket<T>> {
        let id = self.0.ids.next()?;
        let queue = Rc::downgrade(&self.0);
        let (ticket, task) = track(id, future, move |id| {
            if let Some(queue) = queue.upgrade() {
                queue.ready.borrow_mut().push_back(id);
            }
        });
        self.0.jobs.borrow_mut().insert(id, Box::pin(task));
        self.0.runnable.send(id)?;
        Ok(ticket)
    }
}

pub(crate) struct Sim {
    now: u64,
    next_input: usize,
    inputs: BTreeMap<(u64, usize), Input>,
    runnable: mpsc::Receiver<TaskId>,
    submitter: SimSubmitter,
}

impl Sim {
    pub(crate) fn new() -> Self {
        let (send, runnable) = mpsc::channel();
        Self {
            now: 0,
            next_input: 0,
            inputs: BTreeMap::new(),
            runnable,
            submitter: SimSubmitter(Rc::new(Queue {
                ids: TaskIds::default(),
                jobs: RefCell::default(),
                ready: RefCell::default(),
                runnable: send,
            })),
        }
    }

    pub(crate) fn now(&self) -> u64 {
        self.now
    }

    fn poll_task(&self, task: TaskId) {
        let queue = &self.submitter.0;
        let Some(mut job) = queue.jobs.borrow_mut().remove(&task) else {
            return;
        };
        let waker = Waker::from(Arc::new(WakeTask {
            task,
            runnable: queue.runnable.clone(),
        }));
        if job
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
        {
            queue.jobs.borrow_mut().insert(task, job);
        }
    }
}

impl Drive for Sim {
    type Submitter = SimSubmitter;

    fn submitter(&self) -> Self::Submitter {
        self.submitter.clone()
    }

    fn enqueue_turn(&self) -> Result<TaskId> {
        let task = self.submitter.0.ids.next()?;
        self.submitter.0.ready.borrow_mut().push_back(task);
        Ok(task)
    }

    fn next_ready(&mut self) -> Result<TaskId> {
        loop {
            if let Some(task) = self.submitter.0.ready.borrow_mut().pop_front() {
                return Ok(task);
            }
            if let Ok(task) = self.runnable.try_recv() {
                self.poll_task(task);
                continue;
            }
            let ((tick, _), input) = self
                .inputs
                .pop_first()
                .ok_or_else(|| eyre!("simulation has pending tasks but no device event"))?;
            self.now = tick;
            let _ = input.port.send(input.sample);
        }
    }
}

impl DemoInput for Sim {
    fn schedule_input(&mut self, input: Input) {
        let key = (self.now + input.delay, self.next_input);
        self.next_input += 1;
        self.inputs.insert(key, input);
    }
}
