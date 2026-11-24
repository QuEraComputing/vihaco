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
//! | `receive_effect_at::<F>()` | Effect | Output from the field's fallible `Receive` handler |
//! | `apply_effect_at::<F>()` | Effect | `()` after applying it to field `F` |
//! | `duplicate()` | Cloneable payload | Two owned values for a fork |
//! | `await_work()` | Backend-specific work handle | Typed result delivered through `Await` |
//!
//! The callback interpreter calls `BuildMessage` with a unit context.
//! The borrowed interpreter lends it the program context. Field markers
//! select the component and identify the site in the topology.

use super::{
    graph::{Label, NodePlan, node},
    runtime::Await,
};
use eyre::Result;
use std::{any::type_name, marker::PhantomData};
#[cfg(feature = "program-image-execution")]
use vihaco_runtime::Apply;
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
    /// The next effect to forward, or `()` for a terminal receiver.
    type Output;

    /// Applies the effect to this component and returns its output.
    fn receive(&mut self, effect: E) -> Result<Self::Output>;
}

/// Delivers an effect to field `F` using its component's `Receive` implementation.
pub(crate) struct ReceiveAt<F>(PhantomData<fn() -> F>);

/// Creates a node that delivers an effect to field `F` and forwards its output.
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

/// Borrows the receiving component, delivers the effect, and reports its output.
impl<M: Field<F>, D, F, E> Operation<M, E, D> for ReceiveAt<F>
where
    M::Child: Receive<E, Output: 'static>,
{
    type Output = <M::Child as Receive<E>>::Output;

    fn start(
        machine: &mut M,
        driver: &D,
        effect: E,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        let result = machine.get_mut().receive(effect);
        done(machine, driver, result);
    }
}

/// Delivers an infallible effect through the selected field's `Apply` implementation.
#[cfg(feature = "program-image-execution")]
pub(crate) struct ApplyAt<F>(PhantomData<fn() -> F>);

/// Creates a node that applies an effect to field `F`.
#[cfg(feature = "program-image-execution")]
#[allow(dead_code, reason = "used by the stdlib program examples")]
pub(crate) fn apply_effect_at<F>() -> NodePlan<ApplyAt<F>> {
    node()
}

#[cfg(feature = "program-image-execution")]
impl<F> Label for ApplyAt<F> {
    const LABEL: &'static str = "apply effect";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

#[cfg(feature = "program-image-execution")]
impl<M: Field<F>, D, F, E> Operation<M, E, D> for ApplyAt<F>
where
    M::Child: Apply<E>,
{
    type Output = ();

    fn start(
        machine: &mut M,
        driver: &D,
        effect: E,
        done: impl FnOnce(&mut M, &D, Result<()>) + 'static,
    ) {
        machine.get_mut().apply(effect);
        done(machine, driver, Ok(()));
    }
}

/// Supplies the same payload to two arms of a fork.
pub(crate) struct Duplicate;

/// Creates a split that clones its payload once and moves the original.
pub(crate) fn duplicate() -> NodePlan<Duplicate> {
    node()
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
