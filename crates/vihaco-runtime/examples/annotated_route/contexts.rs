// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Reusable example execution policy: latency/trace annotations and a scheduler.
//! Applications can supply their own policies against the framework contracts.

use super::framework::{Context, Resume, StaticContext};
use eyre::{Result, eyre};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    time::Duration,
};

// Both forms preserve their concrete annotation type in the route.
pub(super) struct FixedLatencyNs<const NS: u64>;
pub(super) struct DynamicLatency(pub(super) Duration);
pub(super) struct TraceLabel(pub(super) &'static str);

// A single-threaded scheduler. Erasure is confined to its heterogeneous queue.
trait Pending<M> {
    fn run(self: Box<Self>, machine: &mut M, context: &Scheduler<M>) -> Result<()>;
}
impl<M, W: Resume<M, Scheduler<M>>> Pending<M> for W {
    fn run(self: Box<Self>, machine: &mut M, context: &Scheduler<M>) -> Result<()> {
        (*self).resume(machine, context)
    }
}
type Queue<M> = BTreeMap<(Duration, u64), Box<dyn Pending<M>>>;

pub(super) struct Scheduler<M> {
    pub(super) now: Cell<Duration>,
    queue: RefCell<Queue<M>>,
    sequence: Cell<u64>,
    pub(super) trace: RefCell<Vec<(Duration, &'static str)>>,
}
impl<M> Scheduler<M> {
    fn enqueue<W: Resume<M, Self> + 'static>(&self, delay: Duration, work: W) -> Result<()> {
        let due = self
            .now
            .get()
            .checked_add(delay)
            .ok_or_else(|| eyre!("time overflow"))?;
        let sequence = self.sequence.get();
        self.sequence.set(
            sequence
                .checked_add(1)
                .ok_or_else(|| eyre!("sequence overflow"))?,
        );
        self.queue
            .borrow_mut()
            .insert((due, sequence), Box::new(work));
        Ok(())
    }

    pub(super) fn run_next(&self, machine: &mut M) -> Result<bool> {
        // Drop the queue borrow before running work: it may enqueue more work.
        let next = self.queue.borrow_mut().pop_first();
        let Some(((due, _), work)) = next else {
            return Ok(false);
        };
        self.now.set(due);
        work.run(machine, self)?;
        println!("t={} ns: resumed scheduled operation", due.as_nanos());
        Ok(true)
    }
}
impl<M, const NS: u64> StaticContext<M, FixedLatencyNs<NS>> for Scheduler<M> {
    fn submit_static<W>(&self, _: &mut M, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static,
    {
        self.enqueue(Duration::from_nanos(NS), work)
    }
}
impl<M> Context<M, DynamicLatency> for Scheduler<M> {
    fn submit<W>(&self, _: &mut M, latency: DynamicLatency, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static,
    {
        self.enqueue(latency.0, work)
    }
}
impl<M> Context<M, TraceLabel> for Scheduler<M> {
    fn submit<W>(&self, _: &mut M, label: TraceLabel, work: W) -> Result<()>
    where
        W: Resume<M, Self> + 'static,
    {
        // This annotation records submission, not completion, and adds no delay.
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
        }
    }
}
