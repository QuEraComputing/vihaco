// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Operation descriptors and their execution contracts.

use super::{
    graph::{Label, NodePlan, node},
    runtime::{Await, SpawnWorker},
};
use eyre::Result;
use std::{any::type_name, marker::PhantomData};
use vihaco_runtime::{BuildMessage, Execute, Field};

/// Interprets a static operation type for a machine and a typed input.
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

/// Owns an instruction and its prepared message for one invocation.
pub(crate) struct Prepared<I, Message> {
    /// The instruction that the execution node will borrow.
    pub(super) instruction: I,
    /// The prepared snapshot, including any runtime bindings needed by this invocation.
    pub(crate) message: Message,
}

/// Identifies the field that needs an instruction message.
pub(crate) struct BuildAt<F>(PhantomData<fn() -> F>);

/// Describes preparing an instruction message for field F.
pub(crate) fn build_message_at<F>() -> NodePlan<BuildAt<F>> {
    node()
}

// Names message preparation and identifies its target field for inspection.
impl<F> Label for BuildAt<F> {
    const LABEL: &'static str = "build message";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

// Calls BuildMessage and forwards the prepared snapshot with the instruction.
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

/// Identifies the instruction site selected by the step builder.
pub(crate) struct InstructionAt<F>(PhantomData<fn() -> F>);

/// Selects the field for the step’s designated instruction execution.
pub(crate) fn instruction_at<F>() -> NodePlan<InstructionAt<F>> {
    node()
}

/// The executable operation inserted by the step builder.
pub(crate) struct ExecuteAt<F>(PhantomData<fn() -> F>);

// Names the instruction operation and identifies its selected field.
impl<F> Label for ExecuteAt<F> {
    const LABEL: &'static str = "execute instruction";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

// Executes the prepared instruction once on the selected component and forwards its effect.
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

/// Lets a component accept an effect routed to it by the graph.
pub(crate) trait Receive<E> {
    /// Applies the received effect to this component.
    fn receive(&mut self, effect: E) -> Result<()>;
}

/// Identifies the field that receives a produced effect.
pub(crate) struct ReceiveAt<F>(PhantomData<fn() -> F>);

/// Describes sending an effect to the component at field F.
pub(crate) fn receive_effect_at<F>() -> NodePlan<ReceiveAt<F>> {
    node()
}

// Names effect reception and identifies its selected field.
impl<F> Label for ReceiveAt<F> {
    const LABEL: &'static str = "receive effect";

    fn field() -> Option<&'static str> {
        Some(type_name::<F>())
    }
}

// Borrows the receiving component, delivers the effect, and reports completion.
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

/// Defines an application computation with a machine, an input, and an output.
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

/// Adapts a Compute implementation to the operation contract.
pub(crate) struct ComputeNode<C>(PhantomData<fn() -> C>);

/// Describes a node that runs an application computation.
pub(crate) fn compute<C: Compute>() -> NodePlan<ComputeNode<C>> {
    node()
}

// Uses the computation’s declared name in the topology.
impl<C: Compute> Label for ComputeNode<C> {
    const LABEL: &'static str = C::NAME;
}

// Applies the computation on the driver thread and forwards its result.
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

/// Selects a one-shot signal from the invocation’s input.
pub(crate) trait SignalInput {
    /// The invocation input that owns the signal handle.
    type Input: 'static;
    /// The handle the selected driver can await.
    type Signal;

    /// The name displayed for this wait during inspection.
    const NAME: &'static str;

    /// Extracts the wait handle from the current invocation’s input.
    fn take_signal(input: &mut Self::Input) -> Result<Self::Signal>;
}

/// Identifies the signal selection used by a wait node.
pub(crate) struct WaitForSignal<S>(PhantomData<fn() -> S>);

/// Describes a wait that forwards its input when the signal arrives.
pub(crate) fn wait_for_signal<S: SignalInput>() -> NodePlan<WaitForSignal<S>> {
    node()
}

// Uses the signal selection’s declared name for the wait node.
impl<S: SignalInput> Label for WaitForSignal<S> {
    const LABEL: &'static str = S::NAME;
}

// Parks with the input and resumes it when the selected signal completes.
impl<M, D, S: SignalInput> Operation<M, S::Input, D> for WaitForSignal<S>
where
    D: Await<M, S::Signal, Output = ()>,
{
    type Output = S::Input;

    fn start(
        machine: &mut M,
        driver: &D,
        mut input: S::Input,
        done: impl FnOnce(&mut M, &D, Result<S::Input>) + 'static,
    ) {
        match S::take_signal(&mut input) {
            Ok(signal) => driver.wait(machine, signal, move |machine, driver, result| {
                done(machine, driver, result.map(|()| input));
            }),
            Err(error) => done(machine, driver, Err(error)),
        }
    }
}

/// Awaits work supplied by the preceding node using the selected driver.
pub(crate) struct AwaitWork;

pub(crate) fn await_work() -> NodePlan<AwaitWork> {
    node()
}

impl Label for AwaitWork {
    const LABEL: &'static str = "await completion";
}

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

/// Defines work that runs on a separate thread with an owned input and output.
pub(crate) trait Worker {
    /// The owned payload handed to the worker thread.
    type Input: Send + 'static;

    /// The owned result sent back to the driver.
    type Output: Send + 'static;

    /// The name displayed for the worker’s spawn node.
    const NAME: &'static str;

    /// Runs on a worker thread with the owned input.
    fn run(input: Self::Input) -> Result<Self::Output>;
}

/// Carries the runtime completion handle for a particular worker type.
pub(crate) struct Task<W, H>(H, PhantomData<fn() -> W>);

/// Identifies the worker implementation to launch.
pub(crate) struct Spawn<W>(PhantomData<fn() -> W>);

/// Describes launching an owned payload on a worker and producing a typed handle.
pub(crate) fn spawn_task<W: Worker>() -> NodePlan<Spawn<W>> {
    node()
}

// Names the spawn node using the worker’s declared name.
impl<W: Worker> Label for Spawn<W> {
    const LABEL: &'static str = W::NAME;
}

// Submits work to the driver and forwards the worker handle.
impl<M, D: SpawnWorker, W: Worker + 'static> Operation<M, W::Input, D> for Spawn<W> {
    type Output = Task<W, D::Work<W::Output>>;

    fn start(
        machine: &mut M,
        driver: &D,
        input: W::Input,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        done(
            machine,
            driver,
            driver
                .spawn(W::run, input)
                .map(|work| Task(work, PhantomData)),
        );
    }
}

/// Identifies the worker whose result this node accepts.
pub(crate) struct AwaitTask<W>(PhantomData<fn() -> W>);

/// Describes waiting for the result of a worker of type W.
pub(crate) fn await_task<W: Worker>() -> NodePlan<AwaitTask<W>> {
    node()
}

impl<W> Label for AwaitTask<W> {
    const LABEL: &'static str = "await worker result";
}

impl<M, D, W: Worker, H> Operation<M, Task<W, H>, D> for AwaitTask<W>
where
    D: Await<M, H, Output = W::Output>,
{
    type Output = W::Output;

    fn start(
        machine: &mut M,
        driver: &D,
        input: Task<W, H>,
        done: impl FnOnce(&mut M, &D, Result<Self::Output>) + 'static,
    ) {
        driver.wait(machine, input.0, done);
    }
}
