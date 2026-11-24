// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! # Driver capabilities and channel backend
//!
//! `Await<M, H>` consumes a work handle supplied by a previous operation.
//! `FutureDriver<M>` runs a component-owned Rust future. Each accepts an owned
//! one-shot continuation that may finish inline or park. When it calls the
//! continuation, the backend supplies a fresh mutable machine borrow and a
//! shared driver reference. This permits the continuation to start another
//! operation or park again.
//!
//! `AnnotatedDriver` lives in the graph executor. It lets a backend inspect
//! metadata before invoking an annotated operation. The simulator overrides
//! the hook to delay a successful result. Unannotated operations require
//! only the capabilities they use.
//!
//! ## Channel driver
//!
//! `signal` creates a one-shot pair. Sending the result or dropping the
//! sender also notifies the driver. The notification is a readiness hint.
//! A pending job still checks its own receiver. Disconnecting the receiver's
//! producer delivers a route error.
//!
//! `start` puts terminal results from unit-output routes in a FIFO. Use
//! `take_completion` to inspect that queue or `run_until_complete` to pump
//! pending work and wait for an outcome. The five-second timeout applies to
//! one idle receive. These are demo conveniences. The queue reports
//! completion order and carries no invocation IDs. Warehouse supplies its
//! own per-instruction completion slot through `Route::start_with`.
//!
//! ## Local futures
//!
//! The channel driver and simulator use `poll_future` to pin a future and
//! poll it once immediately. A pending poll job owns the future and its
//! continuation. Its waker can be used across threads and only marks the
//! job ready. The driver polls and delivers results on the machine's
//! thread. It clears the ready flag before polling so a wake during the poll
//! can request another pass. The driver releases its queue borrow before
//! running jobs so callbacks can add work.
//!
//! The simulator drives this polling explicitly when advancing its clock.
//! The async digitizer's Tokio test driver uses a current-thread executor
//! and queues completed callbacks for host delivery. Its first poll happens
//! when the executor runs.

use super::graph::{AnnotatedDriver, Route, Run};
use eyre::{Result, eyre};
use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll as TaskPoll, Wake, Waker},
    time::Duration,
};

/// Waits for driver-specific work and delivers its typed result once.
pub(crate) trait Await<M, H> {
    /// The payload produced by completing a handle of type `H`.
    type Output: 'static;

    /// Consumes the handle and delivers its outcome inline or on a later driver turn.
    ///
    /// A parked continuation owns its captures. The driver supplies the machine
    /// borrow when it delivers the outcome.
    fn wait(
        &self,
        machine: &mut M,
        handle: H,
        done: impl FnOnce(&mut M, &Self, Result<Self::Output>) + 'static,
    );
}

/// Drives component-owned futures and hands their results back to the route.
pub(crate) trait FutureDriver<M> {
    /// Drives the owned future and calls `done` once when it resolves.
    ///
    /// The future may be local to the machine's thread. The driver controls
    /// when the first poll happens and when the result reaches the route.
    fn run_future<T: 'static>(
        &self,
        machine: &mut M,
        future: impl Future<Output = Result<T>> + 'static,
        done: impl FnOnce(&mut M, &Self, Result<T>) + 'static,
    );
}

/// A type-erased attempt to advance one pending operation.
///
/// `true` means the job delivered its result and should be discarded.
/// `false` means it is still pending and can be tried again.
pub(crate) type PollJob<M, D> = Box<dyn FnMut(&mut M, &D) -> bool>;

/// A device-completion handle consumed by the channel driver's `Await` implementation.
pub(crate) struct Wait<T>(Receiver<Result<T>>);

/// The producer half of a one-shot completion channel.
///
/// Publishing a result or dropping the producer wakes the driver so it can
/// check the matching `Wait`.
pub(crate) struct Signal<T> {
    result: Option<Sender<Result<T>>>,
    wake: Sender<()>,
}

impl<T> Signal<T> {
    /// Publishes a success or error before notifying the driver.
    pub(crate) fn complete(mut self, result: Result<T>) {
        if let Some(sender) = self.result.take() {
            let _ = sender.send(result);
            let _ = self.wake.send(());
        }
    }
}

/// Wakes the driver on disconnect so the waiting route can receive an error.
impl<T> Drop for Signal<T> {
    fn drop(&mut self) {
        if let Some(sender) = self.result.take() {
            drop(sender);
            let _ = self.wake.send(());
        }
    }
}

/// Tracks whether a local future can be polled again.
///
/// A wake can arrive from another thread. It marks readiness and optionally
/// notifies the driver without borrowing the machine or polling the future.
struct FutureWake {
    ready: AtomicBool,
    notify: Option<Sender<()>>,
}

/// Coalesces repeated wakes until the driver takes the ready flag.
impl Wake for FutureWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if !self.ready.swap(true, Ordering::AcqRel)
            && let Some(notify) = &self.notify
        {
            let _ = notify.send(());
        }
    }
}

/// Runs example routes using channel results and local futures.
///
/// The driver owns pending jobs and wake notifications. Its terminal result
/// queue is a convenience for callers that start routes through `start`.
pub(crate) struct ChannelDriver<M> {
    wake: Receiver<()>,
    sender: Sender<()>,
    pending: RefCell<Vec<PollJob<M, Self>>>,
    finished: RefCell<VecDeque<Result<()>>>,
}

/// Uses the default annotation policy and delivers operation results without a timing delay.
impl<M> AnnotatedDriver<M> for ChannelDriver<M> {}

impl<M: 'static> ChannelDriver<M> {
    /// Creates an empty driver with its own wake channel.
    pub(crate) fn new() -> Self {
        let (sender, wake) = mpsc::channel();
        Self {
            wake,
            sender,
            pending: RefCell::default(),
            finished: RefCell::default(),
        }
    }

    /// Creates matching signal and wait endpoints for one invocation.
    pub(crate) fn signal<T>(&self) -> (Signal<T>, Wait<T>) {
        let (sender, receiver) = mpsc::channel();
        (
            Signal {
                result: Some(sender),
                wake: self.sender.clone(),
            },
            Wait(receiver),
        )
    }

    /// Starts a route and queues its terminal result for the example caller.
    pub(crate) fn start<I, P>(&self, machine: &mut M, route: &Route<I, P>, instruction: I)
    where
        P: Run<M, I, Self, Output = ()>,
    {
        route.start_with(machine, self, instruction, |_, driver, result| {
            driver.complete(result)
        });
    }

    /// Resumes ready callbacks from the queue snapshot and returns their count.
    ///
    /// Work registered by those callbacks is eligible on a later pass.
    pub(crate) fn poll_ready(&self, machine: &mut M) -> usize {
        let mut ready = 0;
        let pending = std::mem::take(&mut *self.pending.borrow_mut());
        // Ready callbacks may park more work. The queue is free for them to use.
        for mut poll in pending {
            if poll(machine, self) {
                ready += 1;
            } else {
                self.pending.borrow_mut().push(poll);
            }
        }
        ready
    }

    /// Records a terminal route result for its caller.
    fn complete(&self, result: Result<()>) {
        self.finished.borrow_mut().push_back(result);
    }

    /// Retrieves the oldest terminal result when available.
    pub(crate) fn take_completion(&self) -> Option<Result<()>> {
        self.finished.borrow_mut().pop_front()
    }

    /// Pumps pending jobs until a terminal result is available.
    ///
    /// Returns a timeout after five seconds of waiting for a wake notification.
    pub(crate) fn run_until_complete(&self, machine: &mut M) -> Result<()> {
        loop {
            if let Some(result) = self.take_completion() {
                return result;
            }
            if self.poll_ready(machine) == 0 {
                self.wake.recv_timeout(Duration::from_secs(5))?;
            }
        }
    }
}

/// Parks the receiver and continuation until a result or disconnect is observed.
impl<M: 'static, T: 'static> Await<M, Wait<T>> for ChannelDriver<M> {
    type Output = T;

    fn wait(
        &self,
        _: &mut M,
        wait: Wait<T>,
        done: impl FnOnce(&mut M, &Self, Result<T>) + 'static,
    ) {
        let mut done = Some(done);
        self.pending
            .borrow_mut()
            .push(Box::new(move |machine, driver| {
                let result = match wait.0.try_recv() {
                    Ok(result) => result,
                    Err(TryRecvError::Empty) => return false,
                    Err(TryRecvError::Disconnected) => {
                        Err(eyre!("signal closed before completion"))
                    }
                };
                done.take().expect("pending work completes once")(machine, driver, result);
                true
            }));
    }
}

/// Polls the future eagerly and enqueues it if it still needs a wake.
impl<M: 'static> FutureDriver<M> for ChannelDriver<M> {
    fn run_future<T: 'static>(
        &self,
        machine: &mut M,
        future: impl Future<Output = Result<T>> + 'static,
        done: impl FnOnce(&mut M, &Self, Result<T>) + 'static,
    ) {
        if let Some(job) = poll_future(machine, self, Some(self.sender.clone()), future, done) {
            self.pending.borrow_mut().push(job);
        }
    }
}

/// Polls a local future once and returns a job if it is still pending.
///
/// A pending job owns the pinned future and continuation. Its waker marks it
/// ready for another poll and sends an optional notification to a waiting driver.
pub(crate) fn poll_future<M, D, T: 'static>(
    machine: &mut M,
    driver: &D,
    notify: Option<Sender<()>>,
    future: impl Future<Output = Result<T>> + 'static,
    done: impl FnOnce(&mut M, &D, Result<T>) + 'static,
) -> Option<PollJob<M, D>> {
    let wake = Arc::new(FutureWake {
        ready: AtomicBool::new(true),
        notify,
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut future = Box::pin(future);
    let mut done = Some(done);
    let mut job = move |machine: &mut M, driver: &D| {
        if !wake.ready.swap(false, Ordering::AcqRel) {
            return false;
        }
        match future.as_mut().poll(&mut Context::from_waker(&waker)) {
            TaskPoll::Pending => false,
            TaskPoll::Ready(result) => {
                done.take().expect("future completes once")(machine, driver, result);
                true
            }
        }
    };
    if job(machine, driver) {
        None
    } else {
        Some(Box::new(job))
    }
}
