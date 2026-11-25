// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Deterministic virtual time. Device input wakes an ordinary Rust future.

use super::super::tasks::{Submit, TaskId, TaskIds, Ticket, TimedSubmit, track};
use super::{DemoInput, Drive, Input};
use crate::framework::RegisteredTopology;
use eyre::{Result, eyre};
use std::{
    any::TypeId,
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap, VecDeque},
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

type Events = BTreeMap<(u64, TaskId), Box<dyn FnOnce()>>;

struct Queue {
    now: Cell<u64>,
    delivery_delays: HashMap<TypeId, Rc<[u64]>>,
    events: RefCell<Events>,
    ids: TaskIds,
    jobs: RefCell<BTreeMap<TaskId, Job>>,
    ready: RefCell<VecDeque<TaskId>>,
    runnable: mpsc::Sender<TaskId>,
}

impl Queue {
    fn schedule(&self, tick: u64, event: impl FnOnce() + 'static) -> Result<()> {
        let sequence = self.ids.next()?;
        self.events
            .borrow_mut()
            .insert((tick, sequence), Box::new(event));
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct SimSubmitter(Rc<Queue>);

impl TimedSubmit for SimSubmitter {
    type Deadline = u64;

    fn deadline(&self, ticks: u64) -> Result<u64> {
        self.0
            .now
            .get()
            .checked_add(ticks)
            .ok_or_else(|| eyre!("simulation deadline overflow"))
    }

    fn submit_at<T: 'static>(&self, deadline: u64, value: T) -> Result<Ticket<T>> {
        if deadline <= self.0.now.get() {
            return self.submit(std::future::ready(Ok(value)));
        }
        let (send, receive) = tokio::sync::oneshot::channel();
        self.0.schedule(deadline, move || {
            let _ = send.send(value);
        })?;
        self.submit(async move { Ok(receive.await?) })
    }
}

impl Submit for SimSubmitter {
    fn edge_delays(&self, route: TypeId) -> Option<Rc<[u64]>> {
        self.0.delivery_delays.get(&route).cloned()
    }

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
    runnable: mpsc::Receiver<TaskId>,
    submitter: SimSubmitter,
}

impl Sim {
    /// Resolves the name before the backend starts running any program turns.
    #[allow(dead_code, reason = "used by the logical_routes example")]
    pub(crate) fn delay_edge(
        mut self,
        route: &RegisteredTopology,
        name: &str,
        ticks: u64,
    ) -> Result<Self> {
        let id = route.topology.named_edge(name)?.0;
        let queue = Rc::get_mut(&mut self.submitter.0)
            .ok_or_else(|| eyre!("edge policy must be bound before the runtime starts"))?;
        let delays = queue.delivery_delays.entry(route.route_key).or_default();
        let mut by_edge = delays.to_vec();
        if by_edge.len() <= id {
            by_edge.resize(id + 1, 0);
        }
        by_edge[id] = ticks;
        *delays = by_edge.into();
        Ok(self)
    }

    pub(crate) fn new() -> Self {
        Self::starting_at(0)
    }

    pub(crate) fn starting_at(tick: u64) -> Self {
        let (send, runnable) = mpsc::channel();
        Self {
            runnable,
            submitter: SimSubmitter(Rc::new(Queue {
                now: Cell::new(tick),
                delivery_delays: HashMap::default(),
                events: RefCell::default(),
                ids: TaskIds::default(),
                jobs: RefCell::default(),
                ready: RefCell::default(),
                runnable: send,
            })),
        }
    }

    pub(crate) fn now(&self) -> u64 {
        self.submitter.0.now.get()
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
            let queue = &self.submitter.0;
            let ((tick, _), event) = queue
                .events
                .borrow_mut()
                .pop_first()
                .ok_or_else(|| eyre!("simulation has pending tasks but no timed event"))?;
            queue.now.set(tick);
            event();
        }
    }
}

impl DemoInput for Sim {
    fn schedule_input(&mut self, input: Input) -> Result<()> {
        let tick = self.submitter.deadline(input.delay)?;
        self.submitter.0.schedule(tick, move || {
            let _ = input.port.send(input.sample);
        })
    }
}
