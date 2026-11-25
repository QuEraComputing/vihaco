// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Deterministic time and event delivery for the route examples.
//!
//! Events are ordered by tick and then by insertion order. An event scheduled
//! for a past tick becomes eligible at the current tick. `advance_next`
//! advances to the next event tick. `advance_to` delivers events through
//! its target and polls woken local futures along the way.
//!
//! `MinExecution<N>` lets an operation start at the current tick and holds
//! successful delivery until at least `N` ticks have elapsed. Operations
//! can mutate the machine at start. Errors propagate as soon as the
//! operation reports them.

use crate::framework::{
    AnnotatedDriver, Await, FutureDriver, Metadata, Operation, PollJob, poll_future,
};
use crate::timing::minimum_execution;
use eyre::{Result, ensure, eyre};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    future::Future,
};

pub(super) struct Scheduled<T> {
    pub(super) tick: u64,
    pub(super) result: Result<T>,
}

type Event<M> = Box<dyn FnOnce(&mut M, &SimDriver<M>)>;
type EventQueue<M> = BTreeMap<(u64, u64), Event<M>>;

pub(super) struct SimDriver<M> {
    now: Cell<u64>,
    sequence: Cell<u64>,
    events: RefCell<EventQueue<M>>,
    futures: RefCell<Vec<PollJob<M, Self>>>,
}

impl<M> SimDriver<M> {
    pub(super) fn new() -> Self {
        Self::starting_at(0)
    }

    /// Creates an empty simulator starting at the supplied tick.
    pub(super) fn starting_at(tick: u64) -> Self {
        Self {
            now: Cell::new(tick),
            sequence: Cell::new(0),
            events: RefCell::default(),
            futures: RefCell::default(),
        }
    }

    pub(super) fn now(&self) -> u64 {
        self.now.get()
    }

    #[cfg(feature = "program-image-execution")]
    pub(super) fn advance_next(&self, machine: &mut M) -> Result<u64> {
        let tick = self
            .events
            .borrow()
            .first_key_value()
            .map(|(&(tick, _), _)| tick)
            .ok_or_else(|| eyre!("program is waiting with no scheduled event"))?;
        self.advance_to(machine, tick)?;
        Ok(tick)
    }

    fn poll_ready(&self, machine: &mut M) {
        let jobs = std::mem::take(&mut *self.futures.borrow_mut());
        for mut job in jobs {
            if !job(machine, self) {
                self.futures.borrow_mut().push(job);
            }
        }
    }

    pub(super) fn advance_to(&self, machine: &mut M, target: u64) -> Result<()> {
        ensure!(target >= self.now(), "simulation cannot go backward");
        self.poll_ready(machine);
        loop {
            let Some(((tick, _), run)) = self.take_ready_event(target) else {
                break;
            };
            self.now.set(tick);
            run(machine, self);
            self.poll_ready(machine);
        }
        self.now.set(target);
        Ok(())
    }

    fn take_ready_event(&self, target: u64) -> Option<((u64, u64), Event<M>)> {
        let mut events = self.events.borrow_mut();
        if events
            .first_key_value()
            .is_some_and(|(&(tick, _), _)| tick <= target)
        {
            events.pop_first()
        } else {
            None
        }
    }
}

impl<M: 'static, T: 'static> Await<M, Scheduled<T>> for SimDriver<M> {
    type Output = T;

    fn wait(
        &self,
        _: &mut M,
        work: Scheduled<T>,
        done: impl FnOnce(&mut M, &Self, Result<T>) + 'static,
    ) {
        let sequence = self.sequence.get();
        self.sequence.set(sequence + 1);
        let key = (work.tick.max(self.now()), sequence);
        self.events.borrow_mut().insert(
            key,
            Box::new(move |machine, driver| {
                done(machine, driver, work.result);
            }),
        );
    }
}

impl<M: 'static> AnnotatedDriver<M> for SimDriver<M> {
    fn run_annotated<I, N: Operation<M, I, Self>>(
        &self,
        machine: &mut M,
        metadata: &Metadata,
        input: I,
        done: impl FnOnce(&mut M, &Self, Result<N::Output>) + 'static,
    ) {
        let ticks = minimum_execution(metadata);
        let Some(ready_at) = self.now().checked_add(ticks) else {
            return done(machine, self, Err(eyre!("simulation node time overflow")));
        };
        N::start(
            machine,
            self,
            input,
            move |machine, driver, result| match result {
                Ok(value) if driver.now() < ready_at => driver.wait(
                    machine,
                    Scheduled {
                        tick: ready_at,
                        result: Ok(value),
                    },
                    done,
                ),
                result => done(machine, driver, result),
            },
        );
    }
}

impl<M> FutureDriver<M> for SimDriver<M> {
    fn run_future<T: 'static>(
        &self,
        machine: &mut M,
        future: impl Future<Output = Result<T>> + 'static,
        done: impl FnOnce(&mut M, &Self, Result<T>) + 'static,
    ) {
        if let Some(job) = poll_future(machine, self, None, future, done) {
            self.futures.borrow_mut().push(job);
        }
    }
}
