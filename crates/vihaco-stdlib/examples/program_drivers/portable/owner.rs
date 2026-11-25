// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The machine owner records route continuations separately from scheduled tasks.

use super::{
    digitizer::{Instruction, Machine, Module, Pause},
    tasks::{Submit, TaskId},
};
use crate::framework::{FutureDriver, Route, Run};
use eyre::{Result, ensure, eyre};
use std::{cell::RefCell, collections::HashMap, future::Future, ops::ControlFlow, rc::Rc};
use vihaco_abi::word::Word;
use vihaco_runtime::HasProgram;
use vihaco_stdlib::{
    Image, Loadable,
    execution_state::{RunState, execute_instruction_with_context, run},
};

type Resume<B> = Box<dyn FnOnce(&mut Machine, &RouteDriver<B>)>;

struct RouteState {
    continuation_pc: u32,
    terminal: Option<Result<()>>,
}

type ActiveRoute = Rc<RefCell<RouteState>>;

struct Parked<B> {
    machine: usize,
    route: ActiveRoute,
    resume: Resume<B>,
}

type PendingRoutes<B> = Rc<RefCell<HashMap<TaskId, Parked<B>>>>;

/// Bridges the current route trait to the machine-independent scheduler.
pub(crate) struct RouteDriver<B> {
    machine: usize,
    submitter: B,
    pending: PendingRoutes<B>,
    route: ActiveRoute,
}

impl<B: Submit> RouteDriver<B> {
    fn new(machine: usize, submitter: &B, pending: &PendingRoutes<B>, route: ActiveRoute) -> Self {
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

impl<B: Submit> FutureDriver<Machine> for RouteDriver<B> {
    fn run_future<T: 'static>(
        &self,
        machine: &mut Machine,
        future: impl Future<Output = Result<T>> + 'static,
        done: impl FnOnce(&mut Machine, &Self, Result<T>) + 'static,
    ) {
        let ticket = match self.submitter.submit(future) {
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

pub(crate) struct StepContext<'a, B> {
    machine: usize,
    submitter: &'a B,
    pending: &'a PendingRoutes<B>,
    pub(crate) module: &'a Module,
}

impl<B: Submit> StepContext<'_, B> {
    pub(crate) fn start<I, P>(
        &self,
        machine: &mut Machine,
        route: &Route<I, P>,
        instruction: I,
    ) -> Result<()>
    where
        P: Run<Machine, I, RouteDriver<B>, Output = ()>,
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

pub(crate) type Loaded = Loadable<Image<Instruction>, Machine>;

pub(crate) struct Owner<B> {
    pub(crate) machines: Vec<Loaded>,
    submitter: B,
    pending: PendingRoutes<B>,
}

impl<B: Submit> Owner<B> {
    pub(crate) fn new(machines: Vec<Loaded>, submitter: B) -> Self {
        Self {
            machines,
            submitter,
            pending: Rc::default(),
        }
    }

    pub(crate) fn enter(&mut self, machine: usize, args: &[Word]) -> Result<()> {
        self.machines[machine].start(args)
    }

    /// Fetch and dispatch at most one instruction from the selected program.
    pub(crate) fn issue_one(&mut self, machine_id: usize) -> Result<()> {
        let (image, machine) = self.machines[machine_id].parts_mut();
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

    /// Deliver one result under the execution guard and return before fetching again.
    pub(crate) fn resume(&mut self, task: TaskId) -> Result<usize> {
        let parked = self
            .pending
            .borrow_mut()
            .remove(&task)
            .ok_or_else(|| eyre!("no parked route for {task:?}"))?;
        let driver = RouteDriver::new(parked.machine, &self.submitter, &self.pending, parked.route);
        let machine = self.machines[parked.machine].machine_mut();
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
        Ok(parked.machine)
    }
}
