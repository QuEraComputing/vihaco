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
//! | `build_message_at::<F>()` | Owned instruction for `Run`, or borrowed instruction and context for `RunSync` | `Prepared` instruction and message |
//! | `instruction_at::<F>()` | `Prepared` instruction and message | Effect from `Execute` on field `F` |
//! | `receive_effect_at::<F>()` | Effect | `()` after fallible delivery to field `F` |
//! | `compute::<C>()` | Application-defined value | Result of `Compute::apply` |
//! | `await_work()` | Backend-specific work handle | Typed result delivered through `Await` |
//!
//! The callback interpreter calls `BuildMessage` with a unit context.
//! The borrowed interpreter lends it the program context. Field markers
//! select the component and identify the site in the topology. The async
//! example uses a `Compute` preparation node because `BuildMessage` requires
//! a field that implements synchronous `Execute`.

use super::{
    graph::{Label, NodePlan, node},
    runtime::Await,
};
use eyre::Result;
use std::{any::type_name, marker::PhantomData};
use vihaco_runtime::{BuildMessage, Execute, Field};

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
pub(crate) struct BuildAt<F>(PhantomData<fn() -> F>);

/// Creates the message-preparation node for field `F`.
pub(crate) fn build_message_at<F>() -> NodePlan<BuildAt<F>> {
    node()
}

/// Names message preparation and identifies its target field for inspection.
impl<F> Label for BuildAt<F> {
    const LABEL: &'static str = "build message";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

/// Calls BuildMessage and forwards the prepared snapshot with the instruction.
impl<M, D, F, I: 'static, Message: 'static> Operation<M, I, D> for BuildAt<F>
where
    M: BuildMessage<F, I, (), Message>,
    M::Child: Execute<I, Message = Message>,
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

/// Marks the field selected for synchronous execution by the step builder.
///
/// `then_execute` turns this selector into an `ExecuteAt` operation.
pub(crate) struct InstructionAt<F>(PhantomData<fn() -> F>);

/// Selects the field for the step’s designated instruction execution.
pub(crate) fn instruction_at<F>() -> NodePlan<InstructionAt<F>> {
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

/// Applies a routed effect to its receiving component.
///
/// A receive error ends the current path and is passed to its continuation.
pub(crate) trait Receive<E> {
    /// Applies the received effect to this component.
    fn receive(&mut self, effect: E) -> Result<()>;
}

/// Delivers an effect to field `F` using its component's `Receive` implementation.
pub(crate) struct ReceiveAt<F>(PhantomData<fn() -> F>);

/// Creates a node that delivers an effect to field `F` and completes with `()`.
pub(crate) fn receive_effect_at<F>() -> NodePlan<ReceiveAt<F>> {
    node()
}

/// Names effect reception and identifies its selected field.
impl<F> Label for ReceiveAt<F> {
    const LABEL: &'static str = "receive effect";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

/// Borrows the receiving component, delivers the effect, and reports completion.
impl<M: Field<F>, D, F, I> Operation<M, I, D> for ReceiveAt<F>
where
    M::Child: Receive<I>,
{
    type Output = ();

    fn start(
        machine: &mut M,
        driver: &D,
        input: I,
        done: impl FnOnce(&mut M, &D, Result<()>) + 'static,
    ) {
        let result = machine.get_mut().receive(input);
        done(machine, driver, result);
    }
}

/// Defines an application computation that runs inline with a machine borrow.
///
/// A computation can split one payload for a fork or transform an effect
/// before delivery.
pub(crate) trait Compute {
    /// The machine available to the computation while it runs.
    type Machine;
    /// The payload consumed by the computation.
    type Input;

    /// The owned value passed to the continuation.
    type Output: 'static;

    /// The name displayed for this computation during inspection.
    const NAME: &'static str;

    /// Runs the computation and returns the downstream payload.
    fn apply(machine: &mut Self::Machine, input: Self::Input) -> Result<Self::Output>;
}

/// Adapts a `Compute` implementation into an operation node.
pub(crate) struct ComputeNode<C>(PhantomData<fn() -> C>);

/// Describes a node that runs an application computation.
pub(crate) fn compute<C: Compute>() -> NodePlan<ComputeNode<C>> {
    node()
}

/// Uses the computation’s declared name in the topology.
impl<C: Compute> Label for ComputeNode<C> {
    const LABEL: &'static str = C::NAME;
}

/// Applies the computation on the driver thread and forwards its result.
impl<D, C: Compute> Operation<C::Machine, C::Input, D> for ComputeNode<C> {
    type Output = C::Output;

    fn start(
        machine: &mut C::Machine,
        driver: &D,
        input: C::Input,
        done: impl FnOnce(&mut C::Machine, &D, Result<C::Output>) + 'static,
    ) {
        let result = C::apply(machine, input);
        done(machine, driver, result);
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
