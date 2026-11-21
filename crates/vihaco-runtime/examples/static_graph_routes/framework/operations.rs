// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Operation descriptors and their execution contracts.

use super::{
    graph::{Label, NodePlan, node},
    runtime::{Done, Driver, Wait},
};
use eyre::Result;
use std::{any::type_name, marker::PhantomData};
use vihaco_runtime::{BuildMessage, Execute, Field};

/// Interprets a static operation type for a machine and a typed input.
pub(crate) trait Operation<M, I> {
    /// The owned value passed to the continuation.
    type Output: 'static;

    /// Starts the operation and either completes inline or parks the callback with the driver.
    fn start(machine: &mut M, driver: &Driver<M>, input: I, done: Done<M, Self::Output>);
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
impl<M, F, I: 'static, Message: 'static> Operation<M, I> for BuildAt<F>
where
    M: BuildMessage<F, I, (), Message>,
    M::Child: Execute<I, Message = Message>,
{
    type Output = Prepared<I, Message>;

    fn start(machine: &mut M, driver: &Driver<M>, instruction: I, done: Done<M, Self::Output>) {
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
impl<M, F, I, Message> Operation<M, Prepared<I, Message>> for ExecuteAt<F>
where
    M: Field<F>,
    M::Child: Execute<I, Message = Message, Effect: 'static, Error: Into<eyre::Report>>,
{
    type Output = <M::Child as Execute<I>>::Effect;

    fn start(
        machine: &mut M,
        driver: &Driver<M>,
        input: Prepared<I, Message>,
        done: Done<M, Self::Output>,
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
impl<M: Field<F>, F, I> Operation<M, I> for ReceiveAt<F>
where
    M::Child: Receive<I>,
{
    type Output = ();

    fn start(machine: &mut M, driver: &Driver<M>, input: I, done: Done<M, ()>) {
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
impl<C: Compute> Operation<C::Machine, C::Input> for ComputeNode<C> {
    type Output = C::Output;

    fn start(
        machine: &mut C::Machine,
        driver: &Driver<C::Machine>,
        input: C::Input,
        done: Done<C::Machine, C::Output>,
    ) {
        let result = C::apply(machine, input);
        done(machine, driver, result);
    }
}

/// Selects a one-shot signal from the invocation’s input.
pub(crate) trait SignalInput {
    /// The invocation input that owns the signal handle.
    type Input: 'static;

    /// The name displayed for this wait during inspection.
    const NAME: &'static str;

    /// Extracts the wait handle from the current invocation’s input.
    fn take_signal(input: &mut Self::Input) -> Result<Wait<()>>;
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
impl<M: 'static, S: SignalInput> Operation<M, S::Input> for WaitForSignal<S> {
    type Output = S::Input;

    fn start(machine: &mut M, driver: &Driver<M>, mut input: S::Input, done: Done<M, S::Input>) {
        match S::take_signal(&mut input) {
            Ok(signal) => driver.park(
                signal,
                Box::new(move |machine, driver, result| {
                    done(machine, driver, result.map(|()| input));
                }),
            ),
            Err(error) => done(machine, driver, Err(error)),
        }
    }
}

/// Marks the point where a device result becomes available to the route.
pub(crate) struct AwaitDevice;

/// Describes waiting on a device handle supplied by the preceding node.
pub(crate) fn await_device_completion() -> NodePlan<AwaitDevice> {
    node()
}

// Names the device-completion boundary for inspection.
impl Label for AwaitDevice {
    const LABEL: &'static str = "await device completion";
}

// Parks on the incoming device handle and forwards its completion value.
impl<M: 'static, T: 'static> Operation<M, Wait<T>> for AwaitDevice {
    type Output = T;

    fn start(_: &mut M, driver: &Driver<M>, input: Wait<T>, done: Done<M, T>) {
        driver.park(input, done);
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
pub(crate) struct Task<W: Worker>(Wait<W::Output>);

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
impl<M: 'static, W: Worker + 'static> Operation<M, W::Input> for Spawn<W> {
    type Output = Task<W>;

    fn start(machine: &mut M, driver: &Driver<M>, input: W::Input, done: Done<M, Self::Output>) {
        done(machine, driver, driver.spawn(W::run, input).map(Task));
    }
}

/// Identifies the worker whose result this node accepts.
pub(crate) struct AwaitTask<W>(PhantomData<fn() -> W>);

/// Describes waiting for the result of a worker of type W.
pub(crate) fn await_task<W: Worker>() -> NodePlan<AwaitTask<W>> {
    node()
}

// Names the worker-result boundary for inspection.
impl<W> Label for AwaitTask<W> {
    const LABEL: &'static str = "await worker result";
}

// Parks on the worker handle until its result is ready.
impl<M: 'static, W: Worker> Operation<M, Task<W>> for AwaitTask<W> {
    type Output = W::Output;

    fn start(_: &mut M, driver: &Driver<M>, input: Task<W>, done: Done<M, Self::Output>) {
        driver.park(input.0, done);
    }
}
