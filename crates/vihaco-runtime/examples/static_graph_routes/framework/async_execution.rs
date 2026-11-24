// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! # Async instructions and component leases
//!
//! An `AsyncExecute` future holds a mutable borrow of its component across
//! suspension. The async node takes the component out of its `AsyncSlot` and
//! places it in an exclusive lease. The future owns the lease, instruction,
//! and message. The machine keeps the slot and stays available between
//! polls. An instruction that tries to lease the same occupied field while
//! it is pending receives an error.
//!
//! When the future finishes, it drops the lease before forwarding success
//! or failure. The next node can use the restored field, as the digitizer's
//! same-field receive demonstrates. Dropping a pending future also restores
//! the component. This is local ownership recovery. Cancellation policy for
//! external work belongs to the backend or machine owner.

use super::{
    graph::{Label, NodePlan, node},
    operations::{Operation, Receive},
    runtime::FutureDriver,
};
use eyre::{Result, eyre};
use std::{any::type_name, cell::RefCell, marker::PhantomData, rc::Rc};
use vihaco_runtime::Field;

pub(crate) trait AsyncExecute<I> {
    type Message;
    type Effect;
    type Error;

    async fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> std::result::Result<Self::Effect, Self::Error>;
}

// The route owns the component while pending. The machine retains this slot.
pub(crate) struct AsyncSlot<D>(Rc<RefCell<Option<D>>>);

impl<D> AsyncSlot<D> {
    pub(crate) fn new(component: D) -> Self {
        Self(Rc::new(RefCell::new(Some(component))))
    }

    pub(crate) fn inspect<R>(&self, read: impl FnOnce(&D) -> R) -> Option<R> {
        self.0.borrow().as_ref().map(read)
    }

    fn lease(&self) -> Result<Lease<D>> {
        let component = self
            .0
            .borrow_mut()
            .take()
            .ok_or_else(|| eyre!("component already leased"))?;
        Ok(Lease {
            slot: Rc::clone(&self.0),
            component: Some(component),
        })
    }
}

impl<D: Receive<E>, E> Receive<E> for AsyncSlot<D> {
    fn receive(&mut self, effect: E) -> Result<()> {
        self.0
            .borrow_mut()
            .as_mut()
            .ok_or_else(|| eyre!("component still leased"))?
            .receive(effect)
    }
}

struct Lease<D> {
    slot: Rc<RefCell<Option<D>>>,
    component: Option<D>,
}

impl<D> Lease<D> {
    fn get_mut(&mut self) -> &mut D {
        self.component
            .as_mut()
            .expect("live lease owns the component")
    }
}

impl<D> Drop for Lease<D> {
    fn drop(&mut self) {
        *self.slot.borrow_mut() = self.component.take();
    }
}

pub(crate) struct AsyncInstructionAt<F>(PhantomData<fn() -> F>);
pub(crate) struct AsyncExecuteAt<F>(PhantomData<fn() -> F>);

pub(crate) fn async_instruction_at<F>() -> NodePlan<AsyncInstructionAt<F>> {
    node()
}

impl<F> Label for AsyncExecuteAt<F> {
    const LABEL: &'static str = "execute async instruction";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

impl<M, F, I, D, Backend> Operation<M, (I, D::Message), Backend> for AsyncExecuteAt<F>
where
    M: Field<F, Child = AsyncSlot<D>> + 'static,
    I: 'static,
    Backend: FutureDriver<M>,
    D: AsyncExecute<I, Message: 'static, Effect: 'static, Error: Into<eyre::Report>> + 'static,
{
    type Output = D::Effect;

    fn start(
        machine: &mut M,
        driver: &Backend,
        (instruction, message): (I, D::Message),
        done: impl FnOnce(&mut M, &Backend, Result<Self::Output>) + 'static,
    ) {
        match machine.get().lease() {
            Ok(mut lease) => {
                let future = async move {
                    let result = lease.get_mut().execute(&instruction, message).await;
                    drop(lease);
                    result.map_err(Into::into)
                };
                driver.run_future(machine, future, done);
            }
            Err(error) => done(machine, driver, Err(error)),
        }
    }
}
