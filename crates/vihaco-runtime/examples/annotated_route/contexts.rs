// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Optional scheduling backend: atomic computation at completion, transfer delay
//! on edges. Runtime queueing/erasure belongs to this backend, not the Route API.
//! Recording is opt-in and stores operation names and times only.

use super::{
    annotations::{ComputeTime, DeliveryLatency, TraceLabel},
    framework::{AnnotationBinding, Computation, Context, Resume, Wire},
};
use eyre::{Result, eyre};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    time::Duration,
};

// These generic hooks are statically dispatched. A no-op observer needs no
// metadata allocation, enum construction, or runtime branch to disable recording.
pub(super) trait Observer<M> {
    fn scheduled<C, W: Resume<M, C>>(&self, _: Duration, _: Duration) {}
    fn completed<C, W: Resume<M, C>>(&self, _: Duration) {}
    fn label(&self, _: Duration, _: &'static str) {}
}
#[derive(Default)]
pub(super) struct NoRecording;
impl<M> Observer<M> for NoRecording {}

pub(super) struct Scheduled {
    pub(super) operation: &'static str,
    pub(super) ready: Duration,
    pub(super) due: Duration,
}
#[derive(Default)]
pub(super) struct Recording {
    pub(super) trace: RefCell<Vec<(Duration, &'static str)>>,
    pub(super) scheduled: RefCell<Vec<Scheduled>>,
    pub(super) completed: RefCell<Vec<(Duration, &'static str)>>,
}
impl<M> Observer<M> for Recording {
    fn scheduled<C, W: Resume<M, C>>(&self, ready: Duration, due: Duration) {
        self.scheduled.borrow_mut().push(Scheduled {
            operation: std::any::type_name::<W::Operation>(),
            ready,
            due,
        });
    }
    fn completed<C, W: Resume<M, C>>(&self, due: Duration) {
        self.completed
            .borrow_mut()
            .push((due, std::any::type_name::<W::Operation>()));
    }
    fn label(&self, now: Duration, label: &'static str) {
        self.trace.borrow_mut().push((now, label));
    }
}

trait Pending<M, O> {
    fn run(self: Box<Self>, machine: &mut M, context: &Scheduler<M, O>) -> Result<()>;
}
impl<M, O: Observer<M>, W: Resume<M, Scheduler<M, O>>> Pending<M, O> for W {
    fn run(self: Box<Self>, machine: &mut M, context: &Scheduler<M, O>) -> Result<()> {
        let due = context.now.get();
        (*self).resume(machine, context)?;
        context.observer.completed::<Scheduler<M, O>, W>(due);
        Ok(())
    }
}
type Queue<M, O> = BTreeMap<(Duration, u64), Box<dyn Pending<M, O>>>;
pub(super) struct Scheduler<M, O = NoRecording> {
    pub(super) now: Cell<Duration>,
    queue: RefCell<Queue<M, O>>,
    sequence: Cell<u64>,
    pub(super) observer: O,
}
pub(super) type RecordingScheduler<M> = Scheduler<M, Recording>;
impl<M, O: Observer<M>> Scheduler<M, O> {
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
        self.observer.scheduled::<Self, W>(ready, due);
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
        work.run(machine, self)?;
        Ok(true)
    }
}
// Annotation compatibility is selected entirely by family and graph kind.
impl<M, O: Observer<M>> Context<M, ComputeTime, Computation> for Scheduler<M, O> {
    fn submit<A, W>(&self, _: &mut M, annotation: A, work: W) -> Result<()>
    where
        A: AnnotationBinding<Family = ComputeTime>,
        W: Resume<M, Self, Kind = Computation> + 'static,
    {
        self.enqueue(annotation.into_value(), work)
    }
}
impl<M, O: Observer<M>> Context<M, DeliveryLatency, Wire> for Scheduler<M, O> {
    fn submit<A, W>(&self, _: &mut M, annotation: A, work: W) -> Result<()>
    where
        A: AnnotationBinding<Family = DeliveryLatency>,
        W: Resume<M, Self, Kind = Wire> + 'static,
    {
        self.enqueue(annotation.into_value(), work)
    }
}
impl<M, O: Observer<M>, Kind> Context<M, TraceLabel, Kind> for Scheduler<M, O> {
    fn submit<A, W>(&self, _: &mut M, annotation: A, work: W) -> Result<()>
    where
        A: AnnotationBinding<Family = TraceLabel>,
        W: Resume<M, Self, Kind = Kind> + 'static,
    {
        self.observer.label(self.now.get(), annotation.into_value());
        self.enqueue(Duration::ZERO, work)
    }
}
impl<M, O: Default> Default for Scheduler<M, O> {
    fn default() -> Self {
        Self {
            now: Cell::new(Duration::ZERO),
            queue: RefCell::new(BTreeMap::new()),
            sequence: Cell::new(0),
            observer: O::default(),
        }
    }
}
