// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Issue and resume one instruction using a machine lent by the runtime.

use super::tasks::{Submit, TaskId, Ticket, TimedSubmit};
use crate::{
    framework::{
        EdgeId, FutureDriver, Metadata, Operation, Route, RouteDelivery, Run,
        synchronous::{StartCallback, StartRoute},
    },
    timing::minimum_execution,
};
use eyre::{Result, ensure, eyre};
use std::{
    any::TypeId,
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    future::Future,
    ops::ControlFlow,
    rc::Rc,
};
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

type Resume<M, B> = Box<dyn FnOnce(&mut M, &RouteContext<M, B>)>;

// Shared only by pending continuations for one in-flight instruction.
type ActiveRoute = Rc<Cell<u32>>;

struct Parked<M, B> {
    machine: usize,
    route: ActiveRoute,
    resume: Resume<M, B>,
    edge_delays: Option<Rc<[u64]>>,
}

type PendingRoutes<M, B> = Rc<RefCell<HashMap<TaskId, Parked<M, B>>>>;

/// Gives one invocation access to the program driver’s scheduling services.
pub(crate) struct RouteContext<M, B> {
    machine: usize,
    submitter: B,
    pending: PendingRoutes<M, B>,
    route: OnceCell<ActiveRoute>,
    terminal: RefCell<Option<Result<()>>>,
    edge_delays: Option<Rc<[u64]>>,
}

impl<M: 'static, B: Submit> RouteContext<M, B> {
    fn new(
        machine: usize,
        submitter: &B,
        pending: &PendingRoutes<M, B>,
        route: Option<ActiveRoute>,
        edge_delays: Option<Rc<[u64]>>,
    ) -> Self {
        Self {
            machine,
            submitter: submitter.clone(),
            pending: Rc::clone(pending),
            route: route.map(OnceCell::from).unwrap_or_default(),
            terminal: RefCell::default(),
            edge_delays,
        }
    }

    fn has_pending_work(&self) -> bool {
        self.route.get().is_some_and(|route| {
            self.pending
                .borrow()
                .values()
                .any(|task| Rc::ptr_eq(&task.route, route))
        })
    }
}

impl<M: 'static, B: Submit> FutureDriver<M> for RouteContext<M, B> {
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

impl<M: 'static, B: TimedSubmit> RouteDelivery<M> for RouteContext<M, B> {
    fn deliver_edge<E: 'static>(
        &self,
        machine: &mut M,
        id: EdgeId,
        effect: E,
        done: impl FnOnce(&mut M, &Self, Result<E>) + 'static,
    ) {
        let ticks = self
            .edge_delays
            .as_ref()
            .and_then(|table| table.get(id.0))
            .copied()
            .unwrap_or(0);
        if ticks == 0 {
            done(machine, self, Ok(effect));
        } else {
            let ticket = self
                .submitter
                .deadline(ticks)
                .and_then(|deadline| self.submitter.submit_at(deadline, effect));
            self.register(machine, ticket, done);
        }
    }

    fn run_annotated<I, N: Operation<M, I, Self, Output: 'static>>(
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

impl<M: 'static, B: Submit> RouteContext<M, B> {
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
                route: Rc::clone(self.route.get_or_init(Rc::default)),
                edge_delays: self.edge_delays.clone(),
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
    pub(crate) fn start<I>(
        &self,
        machine: &mut M,
        route: &impl StartRoute<M, I, Module<M>, RouteContext<M, B>>,
        instruction: I,
    ) -> Result<()> {
        route.start(machine, instruction, self.module, self)
    }
}

impl<M, B> StartCallback<M, RouteContext<M, B>> for StepContext<'_, M, B>
where
    M: HasInstructionSet + HasProgram<Program = ExecutionState<Pause>> + 'static,
    B: Submit,
{
    fn start_callback<I, P, Mode, Input>(
        &self,
        machine: &mut M,
        route: &Route<I, P, Mode>,
        input: Input,
    ) -> Result<()>
    where
        I: 'static,
        P: Run<M, Input, RouteContext<M, B>, Output = ()> + 'static,
    {
        // Inline routes keep completion on this stack frame.
        let edge_delays = if P::NAMED_EDGES == 0 {
            None
        } else {
            self.submitter.edge_delays(TypeId::of::<(I, P)>())
        };
        let route_context = RouteContext::new(
            self.machine,
            self.submitter,
            self.pending,
            None,
            edge_delays,
        );
        route.start_with(
            machine,
            &route_context,
            input,
            |_, route_context, result| {
                *route_context.terminal.borrow_mut() = Some(result);
            },
        );
        let result = route_context.terminal.borrow_mut().take();
        if let Some(result) = result {
            return result;
        }
        ensure!(
            route_context.has_pending_work(),
            "route started without pending work"
        );
        // Checked fetch will restore the originating PC when it sees the pause.
        route_context
            .route
            .get()
            .expect("pending work has a route")
            .set(machine.program().pc());
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
        let route = parked.route;
        let route_context = RouteContext::new(
            parked.machine,
            &self.submitter,
            &self.pending,
            Some(Rc::clone(&route)),
            parked.edge_delays,
        );
        let machine = loaded.machine_mut();
        let origin = machine.program().pc();
        ensure!(
            matches!(machine.program().run_state(), RunState::Other(Pause::Route)),
            "task owner is not waiting for a route"
        );
        let continuation_pc = route.get();
        let result = run(
            machine,
            |machine| {
                machine.program_mut().resume_at(continuation_pc);
                (parked.resume)(machine, &route_context);
                let result = route_context.terminal.borrow_mut().take();
                if let Some(result) = result {
                    return result;
                }
                ensure!(
                    route_context.has_pending_work(),
                    "route returned without completion or pending work"
                );
                // Another stage parked. Keep the same visible origin until retirement.
                route.set(machine.program().pc());
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
