// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A ProgramDriver that routes commands to thread-local Tokio drivers.

use crate::{
    machine::{Machine, with_machine},
    tokio_driver::TokioDriver,
};
use eyre::{Result, eyre};
use std::{
    sync::Arc,
    thread::{self, JoinHandle, ThreadId},
};
use tokio::sync::{Barrier, mpsc, oneshot};
use vihaco_abi::word::Word;
use vihaco_stdlib::program_driver::{ProgramDriver, ProgramStop, RunCommand};

type Stop = ProgramStop<u64>;
type Reply<T> = oneshot::Sender<Result<T>>;

enum Command {
    Start(Vec<Word>, Reply<()>),
    Run(RunCommand, Reply<Stop>),
    Deliver(u64, Reply<Stop>),
    Inspect(Reply<(ThreadId, Vec<Word>)>),
}

struct Worker {
    commands: mpsc::Sender<Command>,
    thread: JoinHandle<Result<()>>,
}

/// A machine key indexes one owner thread and its Tokio LocalSet.
pub(super) struct TokioThreads {
    workers: Vec<Worker>,
}

impl TokioThreads {
    pub(super) fn new(samples: &[Word], gate: Option<Arc<Barrier>>) -> Result<Self> {
        let mut fleet = Self {
            workers: Vec::new(),
        };
        for (index, &sample) in samples.iter().enumerate() {
            let (commands, mut incoming) = mpsc::channel(8);
            let gate = gate.clone();
            let thread = thread::Builder::new()
                .name(format!("vihaco-tokio-{index}"))
                .spawn(move || {
                    // Construct the machine and LocalSet on the owner thread.
                    let machine = Machine {
                        tokio_gate: gate,
                        ..Machine::default()
                    };
                    let mut driver = with_machine(machine, TokioDriver::new()?, sample);
                    while let Some(command) = incoming.blocking_recv() {
                        match command {
                            Command::Start(args, reply) => {
                                let _ = reply.send(driver.start((), &args));
                            }
                            Command::Run(mode, reply) => {
                                let _ = reply.send(driver.run((), mode));
                            }
                            Command::Deliver(work, reply) => {
                                let _ = reply.send(driver.deliver((), work, ()));
                            }
                            Command::Inspect(reply) => {
                                let _ = reply.send(Ok((
                                    thread::current().id(),
                                    driver.machine().execution.stack().to_vec(),
                                )));
                            }
                        }
                    }
                    Ok(())
                })?;
            fleet.workers.push(Worker { commands, thread });
        }
        Ok(fleet)
    }

    fn submit<T>(
        &self,
        machine: usize,
        command: impl FnOnce(Reply<T>) -> Command,
    ) -> Result<oneshot::Receiver<Result<T>>> {
        let worker = self
            .workers
            .get(machine)
            .ok_or_else(|| eyre!("unknown machine {machine}"))?;
        let (send, receive) = oneshot::channel();
        worker
            .commands
            .blocking_send(command(send))
            .map_err(|_| eyre!("machine {machine} stopped"))?;
        Ok(receive)
    }

    fn receive<T>(response: oneshot::Receiver<Result<T>>) -> Result<T> {
        response
            .blocking_recv()
            .map_err(|_| eyre!("machine dropped its response"))?
    }

    pub(super) fn inspect(&self, machine: usize) -> Result<(ThreadId, Vec<Word>)> {
        Self::receive(self.submit(machine, Command::Inspect)?)
    }

    /// Authorize all deliveries before waiting for any, allowing parallel workers.
    pub(super) fn deliver_together(&self, work: &[(usize, u64)]) -> Result<Vec<Stop>> {
        let responses = work
            .iter()
            .map(|&(machine, work)| self.submit(machine, |reply| Command::Deliver(work, reply)))
            .collect::<Result<Vec<_>>>()?;
        responses.into_iter().map(Self::receive).collect()
    }
}

impl ProgramDriver for TokioThreads {
    type MachineId = usize;
    type WorkId = u64;
    type Delivery = ();

    fn start(&mut self, machine: usize, arguments: &[Word]) -> Result<()> {
        Self::receive(self.submit(machine, |reply| Command::Start(arguments.to_vec(), reply))?)
    }

    fn run(&mut self, machine: usize, mode: RunCommand) -> Result<Stop> {
        Self::receive(self.submit(machine, |reply| Command::Run(mode, reply))?)
    }

    fn deliver(&mut self, machine: usize, work: u64, (): ()) -> Result<Stop> {
        Self::receive(self.submit(machine, |reply| Command::Deliver(work, reply))?)
    }
}

impl Drop for TokioThreads {
    fn drop(&mut self) {
        for Worker { commands, thread } in self.workers.drain(..) {
            drop(commands);
            let _ = thread.join();
        }
    }
}
