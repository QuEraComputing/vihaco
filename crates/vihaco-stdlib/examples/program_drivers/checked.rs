// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Checked issue and retirement shared by all five scenarios.

use crate::framework::{Route, Run};
use eyre::{Result, ensure, eyre};
use std::{cell::RefCell, collections::BTreeSet, convert::Infallible, ops::ControlFlow, rc::Rc};
use vihaco_abi::{Type, Value, word::Word};
use vihaco_module::module::LocalModule;
use vihaco_runtime::{Composite, Dispatch, HasProgram};
use vihaco_stdlib::{
    ExecutionState, Image, Loadable, RunState,
    execution_state::{execute_instruction_with_context, run},
    program_driver::{ProgramDriver, ProgramStop, RunCommand},
};

pub(super) type Module<I> = LocalModule<I, Value, Type>;
type Completion = Rc<RefCell<Option<Result<()>>>>;
type Stop = ProgramStop<u64>;

/// A backend delivers a readiness event with a fresh machine borrow.
pub(super) trait Backend<M> {
    type Delivery;

    /// Apply one delivery. An error means the backend left the route parked.
    fn deliver(&mut self, machine: &mut M, delivery: Self::Delivery) -> Result<()>;
}

/// Drives an inline program with no backend deliveries.
pub(super) struct Inline;

impl<M> Backend<M> for Inline {
    type Delivery = Infallible;

    fn deliver(&mut self, _: &mut M, delivery: Infallible) -> Result<()> {
        match delivery {}
    }
}

/// ExecutionState owns this issue gate, while the driver owns the pending route.
#[derive(Debug)]
pub enum Pause {
    Route,
    Faulted,
}

struct PendingRoute {
    continuation_pc: u32,
    completion: Completion,
}

/// Per-dispatch services passed to authored Steps alongside the loaded module.
pub(super) struct StepContext<'a, I, B> {
    pub(super) module: &'a Module<I>,
    backend: &'a B,
    pending: RefCell<Option<PendingRoute>>,
}

impl<I, B> StepContext<'_, I, B> {
    pub(super) fn start<M, Inst, P>(
        &self,
        machine: &mut M,
        route: &Route<Inst, P>,
        instruction: Inst,
    ) -> Result<()>
    where
        M: HasProgram<Program = ExecutionState<Pause>>,
        P: Run<M, Inst, B, Output = ()>,
    {
        ensure!(
            self.pending.borrow().is_none(),
            "a route is already pending"
        );
        let completion = Completion::default();
        let slot = Rc::clone(&completion);
        route.start_with(machine, self.backend, instruction, move |_, _, result| {
            *slot.borrow_mut() = Some(result);
        });
        let result = completion.borrow_mut().take();
        if let Some(result) = result {
            return result;
        }

        let program = machine.program_mut();
        *self.pending.borrow_mut() = Some(PendingRoute {
            continuation_pc: program.pc(),
            completion,
        });
        // Checked fetch restores the originating PC when it sees this gate.
        program.suspend(Pause::Route);
        Ok(())
    }
}

struct Parked {
    origin: u32,
    work: u64,
    command: RunCommand,
    route: PendingRoute,
}

impl Parked {
    fn stop(&self) -> Stop {
        Stop::Pending {
            pc: self.origin,
            work: self.work,
        }
    }
}

/// Runs a loaded machine with a route backend. A fleet can own several drivers.
pub(super) struct CheckedDriver<M: Composite, I, B> {
    loaded: Loadable<Image<I>, M>,
    backend: B,
    pending: Option<Parked>,
    last_work: u64,
    breakpoints: BTreeSet<u32>,
    at_breakpoint: Option<u32>,
    retirements: usize,
}

impl<M: Composite, I, B> CheckedDriver<M, I, B> {
    pub(super) fn new(loaded: Loadable<Image<I>, M>, backend: B) -> Self {
        Self {
            loaded,
            backend,
            pending: None,
            last_work: 0,
            breakpoints: BTreeSet::new(),
            at_breakpoint: None,
            retirements: 0,
        }
    }

    pub(super) fn set_breakpoint(&mut self, pc: u32) {
        self.breakpoints.insert(pc);
    }

    pub(super) fn machine(&self) -> &M {
        self.loaded.machine()
    }

    pub(super) fn backend(&self) -> &B {
        &self.backend
    }

    pub(super) fn retirements(&self) -> usize {
        self.retirements
    }
}

impl<M: Composite, I, B> CheckedDriver<M, I, B>
where
    Self: ProgramDriver<MachineId = (), WorkId = u64, Delivery = ()>,
{
    /// Run the loaded program to halt, letting the backend drive pending work.
    ///
    /// This convenience entry point is for backends that can advance without
    /// a host-supplied payload. A breakpoint returns an error at its stop PC.
    pub(super) fn execute(&mut self, arguments: &[Word]) -> Result<()> {
        self.start((), arguments)?;
        let mut stop = self.run((), RunCommand::Continue)?;
        loop {
            stop = match stop {
                ProgramStop::Pending { work, .. } => self.deliver((), work, ())?,
                ProgramStop::Halted { .. } => return Ok(()),
                ProgramStop::Breakpoint { pc } => return Err(eyre!("breakpoint at PC {pc}")),
                ProgramStop::Retired { pc } => {
                    return Err(eyre!("continuous execution stopped after retiring PC {pc}"));
                }
            };
        }
    }
}

fn next_work(last: &mut u64) -> Result<u64> {
    *last = last
        .checked_add(1)
        .ok_or_else(|| eyre!("work ID exhausted"))?;
    Ok(*last)
}

impl<M, I, B> ProgramDriver for CheckedDriver<M, I, B>
where
    M: HasProgram<Program = ExecutionState<Pause>>,
    M: for<'a> Dispatch<StepContext<'a, I, B>, Instruction = I>,
    B: Backend<M>,
{
    type MachineId = ();
    type WorkId = u64;
    type Delivery = B::Delivery;

    fn start(&mut self, (): (), arguments: &[Word]) -> Result<()> {
        self.loaded.start(arguments)?;
        self.at_breakpoint = None;
        self.retirements = 0;
        Ok(())
    }

    fn run(&mut self, (): (), command: RunCommand) -> Result<Stop> {
        if let Some(parked) = &self.pending {
            return Ok(parked.stop());
        }
        let (image, machine) = self.loaded.parts_mut();
        let module = &image.module;
        ensure!(
            !matches!(machine.program().run_state(), RunState::Other(_)),
            "program is faulted or has no pending route"
        );

        let mut stop = None;
        let result = run(
            machine,
            |_| Ok(()),
            |machine, ()| {
                let origin = machine.program().pc();
                // A new command can pass the breakpoint where the driver just stopped.
                if self.breakpoints.contains(&origin) && self.at_breakpoint.take() != Some(origin) {
                    self.at_breakpoint = Some(origin);
                    stop = Some(Stop::Breakpoint { pc: origin });
                    return Ok(ControlFlow::Break(()));
                }

                let context = StepContext {
                    module,
                    backend: &self.backend,
                    pending: RefCell::new(None),
                };
                execute_instruction_with_context(module, machine, None, &context)?;
                if let Some(route) = context.pending.into_inner() {
                    let parked = Parked {
                        origin,
                        work: next_work(&mut self.last_work)?,
                        command,
                        route,
                    };
                    stop = Some(parked.stop());
                    self.pending = Some(parked);
                    return Ok(ControlFlow::Break(()));
                }

                ensure!(
                    !matches!(machine.program().run_state(), RunState::Other(_)),
                    "step suspended without a parked route"
                );
                self.retirements += 1;
                if machine.program().is_halted() {
                    stop = Some(Stop::Halted { pc: origin });
                    Ok(ControlFlow::Break(()))
                } else if command == RunCommand::SingleStep {
                    stop = Some(Stop::Retired { pc: origin });
                    Ok(ControlFlow::Break(()))
                } else {
                    Ok(ControlFlow::Continue(()))
                }
            },
        );
        if let Err(error) = result {
            machine.program_mut().suspend(Pause::Faulted);
            return Err(error);
        }
        Ok(stop.unwrap_or_else(|| Stop::Halted {
            pc: machine.program().pc(),
        }))
    }

    fn deliver(&mut self, (): (), work: u64, delivery: B::Delivery) -> Result<Stop> {
        let parked = self
            .pending
            .as_mut()
            .ok_or_else(|| eyre!("no work is pending"))?;
        ensure!(parked.work == work, "stale work ID");
        let origin = parked.origin;
        let mut result = None;

        run(
            self.loaded.machine_mut(),
            |machine| {
                machine
                    .program_mut()
                    .resume_at(parked.route.continuation_pc);
                let delivered = self.backend.deliver(machine, delivery);
                result = parked.route.completion.borrow_mut().take();
                if result.is_none() {
                    let program = machine.program_mut();
                    parked.route.continuation_pc = program.pc();
                    program.resume_at(origin);
                    program.suspend(Pause::Route);
                }
                delivered
            },
            |_, ()| Ok(ControlFlow::Break(())),
        )?;

        match result {
            None => {
                parked.work = next_work(&mut self.last_work)?;
                Ok(parked.stop())
            }
            Some(result) => {
                let command = parked.command;
                self.pending = None;
                let program = self.loaded.machine_mut().program_mut();
                if let Err(error) = result {
                    program.resume_at(origin);
                    program.suspend(Pause::Faulted);
                    return Err(error);
                }
                self.retirements += 1;
                if program.is_halted() {
                    Ok(Stop::Halted { pc: origin })
                } else if command == RunCommand::SingleStep {
                    Ok(Stop::Retired { pc: origin })
                } else {
                    self.run((), command)
                }
            }
        }
    }
}
