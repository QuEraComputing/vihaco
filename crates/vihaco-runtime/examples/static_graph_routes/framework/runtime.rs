// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The driver keeps all per-invocation state, including parked continuations.

use super::graph::{Route, Run};
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
    thread,
    time::Duration,
};

/// Waits for driver-specific work and delivers its typed result once.
pub(crate) trait Await<M, H> {
    type Output: 'static;

    fn wait(
        &self,
        machine: &mut M,
        handle: H,
        done: impl FnOnce(&mut M, &Self, Result<Self::Output>) + 'static,
    );
}

/// Submits a worker and returns the driver's own completion handle.
pub(crate) trait SpawnWorker {
    type Work<T: Send + 'static>: 'static;

    fn spawn<I, O>(&self, work: fn(I) -> Result<O>, input: I) -> Result<Self::Work<O>>
    where
        I: Send + 'static,
        O: Send + 'static;
}

/// Lets an async route node submit the opaque future returned by async Rust.
pub(crate) trait FutureDriver<M> {
    fn run_future<T: 'static>(
        &self,
        machine: &mut M,
        future: impl Future<Output = Result<T>> + 'static,
        done: impl FnOnce(&mut M, &Self, Result<T>) + 'static,
    );
}

pub(crate) struct RustFuture<F>(pub(crate) F);

impl<M, D, F, T: 'static> Await<M, RustFuture<F>> for D
where
    D: FutureDriver<M>,
    F: Future<Output = Result<T>> + 'static,
{
    type Output = T;

    fn wait(
        &self,
        machine: &mut M,
        RustFuture(future): RustFuture<F>,
        done: impl FnOnce(&mut M, &Self, Result<T>) + 'static,
    ) {
        self.run_future(machine, future, done);
    }
}

// The example drivers erase future poll jobs when they need to store them.
pub(crate) type PollJob<M, D> = Box<dyn FnMut(&mut M, &D) -> bool>;

/// The receiving half of a one-shot completion channel.
pub(crate) struct Wait<T>(Receiver<Result<T>>);

/// Publishes a completion result and notifies the driver.
pub(crate) struct Signal<T> {
    result: Option<Sender<Result<T>>>,
    wake: Sender<()>,
}

// Publishes the result before sending a wake notification.
impl<T> Signal<T> {
    /// Consumes the signal and publishes its completion result.
    pub(crate) fn complete(mut self, result: Result<T>) {
        if let Some(sender) = self.result.take() {
            let _ = sender.send(result);
            let _ = self.wake.send(());
        }
    }
}

// Wakes the driver if the producer closes its channel before publishing a result.
impl<T> Drop for Signal<T> {
    fn drop(&mut self) {
        if let Some(sender) = self.result.take() {
            drop(sender);
            let _ = self.wake.send(());
        }
    }
}

struct FutureWake {
    ready: AtomicBool,
    notify: Option<Sender<()>>,
}

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

/// Owns the parked continuations, wake channel, and terminal outcomes.
pub(crate) struct ChannelDriver<M> {
    wake: Receiver<()>,
    sender: Sender<()>,
    pending: RefCell<Vec<PollJob<M, Self>>>,
    finished: RefCell<VecDeque<Result<()>>>,
}

// Runs ready callbacks on the driver thread with a fresh borrow of the machine.
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

    /// Resumes ready callbacks from the current queue and returns their count.
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
    pub(crate) fn complete(&self, result: Result<()>) {
        self.finished.borrow_mut().push_back(result);
    }

    /// Retrieves the oldest terminal result when available.
    pub(crate) fn take_completion(&self) -> Option<Result<()>> {
        self.finished.borrow_mut().pop_front()
    }

    /// Polls pending work and waits for a wakeup until a route completes or the wait times out.
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

impl<M: 'static> SpawnWorker for ChannelDriver<M> {
    type Work<T: Send + 'static> = Wait<T>;

    fn spawn<I, O>(&self, work: fn(I) -> Result<O>, input: I) -> Result<Wait<O>>
    where
        I: Send + 'static,
        O: Send + 'static,
    {
        let (signal, wait) = self.signal();
        thread::Builder::new().spawn(move || signal.complete(work(input)))?;
        Ok(wait)
    }
}

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

/// Polls once and returns a job only if the future still needs its driver.
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
