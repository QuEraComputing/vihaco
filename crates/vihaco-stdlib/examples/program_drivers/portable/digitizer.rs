// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! An async instruction keeps its component across a suspension.

use super::{
    owner::{RouteDriver, StepContext},
    tasks::Submit,
};
use crate::framework::{
    AsyncBuildMessage, AsyncSlot, Describe, FutureDriver, Route, Run, apply_effect_at,
    async_build_message_at, async_instruction_at, step_graph,
};
use eyre::{Result, WrapErr, eyre};
use std::{collections::VecDeque, convert::Infallible, future::Future};
use tokio::sync::oneshot;
use vihaco_abi::{Type, Value};
use vihaco_module::module::LocalModule;
use vihaco_runtime::{
    Apply, AsyncExecute, Execute, HasInstructionSet, ReusableMessageRecipe, Step, component,
    composite, dialect, field,
};
use vihaco_stdlib::ExecutionState;

#[dialect]
pub(crate) mod acquisition {
    #[instruction]
    pub struct Capture;
}

use acquisition::Capture;

#[component]
#[instructions { acquisition }]
#[derive(Default)]
pub(crate) struct Digitizer {
    issued: usize,
    sample: Option<u16>,
}

impl AsyncExecute<Capture> for Digitizer {
    type Message = oneshot::Receiver<u16>;
    type Effect = u16;
    type Error = eyre::Report;

    async fn execute(&mut self, _: &Capture, sample: Self::Message) -> Result<u16> {
        self.issued += 1;
        // The component remains leased while its own future waits for the sample.
        sample.await.wrap_err("digitizer sample channel closed")
    }
}

impl Apply<u16> for Digitizer {
    type Effect = ();
    type Error = Infallible;

    fn apply(&mut self, sample: u16) -> Result<(), Self::Error> {
        self.sample = Some(sample);
        Ok(())
    }
}

impl Apply<u16> for AsyncSlot<Digitizer> {
    type Effect = ();
    type Error = eyre::Report;

    fn apply(&mut self, sample: u16) -> Result<()> {
        self.apply_to_component(sample)
    }
}

#[derive(Debug)]
pub(crate) enum Pause {
    Route,
    Faulted,
}

// The lease slot has the same inventory as the component it holds. Make
// the companion modules available under the field type's alias.
pub use __vihaco_fields_Digitizer as __vihaco_fields_CaptureDevice;
pub use __vihaco_inventory_Digitizer as __vihaco_inventory_CaptureDevice;
type CaptureDevice = AsyncSlot<Digitizer>;

#[composite]
pub(crate) struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState<Pause>,
    #[device(1)]
    digitizer: CaptureDevice,
    samples: VecDeque<oneshot::Receiver<oneshot::Receiver<u16>>>,
}

type ProgramField = field!(Machine::execution);
type DigitizerField = field!(Machine::digitizer);
pub(crate) type Instruction = <Machine as HasInstructionSet>::Runtime;
pub(crate) type SyntaxInstruction = <Machine as HasInstructionSet>::Syntax;
pub(crate) type Module = LocalModule<Instruction, Value, Type>;

impl<I, Message, B> Step<ProgramField, I, StepContext<'_, B>> for Machine
where
    ExecutionState<Pause>: Execute<I, Message = Message, Effect = (), Error = eyre::Report>,
    Message: ReusableMessageRecipe<Self, ProgramField, I, Module>,
{
    fn step(&mut self, instruction: &I, context: &StepContext<'_, B>) -> Result<()> {
        let message = Message::prepare(self, instruction, context.module)?;
        self.execution.execute(instruction, message)
    }
}

impl<B: Submit> Step<DigitizerField, Capture, StepContext<'_, B>> for Machine {
    fn step(&mut self, _: &Capture, context: &StepContext<'_, B>) -> Result<()> {
        context.start(self, &route(), Capture)
    }
}

impl AsyncBuildMessage<DigitizerField, Capture> for Machine {
    type Message = oneshot::Receiver<u16>;
    type Error = eyre::Report;

    fn build_message(
        &mut self,
        _: &Capture,
        _: &(),
    ) -> impl Future<Output = Result<Self::Message>> + 'static {
        let sample = self
            .samples
            .pop_front()
            .ok_or_else(|| eyre!("capture has no sample channel"));
        async move { sample?.await.wrap_err("sample handle source closed") }
    }
}

fn route<D: FutureDriver<Machine>>()
-> Route<Capture, impl Run<Machine, Capture, D, Output = ()> + Describe> {
    step_graph::<Capture>()
        .begin(async_build_message_at::<DigitizerField>())
        .then_execute_async(async_instruction_at::<DigitizerField>())
        // Execution returns the lease before this node borrows the same field.
        .then(apply_effect_at::<DigitizerField>())
        .finish()
}

pub(crate) fn fixture(count: usize) -> (Machine, Vec<oneshot::Sender<u16>>) {
    let (ports, samples) = (0..count)
        .map(|_| {
            let (send_sample, sample) = oneshot::channel();
            let (publish_handle, handle) = oneshot::channel();
            assert!(publish_handle.send(sample).is_ok());
            (send_sample, handle)
        })
        .unzip();
    let machine = Machine {
        execution: ExecutionState::default(),
        digitizer: AsyncSlot::new(Digitizer::default()),
        samples,
    };
    (machine, ports)
}

impl Machine {
    pub(crate) fn digitizer_state(&self) -> Option<(usize, Option<u16>)> {
        self.digitizer
            .inspect(|digitizer| (digitizer.issued, digitizer.sample))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TopologySignature {
    pub(crate) nodes: Vec<(&'static str, Option<&'static str>)>,
    pub(crate) edges: Vec<(usize, usize)>,
}

/// Inspect the same declaration under either interpreter.
pub(crate) fn topology<B: Submit>() -> TopologySignature {
    let topology = route::<RouteDriver<B>>().topology();
    TopologySignature {
        nodes: topology
            .nodes
            .into_iter()
            .map(|node| (node.name, node.field))
            .collect(),
        edges: topology
            .edges
            .into_iter()
            .map(|edge| (edge.from, edge.to))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::{Context, Poll, Waker};

    #[test]
    fn message_preparation_can_wait_without_leasing_the_digitizer() -> Result<()> {
        let (mut machine, _) = fixture(0);
        let (publish, handle) = oneshot::channel();
        let (_send_sample, sample) = oneshot::channel();
        machine.samples.push_back(handle);

        let mut prepare = std::pin::pin!(machine.build_message(&Capture, &()));
        let mut context = Context::from_waker(Waker::noop());
        assert!(prepare.as_mut().poll(&mut context).is_pending());
        assert_eq!(machine.digitizer_state(), Some((0, None)));
        assert!(publish.send(sample).is_ok());
        assert!(matches!(
            prepare.as_mut().poll(&mut context),
            Poll::Ready(Ok(_))
        ));
        Ok(())
    }
}
