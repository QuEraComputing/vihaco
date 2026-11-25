// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Issue and resume one instruction using a machine lent by the runtime.

use super::tasks::{Submit, TaskId, Ticket, TimedSubmit};
use crate::{
    framework::{AnnotatedDriver, FutureDriver, Metadata, Operation, Route, Run},
    timing::minimum_execution,
};
use eyre::{Result, ensure, eyre};
use std::{cell::RefCell, collections::HashMap, future::Future, ops::ControlFlow, rc::Rc};
use vihaco_abi::{Type, Value, word::Word};
use vihaco_module::module::LocalModule;
use vihaco_runtime::{Composite, Dispatch, HasInstructionSet, HasProgram};
use vihaco_stdlib::{
    ExecutionState, Image, Loadable,
    execution_state::{RunState, execute_instruction_with_context, run},
};

pub(crate) type Module<M> = LocalModule<<M as HasInstructionSet>::Runtime, Value, Type>;
pub(crate) type Loaded<M> = Loadable<Image<<M as HasInstructionSet>::Runtime>, M>;

#[derive(Debug)]
pub(crate) enum Pause {
    Route,
    Faulted,
}

type Resume<M, B> = Box<dyn FnOnce(&mut M, &RouteDriver<M, B>)>;

struct RouteState {
    continuation_pc: u32,
    terminal: Option<Result<()>>,
}

type ActiveRoute = Rc<RefCell<RouteState>>;

struct Parked<M, B> {
    machine: usize,
    route: ActiveRoute,
    resume: Resume<M, B>,
}

type PendingRoutes<M, B> = Rc<RefCell<HashMap<TaskId, Parked<M, B>>>>;

/// Bridges the current route trait to the machine-independent scheduler.
pub(crate) struct RouteDriver<M, B> {
    machine: usize,
    submitter: B,
    pending: PendingRoutes<M, B>,
    route: ActiveRoute,
}

impl<M: 'static, B: Submit> RouteDriver<M, B> {
    fn new(
        machine: usize,
        submitter: &B,
        pending: &PendingRoutes<M, B>,
        route: ActiveRoute,
    ) -> Self {
        Self {
            machine,
            submitter: submitter.clone(),
            pending: Rc::clone(pending),
            route,
        }
    }

    fn has_pending_work(&self) -> bool {
        self.pending
            .borrow()
            .values()
            .any(|task| Rc::ptr_eq(&task.route, &self.route))
    }
}

impl<M: 'static, B: Submit> FutureDriver<M> for RouteDriver<M, B> {
    fn run_future<T: 'static>(
        &self,
        machine: &mut M,
        future: impl Future<Output = Result<T>> + 'static,
        done: impl FnOnce(&mut M, &Self, Result<T>) + 'static,
    ) {
        let ticket = self.submitter.submit(future);
        self.register(machine, ticket, done);
    }
}

impl<M: 'static, B: TimedSubmit> AnnotatedDriver<M> for RouteDriver<M, B> {
    fn run_annotated<I, N: Operation<M, I, Self>>(
        &self,
        machine: &mut M,
        metadata: &Metadata,
        input: I,
        done: impl FnOnce(&mut M, &Self, Result<N::Output>) + 'static,
    ) {
        let ticks = minimum_execution(metadata);
        if ticks == 0 {
            return N::start(machine, self, input, done);
        }

        let deadline = match self.submitter.deadline(ticks) {
            Ok(deadline) => deadline,
            Err(error) => return done(machine, self, Err(error)),
        };
        N::start(
            machine,
            self,
            input,
            move |machine, driver, result| match result {
                Ok(value) => {
                    let ticket = driver.submitter.submit_at(deadline, value);
                    driver.register(machine, ticket, done);
                }
                Err(error) => done(machine, driver, Err(error)),
            },
        );
    }
}

impl<M: 'static, B: Submit> RouteDriver<M, B> {
    fn register<T: 'static>(
        &self,
        machine: &mut M,
        ticket: Result<Ticket<T>>,
        done: impl FnOnce(&mut M, &Self, Result<T>) + 'static,
    ) {
        let ticket = match ticket {
            Ok(ticket) => ticket,
            Err(error) => return done(machine, self, Err(error)),
        };
        self.pending.borrow_mut().insert(
            ticket.id(),
            Parked {
                machine: self.machine,
                route: Rc::clone(&self.route),
                resume: Box::new(move |machine, driver| {
                    done(machine, driver, ticket.take_result())
                }),
            },
        );
    }
}

pub(crate) struct StepContext<'a, M: HasInstructionSet, B> {
    machine: usize,
    submitter: &'a B,
    pending: &'a PendingRoutes<M, B>,
    pub(crate) module: &'a Module<M>,
}

impl<M, B> StepContext<'_, M, B>
where
    M: HasInstructionSet + HasProgram<Program = ExecutionState<Pause>> + 'static,
    B: Submit,
{
    pub(crate) fn start<I, P>(
        &self,
        machine: &mut M,
        route: &Route<I, P>,
        instruction: I,
    ) -> Result<()>
    where
        P: Run<M, I, RouteDriver<M, B>, Output = ()>,
    {
        let state = Rc::new(RefCell::new(RouteState {
            continuation_pc: machine.program().pc(),
            terminal: None,
        }));
        let driver = RouteDriver::new(
            self.machine,
            self.submitter,
            self.pending,
            Rc::clone(&state),
        );
        let terminal = Rc::clone(&state);
        route.start_with(machine, &driver, instruction, move |_, _, result| {
            terminal.borrow_mut().terminal = Some(result);
        });
        let result = state.borrow_mut().terminal.take();
        if let Some(result) = result {
            return result;
        }
        ensure!(
            driver.has_pending_work(),
            "route started without pending work"
        );
        // Checked fetch will restore the originating PC when it sees the pause.
        state.borrow_mut().continuation_pc = machine.program().pc();
        machine.program_mut().suspend(Pause::Route);
        Ok(())
    }
}

pub(crate) struct Driver<M, B> {
    submitter: B,
    pending: PendingRoutes<M, B>,
}

impl<M, B> Driver<M, B>
where
    M: Composite + HasInstructionSet + HasProgram<Program = ExecutionState<Pause>> + 'static,
    M: for<'a> Dispatch<StepContext<'a, M, B>, Instruction = M::Runtime>,
    B: Submit,
{
    pub(crate) fn new(submitter: B) -> Self {
        Self {
            submitter,
            pending: Rc::default(),
        }
    }

    pub(crate) fn enter(&self, loaded: &mut Loaded<M>, args: &[Word]) -> Result<()> {
        loaded.start(args)
    }

    /// Fetch and dispatch one instruction from the selected program.
    pub(crate) fn issue_one(&self, machine_id: usize, loaded: &mut Loaded<M>) -> Result<()> {
        let (image, machine) = loaded.parts_mut();
        let context = StepContext {
            machine: machine_id,
            submitter: &self.submitter,
            pending: &self.pending,
            module: &image.module,
        };
        let result = run(
            machine,
            |_| Ok(()),
            |machine, ()| {
                execute_instruction_with_context(&image.module, machine, None, &context)?;
                Ok(ControlFlow::Break(()))
            },
        );
        if result.is_err() {
            machine.program_mut().suspend(Pause::Faulted);
        }
        result
    }

    pub(crate) fn waiting_machine(&self, task: TaskId) -> Result<usize> {
        self.pending
            .borrow()
            .get(&task)
            .map(|parked| parked.machine)
            .ok_or_else(|| eyre!("no parked route for {task:?}"))
    }

    /// Deliver one result under the execution guard and return before fetching again.
    pub(crate) fn resume(&self, task: TaskId, loaded: &mut Loaded<M>) -> Result<()> {
        let parked = self
            .pending
            .borrow_mut()
            .remove(&task)
            .ok_or_else(|| eyre!("no parked route for {task:?}"))?;
        let driver = RouteDriver::new(parked.machine, &self.submitter, &self.pending, parked.route);
        let machine = loaded.machine_mut();
        let origin = machine.program().pc();
        ensure!(
            matches!(machine.program().run_state(), RunState::Other(Pause::Route)),
            "task owner is not waiting for a route"
        );
        let continuation_pc = driver.route.borrow().continuation_pc;
        let result = run(
            machine,
            |machine| {
                machine.program_mut().resume_at(continuation_pc);
                (parked.resume)(machine, &driver);
                let result = driver.route.borrow_mut().terminal.take();
                if let Some(result) = result {
                    return result;
                }
                ensure!(
                    driver.has_pending_work(),
                    "route returned without completion or pending work"
                );
                // Another stage parked. Keep the same visible origin until retirement.
                driver.route.borrow_mut().continuation_pc = machine.program().pc();
                machine.program_mut().resume_at(origin);
                machine.program_mut().suspend(Pause::Route);
                Ok(())
            },
            |_, ()| Ok(ControlFlow::Break(())),
        );
        if let Err(error) = result {
            machine.program_mut().resume_at(origin);
            machine.program_mut().suspend(Pause::Faulted);
            return Err(error);
        }
        Ok(())
    }
}
