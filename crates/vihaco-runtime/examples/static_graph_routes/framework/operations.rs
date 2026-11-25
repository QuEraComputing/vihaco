// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! # Operations and their payloads
//!
//! A builder function selects an operation type. `Operation::start` interprets
//! an occurrence of that type for a machine, input, and driver. Its output
//! becomes the next node's input. A reported error ends the current path.
//!
//! | Constructor | Input | Output or continuation |
//! | --- | --- | --- |
//! | `build_message_at::<F, Message>()` | Owned instruction for `Run`, or borrowed instruction and context for `RunSync` | `Prepared` instruction and message |
//! | `execute_instruction_at::<F>()` | `Prepared` instruction and message | Effect from `Execute` on field `F` |
//! | `apply_effect_at::<F>()` | Effect | Output of `Apply` on field `F` |
//! | `duplicate()` | Cloneable payload | Two owned values for a fork |
//! | `await_work()` | Backend-specific work handle | Typed result delivered through `Await` |
//!
//! The callback interpreter calls `BuildMessage` with a unit context.
//! The borrowed interpreter lends it the program context. Field markers
//! select the component and identify the site in the topology.

use super::{
    graph::{
        BindOrigin, EmitsFrom, FieldOrigin, Fork, Label, NoInstruction, NoOrigin, Node, NodePlan,
        OneInstruction, ResolveOperation, Unannotated, node, split_with,
    },
    runtime::Await,
};
use eyre::Result;
use std::{any::type_name, marker::PhantomData};
use vihaco_runtime::{Apply, BuildMessage, Execute, Field, ReusableMessageRecipe};

/// Runs one operation for machine `M`, input `I`, and driver `D`.
///
/// A synchronous operation can report its result inline. A waiting operation
/// hands its continuation and the state it needs to the driver.
pub(crate) trait Operation<M, I, D> {
    /// The owned value passed to the continuation.
    type Output: 'static;

    /// Starts the operation and either completes inline or parks the callback with the driver.
    fn start(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    );
}

/// Carries an owned or borrowed instruction alongside its prepared message.
pub(crate) struct Prepared<I, Message> {
    /// The instruction that the execution node will borrow.
    pub(super) instruction: I,
    /// The prepared snapshot, including any runtime bindings needed by this invocation.
    pub(super) message: Message,
}

/// Runs the machine's `BuildMessage` implementation for field `F`.
pub(crate) struct BuildAt<F, Message>(PhantomData<fn() -> (F, Message)>);

/// Creates the message-preparation node for field `F`.
pub(crate) fn build_message_at<F, Message>() -> NodePlan<BuildAt<F, Message>> {
    node()
}

/// Names message preparation and identifies its target field for inspection.
impl<F, Message> Label for BuildAt<F, Message> {
    const LABEL: &'static str = "build message";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

/// Calls BuildMessage and forwards the prepared snapshot with the instruction.
impl<M, D, F, I: 'static, Message: 'static> Operation<M, I, D> for BuildAt<F, Message>
where
    M: BuildMessage<F, I, (), Message>,
{
    type Output = Prepared<I, Message>;

    fn start(
        machine: &mut M,
        driver: &D,
        instruction: I,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        let result = machine
            .build_message(&instruction, &())
            .map(|message| Prepared {
                instruction,
                message,
            });
        done(machine, driver, result);
    }
}

/// Prepares the selected message using its reusable recipe.
pub(crate) struct PrepareAt<F, Message>(PhantomData<fn() -> (F, Message)>);

/// Builds a message from a recipe for the destination field.
pub(crate) fn prepare_message_at<F, Message>() -> NodePlan<PrepareAt<F, Message>> {
    node()
}

impl<F, Message> Label for PrepareAt<F, Message> {
    const LABEL: &'static str = "prepare message";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

impl<M, D, F, I: 'static, Message> Operation<M, I, D> for PrepareAt<F, Message>
where
    M: Field<F>,
    Message: ReusableMessageRecipe<M, F, I, ()> + 'static,
{
    type Output = Prepared<I, Message>;

    fn start(
        machine: &mut M,
        driver: &D,
        instruction: I,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        let result = Message::prepare(machine, &instruction, &()).map(|message| Prepared {
            instruction,
            message,
        });
        done(machine, driver, result);
    }
}

/// Executes the prepared instruction at the selected field.
pub(crate) fn execute_instruction_at<F>() -> NodePlan<ExecuteAt<F>, OneInstruction, EmitsFrom<F>> {
    node()
}

/// Runs `Execute` at field `F` and forwards the resulting effect.
pub(crate) struct ExecuteAt<F>(PhantomData<fn() -> F>);

/// Names the instruction operation and identifies its selected field.
impl<F> Label for ExecuteAt<F> {
    const LABEL: &'static str = "execute instruction";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

/// Executes the prepared instruction once on the selected component and forwards its effect.
impl<M, D, F, I, Message> Operation<M, Prepared<I, Message>, D> for ExecuteAt<F>
where
    M: Field<F>,
    M::Child: Execute<I, Message = Message, Effect: 'static, Error: Into<eyre::Report>>,
{
    type Output = <M::Child as Execute<I>>::Effect;

    fn start(
        machine: &mut M,
        driver: &D,
        input: Prepared<I, Message>,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        let result = machine
            .get_mut()
            .execute(&input.instruction, input.message)
            .map_err(Into::into);
        done(machine, driver, result);
    }
}

/// Applies a routed effect to field `F`.
pub(crate) struct ApplyEffectAt<F>(PhantomData<fn() -> F>);

/// Creates a node that applies an effect to field `F` and forwards its output.
pub(crate) fn apply_effect_at<F>() -> NodePlan<ApplyEffectAt<F>, NoInstruction, EmitsFrom<F>> {
    node()
}

impl<F> Label for ApplyEffectAt<F> {
    const LABEL: &'static str = "apply effect";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

impl<M: Field<F>, D, F, E> Operation<M, E, D> for ApplyEffectAt<F>
where
    M::Child: Apply<E, Effect: 'static, Error: Into<eyre::Report>>,
{
    type Output = <M::Child as Apply<E>>::Effect;

    fn start(
        machine: &mut M,
        driver: &D,
        effect: E,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        let result = machine.get_mut().apply(effect).map_err(Into::into);
        done(machine, driver, result);
    }
}

/// Composite policy for an effect traveling from `Source` to `Destination`.
pub(crate) trait HandleEffect<Source, Destination, E, Context: ?Sized = ()>:
    Field<Source> + Field<Destination>
{
    /// The effect passed to the next node.
    type Effect;

    /// Handles the incoming effect according to the composite's policy.
    fn handle_effect(&mut self, effect: E, context: &Context) -> Result<Self::Effect>;
}

/// Runs synchronous composite effect policy for a pair of fields.
pub(crate) struct HandleEffectBetween<Source, Destination>(
    PhantomData<fn() -> (Source, Destination)>,
);

/// Composite effect handler whose source will be resolved when the route is finished.
pub(crate) struct HandleEffectAt<Destination>(PhantomData<fn() -> Destination>);

/// Selects the composite's handler for an effect arriving at the destination.
pub(crate) fn handle_effect_at<Destination>()
-> NodePlan<HandleEffectAt<Destination>, NoInstruction, BindOrigin> {
    node()
}

impl<Source, Destination> ResolveOperation<HandleEffectAt<Destination>, FieldOrigin<Source>>
    for BindOrigin
{
    type Operation = HandleEffectBetween<Source, Destination>;
    type Origin = NoOrigin;
}

impl<Source, Destination> Label for HandleEffectBetween<Source, Destination> {
    const LABEL: &'static str = "handle composite effect";

    fn source_field() -> Option<&'static str> {
        Some(type_name::<Source>())
    }

    fn field() -> Option<&'static str> {
        Some(type_name::<Destination>())
    }
}

impl<M, D, Source, Destination, E> Operation<M, E, D> for HandleEffectBetween<Source, Destination>
where
    M: HandleEffect<Source, Destination, E, Effect: 'static>,
{
    type Output = M::Effect;

    fn start(
        machine: &mut M,
        driver: &D,
        effect: E,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        let result = machine.handle_effect(effect, &());
        done(machine, driver, result);
    }
}

/// Supplies the same payload to two arms of a fork.
pub(crate) struct Duplicate;

/// Creates a split that clones its payload once and moves the original.
pub(crate) fn duplicate() -> Fork<Node<Duplicate, Unannotated, NoInstruction>, ()> {
    split_with(node())
}

impl Label for Duplicate {
    const LABEL: &'static str = "duplicate payload";
}

impl<M, D, T: Clone + 'static> Operation<M, T, D> for Duplicate {
    type Output = (T, T);

    fn start(
        machine: &mut M,
        driver: &D,
        input: T,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        done(machine, driver, Ok((input.clone(), input)));
    }
}

/// Awaits work supplied by the preceding node using the selected driver.
pub(crate) struct AwaitWork;

/// Creates a node that consumes a work handle and forwards the driver's result.
pub(crate) fn await_work() -> NodePlan<AwaitWork> {
    node()
}

/// Gives the pending-work node a diagnostic label in the topology.
impl Label for AwaitWork {
    const LABEL: &'static str = "await completion";
}

/// Passes the incoming handle and continuation to the driver's `Await` implementation.
impl<M, D, H> Operation<M, H, D> for AwaitWork
where
    D: Await<M, H>,
{
    type Output = D::Output;

    fn start(
        machine: &mut M,
        driver: &D,
        input: H,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        driver.wait(machine, input, done);
    }
}
