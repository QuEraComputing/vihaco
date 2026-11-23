// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Async instructions that borrow an exclusively leased component.

use super::{
    graph::{Label, NodePlan, node},
    operations::{Operation, Receive},
    runtime::{Await, FutureDriver, RustFuture},
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
                driver.wait(machine, RustFuture(future), done);
            }
            Err(error) => done(machine, driver, Err(error)),
        }
    }
}
