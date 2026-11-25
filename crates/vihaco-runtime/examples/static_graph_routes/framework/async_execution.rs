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
//! same-field effect application demonstrates. Dropping a pending future
//! also restores the component. Cancellation policy for
//! external work belongs to the backend or machine owner.

use super::{
    graph::{Label, NodePlan, node},
    operations::{Operation, Prepared},
    runtime::FutureDriver,
};
use eyre::{Result, eyre};
use std::{any::type_name, cell::RefCell, marker::PhantomData, rc::Rc};
use vihaco_runtime::{Apply, Field};

/// Executes an instruction while borrowing its component across suspension.
///
/// The route submits the future to a local driver. The component's exclusive
/// lease lets the driver release its machine borrow between polls.
pub(crate) trait AsyncExecute<I> {
    /// Invocation data prepared before the async execution node.
    type Message;
    /// The payload sent to the successor after the component is restored.
    type Effect;
    /// The failure reported by the component, converted to an eyre report by the node.
    type Error;

    /// Runs one instruction using the prepared message and the leased component.
    async fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> std::result::Result<Self::Effect, Self::Error>;
}

/// Prepares the owned message for an async instruction at field `F`.
pub(crate) trait BuildAsyncMessage<F, I>: Field<F> {
    /// The selected async instruction's message.
    type Message;

    /// Resolves the message before the async component is leased.
    fn build_async_message(&mut self, instruction: &I) -> Result<Self::Message>;
}

/// Pairs an owned instruction with the message for its async execution site.
pub(crate) struct BuildAsyncAt<F>(PhantomData<fn() -> F>);

/// Creates the message preparation node for an async instruction at field `F`.
pub(crate) fn build_async_message_at<F>() -> NodePlan<BuildAsyncAt<F>> {
    node()
}

impl<F> Label for BuildAsyncAt<F> {
    const LABEL: &'static str = "build async message";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

impl<M, Backend, F, I, D> Operation<M, I, Backend> for BuildAsyncAt<F>
where
    M: BuildAsyncMessage<F, I, Message = D::Message> + Field<F, Child = AsyncSlot<D>>,
    I: 'static,
    D: AsyncExecute<I, Message: 'static>,
{
    type Output = Prepared<I, D::Message>;

    fn start(
        machine: &mut M,
        driver: &Backend,
        instruction: I,
        done: impl FnOnce(&mut M, &Backend, Result<Self::Output>) + 'static,
    ) {
        let result = machine
            .build_async_message(&instruction)
            .map(|message| Prepared {
                instruction,
                message,
            });
        done(machine, driver, result);
    }
}

/// Holds the component on the machine whenever no invocation has leased it.
///
/// A pending route owns the component. The slot remains empty until its lease
/// returns the component, so a second invocation can detect that the field is busy.
pub(crate) struct AsyncSlot<D>(Rc<RefCell<Option<D>>>);

impl<D> AsyncSlot<D> {
    /// Creates a slot containing a component ready to be leased.
    pub(crate) fn new(component: D) -> Self {
        Self(Rc::new(RefCell::new(Some(component))))
    }

    /// Reads the component while it is available and returns `None` while it is leased.
    pub(crate) fn inspect<R>(&self, read: impl FnOnce(&D) -> R) -> Option<R> {
        self.0.borrow().as_ref().map(read)
    }

    /// Moves the component into a lease or reports that an invocation already holds it.
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

    /// Applies an effect to the restored component or reports that it is busy.
    pub(crate) fn apply_to_component<E>(&mut self, effect: E) -> Result<D::Effect>
    where
        D: Apply<E, Error: Into<eyre::Report>>,
    {
        self.0
            .borrow_mut()
            .as_mut()
            .ok_or_else(|| eyre!("component still leased"))?
            .apply(effect)
            .map_err(Into::into)
    }
}

/// Owns the component and a reference to the slot that receives it on drop.
struct Lease<D> {
    slot: Rc<RefCell<Option<D>>>,
    component: Option<D>,
}

impl<D> Lease<D> {
    /// Borrows the component exclusively for its async instruction.
    fn get_mut(&mut self) -> &mut D {
        self.component
            .as_mut()
            .expect("live lease owns the component")
    }
}

/// Restores the component on normal completion, failure, or drop of the pending future.
impl<D> Drop for Lease<D> {
    fn drop(&mut self) {
        *self.slot.borrow_mut() = self.component.take();
    }
}

/// Selects a field for `then_execute_async` in the step builder.
pub(crate) struct AsyncInstructionAt<F>(PhantomData<fn() -> F>);

/// Runs the async instruction at field `F` using the incoming instruction and message.
pub(crate) struct AsyncExecuteAt<F>(PhantomData<fn() -> F>);

/// Creates the field selector for the step's designated async execution node.
pub(crate) fn async_instruction_at<F>() -> NodePlan<AsyncInstructionAt<F>> {
    node()
}

/// Identifies the async execution site when projecting the route topology.
impl<F> Label for AsyncExecuteAt<F> {
    const LABEL: &'static str = "execute async instruction";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

/// Submits the leased component's future and restores it before delivering the result.
impl<M, F, I, D, Backend> Operation<M, Prepared<I, D::Message>, Backend> for AsyncExecuteAt<F>
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
        Prepared {
            instruction,
            message,
        }: Prepared<I, D::Message>,
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
