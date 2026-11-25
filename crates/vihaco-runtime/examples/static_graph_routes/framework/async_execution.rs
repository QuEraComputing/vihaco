// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! # Async preparation and leased execution
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
    graph::{
        BindOrigin, EmitsFrom, FieldOrigin, Label, NoInstruction, NoOrigin, NodePlan,
        OneInstruction, ResolveOperation, node,
    },
    operations::{Entry, Instruction, Operation, Prepared},
    runtime::FutureDriver,
};
use eyre::{Result, eyre};
use std::{any::type_name, cell::RefCell, future::Future, marker::PhantomData, rc::Rc};
use vihaco_runtime::{Apply, AsyncExecute, Field};

/// Prepares an owned message asynchronously for an instruction at field `F`.
pub(crate) trait AsyncBuildMessage<F, I, Context: ?Sized = ()>: Field<F> {
    /// The selected async instruction's message.
    type Message: 'static;
    /// The error produced during preparation.
    type Error;

    /// Returns an owned future that can finish before the component is leased.
    fn build_message(
        &mut self,
        instruction: &I,
        context: &Context,
    ) -> impl Future<Output = Result<Self::Message, Self::Error>> + 'static;
}

/// Pairs an owned instruction with the message for its async execution site.
pub(crate) struct BuildAsyncAt<F>(PhantomData<fn() -> F>);

/// Creates the async message preparation node for field `F`.
pub(crate) fn async_build_message_at<F>() -> NodePlan<BuildAsyncAt<F>> {
    node()
}

impl<F> Label for BuildAsyncAt<F> {
    const LABEL: &'static str = "async build message";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

impl<M, Backend, F, I: Clone + 'static, C: ?Sized> Operation<M, Entry<'_, I, C>, Backend>
    for BuildAsyncAt<F>
where
    M: AsyncBuildMessage<F, I, C, Error: Into<eyre::Report>>,
    Backend: FutureDriver<M>,
{
    type Output = Prepared<'static, I, M::Message>;

    fn start(
        machine: &mut M,
        driver: &Backend,
        Entry {
            instruction,
            context,
        }: Entry<'_, I, C>,
        done: impl FnOnce(&mut M, &Backend, Result<Self::Output>) + 'static,
    ) {
        let message = machine.build_message(instruction, context);
        // A later stage still needs the instruction after preparation suspends.
        let instruction = instruction.clone();
        let prepare = async move {
            Ok(Prepared {
                instruction: Instruction::Owned(instruction),
                message: message.await.map_err(Into::into)?,
            })
        };
        driver.run_future(machine, prepare, done);
    }
}

/// Reusable asynchronous message construction from a parent's capabilities.
pub(crate) trait AsyncReusableMessageRecipe<Parent, F, I, Context: ?Sized = ()>:
    Sized
{
    /// The error produced during preparation.
    type Error;

    /// Takes a short borrow of the parent and returns owned preparation work.
    fn prepare(
        parent: &mut Parent,
        instruction: &I,
        context: &Context,
    ) -> impl Future<Output = Result<Self, Self::Error>> + 'static;
}

/// Prepares the selected message asynchronously using its reusable recipe.
pub(crate) struct PrepareAsyncAt<F, Message>(PhantomData<fn() -> (F, Message)>);

/// Builds a message asynchronously from a recipe for the destination field.
pub(crate) fn async_prepare_message_at<F, Message>() -> NodePlan<PrepareAsyncAt<F, Message>> {
    node()
}

impl<F, Message> Label for PrepareAsyncAt<F, Message> {
    const LABEL: &'static str = "async prepare message";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

impl<M, Backend, F, I: Clone + 'static, C: ?Sized, Message> Operation<M, Entry<'_, I, C>, Backend>
    for PrepareAsyncAt<F, Message>
where
    M: Field<F>,
    Message: AsyncReusableMessageRecipe<M, F, I, C, Error: Into<eyre::Report>> + 'static,
    Backend: FutureDriver<M>,
{
    type Output = Prepared<'static, I, Message>;

    fn start(
        machine: &mut M,
        driver: &Backend,
        Entry {
            instruction,
            context,
        }: Entry<'_, I, C>,
        done: impl FnOnce(&mut M, &Backend, Result<Self::Output>) + 'static,
    ) {
        let message = Message::prepare(machine, instruction, context);
        // A later stage still needs the instruction after preparation suspends.
        let instruction = instruction.clone();
        let prepare = async move {
            Ok(Prepared {
                instruction: Instruction::Owned(instruction),
                message: message.await.map_err(Into::into)?,
            })
        };
        driver.run_future(machine, prepare, done);
    }
}

/// Handles an effect asynchronously at the receiving field.
pub(crate) trait AsyncApply<E> {
    /// The output forwarded to the next route node.
    type Effect: 'static;
    /// The error produced while handling the effect.
    type Error;

    /// Starts handling the effect and returns owned work that can outlive the field borrow.
    fn apply(
        &mut self,
        effect: E,
    ) -> impl Future<Output = Result<Self::Effect, Self::Error>> + 'static;
}

/// Handles an incoming effect at field `F` and forwards its eventual result.
pub(crate) struct AsyncApplyEffectAt<F>(PhantomData<fn() -> F>);

/// Creates an async effect handler for the selected receiving field.
pub(crate) fn async_apply_effect_at<F>()
-> NodePlan<AsyncApplyEffectAt<F>, NoInstruction, EmitsFrom<F>> {
    node()
}

impl<F> Label for AsyncApplyEffectAt<F> {
    const LABEL: &'static str = "async apply effect";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

impl<M, Backend, F, E> Operation<M, E, Backend> for AsyncApplyEffectAt<F>
where
    M: Field<F>,
    M::Child: AsyncApply<E, Error: Into<eyre::Report>>,
    Backend: FutureDriver<M>,
{
    type Output = <M::Child as AsyncApply<E>>::Effect;

    fn start(
        machine: &mut M,
        driver: &Backend,
        effect: E,
        done: impl FnOnce(&mut M, &Backend, Result<Self::Output>) + 'static,
    ) {
        let work = AsyncApply::apply(machine.get_mut(), effect);
        driver.run_future(machine, async move { work.await.map_err(Into::into) }, done);
    }
}

/// Composite policy that can suspend while handling an effect between two fields.
pub(crate) trait AsyncHandleEffect<Source, Destination, E, Context: ?Sized = ()>:
    Field<Source> + Field<Destination>
{
    /// The effect passed to the next node.
    type Effect: 'static;

    /// Takes a short composite borrow and returns owned work.
    fn handle_effect(
        &mut self,
        effect: E,
        context: &Context,
    ) -> impl Future<Output = Result<Self::Effect>> + 'static;
}

/// Runs asynchronous composite effect policy for a pair of fields.
pub(crate) struct AsyncHandleEffectBetween<Source, Destination>(
    PhantomData<fn() -> (Source, Destination)>,
);

/// Composite effect handler whose source will be resolved when the route is finished.
pub(crate) struct AsyncHandleEffectAt<Destination>(PhantomData<fn() -> Destination>);

/// Selects the composite's async handler for an effect arriving at the destination.
pub(crate) fn async_handle_effect_at<Destination>()
-> NodePlan<AsyncHandleEffectAt<Destination>, NoInstruction, BindOrigin> {
    node()
}

impl<Source, Destination> ResolveOperation<AsyncHandleEffectAt<Destination>, FieldOrigin<Source>>
    for BindOrigin
{
    type Operation = AsyncHandleEffectBetween<Source, Destination>;
    type Origin = NoOrigin;
}

impl<Source, Destination> Label for AsyncHandleEffectBetween<Source, Destination> {
    const LABEL: &'static str = "async handle composite effect";

    fn source_field() -> Option<&'static str> {
        Some(type_name::<Source>())
    }

    fn field() -> Option<&'static str> {
        Some(type_name::<Destination>())
    }
}

impl<M, Backend, Source, Destination, E> Operation<M, E, Backend>
    for AsyncHandleEffectBetween<Source, Destination>
where
    M: AsyncHandleEffect<Source, Destination, E>,
    Backend: FutureDriver<M>,
{
    type Output = M::Effect;

    fn start(
        machine: &mut M,
        driver: &Backend,
        effect: E,
        done: impl FnOnce(&mut M, &Backend, Result<Self::Output>) + 'static,
    ) {
        let work = machine.handle_effect(effect, &());
        driver.run_future(machine, work, done);
    }
}

/// Holds the component on the machine whenever no invocation has leased it.
///
/// A pending route owns the component. The slot remains empty until its lease
/// returns the component, so a second invocation can detect that the field is busy.
pub(crate) struct AsyncSlot<D>(Rc<RefCell<Option<D>>>);

#[cfg(feature = "program-image-execution")]
impl<D: vihaco_runtime::__private::HasInventory> vihaco_runtime::__private::HasInventory
    for AsyncSlot<D>
{
    type Provider = D::Provider;
}

#[cfg(feature = "program-image-execution")]
impl<D: vihaco_runtime::__private::Capabilities> vihaco_runtime::__private::Capabilities
    for AsyncSlot<D>
{
    const DESCRIPTION: &'static vihaco_runtime::__private::Description = D::DESCRIPTION;
}

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

/// Runs the async instruction at field `F` using the incoming instruction and message.
pub(crate) struct AsyncExecuteAt<F>(PhantomData<fn() -> F>);

/// Executes the prepared async instruction at the selected field.
pub(crate) fn execute_async_instruction_at<F>()
-> NodePlan<AsyncExecuteAt<F>, OneInstruction, EmitsFrom<F>> {
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
impl<M, F, I, D, Backend> Operation<M, Prepared<'_, I, D::Message>, Backend> for AsyncExecuteAt<F>
where
    M: Field<F, Child = AsyncSlot<D>> + 'static,
    I: Clone + 'static,
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
        }: Prepared<'_, I, D::Message>,
        done: impl FnOnce(&mut M, &Backend, Result<Self::Output>) + 'static,
    ) {
        match machine.get().lease() {
            Ok(mut lease) => {
                let instruction = instruction.into_owned();
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
