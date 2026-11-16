// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Example policy: computations apply atomically at completion; edges delay
//! payload availability. No resource contention or wall-clock waiting is modeled.

use super::framework::{Context, Dynamic, Element, Resume, Static};
use eyre::{Result, ensure, eyre};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    time::Duration,
};

pub(super) struct ComputeTimeNs<const NS: u64>;
pub(super) struct DeliveryLatencyNs<const NS: u64>;
pub(super) struct DynamicComputeTime(pub(super) Duration);
pub(super) struct DynamicLatency(pub(super) Duration);
pub(super) struct TraceLabel(pub(super) &'static str);

trait Pending<M> {
    fn element(&self) -> Element;
    fn run(self: Box<Self>, machine: &mut M, context: &Scheduler<M>) -> Result<()>;
}
impl<M, W: Resume<M, Scheduler<M>>> Pending<M> for W {
    fn element(&self) -> Element {
        Resume::element(self)
    }
    fn run(self: Box<Self>, machine: &mut M, context: &Scheduler<M>) -> Result<()> {
        (*self).resume(machine, context)
    }
}
type Queue<M> = BTreeMap<(Duration, u64), Box<dyn Pending<M>>>;
#[derive(Debug)]
pub(super) struct Scheduled {
    pub(super) element: Element,
    pub(super) ready: Duration,
    pub(super) due: Duration,
}
pub(super) struct Scheduler<M> {
    pub(super) now: Cell<Duration>,
    queue: RefCell<Queue<M>>,
    sequence: Cell<u64>,
    pub(super) trace: RefCell<Vec<(Duration, &'static str)>>,
    pub(super) scheduled: RefCell<Vec<Scheduled>>,
    pub(super) completed: RefCell<Vec<(Duration, Element)>>,
}
impl<M> Scheduler<M> {
    fn enqueue<W: Resume<M, Self> + 'static>(&self, delay: Duration, work: W) -> Result<()> {
        let ready = self.now.get();
        let due = ready
            .checked_add(delay)
            .ok_or_else(|| eyre!("time overflow"))?;
        let sequence = self.sequence.get();
        self.sequence.set(
            sequence
                .checked_add(1)
                .ok_or_else(|| eyre!("sequence overflow"))?,
        );
        self.scheduled.borrow_mut().push(Scheduled {
            element: work.element(),
            ready,
            due,
        });
        self.queue
            .borrow_mut()
            .insert((due, sequence), Box::new(work));
        Ok(())
    }
    pub(super) fn run_next(&self, machine: &mut M) -> Result<bool> {
        let next = self.queue.borrow_mut().pop_first();
        let Some(((due, _), work)) = next else {
            return Ok(false);
        };
        self.now.set(due);
        let element = work.element();
        work.run(machine, self)?;
        self.completed.borrow_mut().push((due, element));
        Ok(true)
    }
}
impl<M, const NS: u64> Context<M, Static<ComputeTimeNs<NS>>> for Scheduler<M> {
    fn submit<W>(&self, _: &mut M, _: Static<ComputeTimeNs<NS>>, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static,
    {
        ensure!(
            matches!(work.element(), Element::Node { .. }),
            "computation time requires a node"
        );
        self.enqueue(Duration::from_nanos(NS), work)
    }
}
impl<M, const NS: u64> Context<M, Static<DeliveryLatencyNs<NS>>> for Scheduler<M> {
    fn submit<W>(&self, _: &mut M, _: Static<DeliveryLatencyNs<NS>>, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static,
    {
        ensure!(
            matches!(work.element(), Element::Edge { .. }),
            "delivery latency requires an edge"
        );
        self.enqueue(Duration::from_nanos(NS), work)
    }
}
impl<M> Context<M, Dynamic<DynamicComputeTime>> for Scheduler<M> {
    fn submit<W>(
        &self,
        _: &mut M,
        Dynamic(duration): Dynamic<DynamicComputeTime>,
        work: W,
    ) -> Result<()>
    where
        W: Resume<M, Self> + 'static,
    {
        ensure!(
            matches!(work.element(), Element::Node { .. }),
            "computation time requires a node"
        );
        self.enqueue(duration.0, work)
    }
}
impl<M> Context<M, Dynamic<DynamicLatency>> for Scheduler<M> {
    fn submit<W>(&self, _: &mut M, Dynamic(latency): Dynamic<DynamicLatency>, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static,
    {
        ensure!(
            matches!(work.element(), Element::Edge { .. }),
            "delivery latency requires an edge"
        );
        self.enqueue(latency.0, work)
    }
}
impl<M> Context<M, Dynamic<TraceLabel>> for Scheduler<M> {
    fn submit<W>(&self, _: &mut M, Dynamic(label): Dynamic<TraceLabel>, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static,
    {
        self.trace.borrow_mut().push((self.now.get(), label.0));
        self.enqueue(Duration::ZERO, work)
    }
}
impl<M> Default for Scheduler<M> {
    fn default() -> Self {
        Self {
            now: Cell::new(Duration::ZERO),
            queue: RefCell::new(BTreeMap::new()),
            sequence: Cell::new(0),
            trace: RefCell::new(Vec::new()),
            scheduled: RefCell::new(Vec::new()),
            completed: RefCell::new(Vec::new()),
        }
    }
}
