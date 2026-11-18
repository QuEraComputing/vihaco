// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The driver keeps all per-invocation state, including parked continuations.

use eyre::{Result, eyre};
use std::{
    cell::RefCell,
    collections::VecDeque,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::Duration,
};

/// A one-shot continuation that receives a node or path result.
pub(crate) type Done<M, T> = Box<dyn FnOnce(&mut M, &Driver<M>, Result<T>)>;

/// Checks one parked continuation and reports whether it completed.
type Poll<M> = Box<dyn FnMut(&mut M, &Driver<M>) -> bool>;

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

/// Owns the parked continuations, wake channel, and terminal outcomes.
pub(crate) struct Driver<M> {
    wake: Receiver<()>,
    sender: Sender<()>,
    pending: RefCell<Vec<Poll<M>>>,
    finished: RefCell<VecDeque<Result<()>>>,
}

// Runs ready callbacks on the driver thread with a fresh borrow of the machine.
impl<M: 'static> Driver<M> {
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

    /// Runs the supplied work on a standard-library thread.
    pub(crate) fn spawn<I, O>(&self, work: fn(I) -> Result<O>, input: I) -> Result<Wait<O>>
    where
        I: Send + 'static,
        O: Send + 'static,
    {
        let (signal, wait) = self.signal();
        thread::Builder::new().spawn(move || signal.complete(work(input)))?;
        Ok(wait)
    }

    /// Retains a callback and wait endpoint until a result is available.
    pub(crate) fn park<T: 'static>(&self, wait: Wait<T>, done: Done<M, T>) {
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
