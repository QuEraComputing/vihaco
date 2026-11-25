// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! One program and route reused by all five runners.

use crate::{
    checked::{CheckedDriver, Module, Pause, StepContext},
    framework::{
        AnnotatedDriver, Await, HandleEffect, Route, Run, Signal, await_work,
        execute_instruction_at, handle_effect_at, prepare_message_at, step_graph,
    },
    timing::MinExecution,
};
use eyre::Result;
use vihaco_abi::{Type, word::Word};
use vihaco_module::module::{FunctionInfo, Signature};
use vihaco_runtime::{
    Execute, HasInstructionSet, NoMessage, PushToStack, ReusableMessageRecipe, Step, component,
    composite, dialect, field,
};
use vihaco_stdlib::{ExecutionState, Image, Loadable, program::program};

#[dialect]
pub mod ops {
    #[instruction]
    pub struct Sample(pub u64);
}

/// Work request produced by the sensor. Backends decide how to deliver a value.
pub struct SampleRequest(pub Word);

/// Completion pushes exactly one sample word onto the program operand stack.
pub(super) type SampleEffect = PushToStack<[Word; 1]>;

#[component]
#[instructions { ops }]
#[derive(Default)]
pub struct Sensor {
    pub(super) issued: usize,
}

impl Execute<ops::Sample> for Sensor {
    type Message = NoMessage;
    type Effect = SampleRequest;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &ops::Sample, _: NoMessage) -> Result<SampleRequest> {
        self.issued += 1;
        Ok(SampleRequest(instruction.0))
    }
}

#[composite]
#[derive(Default)]
pub struct Machine {
    #[device(0)]
    #[program]
    pub(super) execution: vihaco_stdlib::ExecutionState<Pause>,
    #[device(1)]
    pub(super) sensor: Sensor,
    // The channel scenario uses this sender to stand in for its external device.
    pub(super) sender: Option<Signal<Word>>,
    pub(super) samples_delivered: usize,
}

type ProgramField = field!(Machine::execution);
type SensorField = field!(Machine::sensor);
pub(super) type Instruction = <Machine as HasInstructionSet>::Runtime;
pub(super) type Driver<B> = CheckedDriver<Machine, Instruction, B>;

impl<I, B, Message> Step<ProgramField, I, StepContext<'_, Instruction, B>> for Machine
where
    ExecutionState<Pause>: Execute<I, Message = Message, Effect = (), Error = eyre::Report>,
    Message: ReusableMessageRecipe<Self, ProgramField, I, Module<Instruction>>,
{
    fn step(&mut self, instruction: &I, context: &StepContext<'_, Instruction, B>) -> Result<()> {
        let message = Message::prepare(self, instruction, context.module)?;
        self.execution.execute(instruction, message)
    }
}

impl HandleEffect<SensorField, ProgramField, SampleEffect> for Machine {
    type Effect = ();

    fn handle_effect(&mut self, effect: SampleEffect, _: &()) -> Result<()> {
        vihaco_runtime::Apply::apply(&mut self.execution, effect)?;
        // The composite counts accepted replies separately from device issues.
        self.samples_delivered += 1;
        Ok(())
    }
}

fn sample_route<
    B: AnnotatedDriver<Machine> + Await<Machine, SampleRequest, Output = SampleEffect>,
>() -> Route<ops::Sample, impl Run<Machine, ops::Sample, B, Output = ()>> {
    step_graph::<ops::Sample>()
        .begin(prepare_message_at::<SensorField, NoMessage>())
        // Simulation releases the request after three ticks, then waits for its reply.
        .then(execute_instruction_at::<SensorField>().annotate::<MinExecution<3>>())
        .then(await_work())
        .then(handle_effect_at::<ProgramField>())
        .finish()
}

impl<B> Step<SensorField, ops::Sample, StepContext<'_, Instruction, B>> for Machine
where
    B: AnnotatedDriver<Machine> + Await<Machine, SampleRequest, Output = SampleEffect>,
{
    fn step(
        &mut self,
        instruction: &ops::Sample,
        context: &StepContext<'_, Instruction, B>,
    ) -> Result<()> {
        context.start(self, &sample_route(), ops::Sample(instruction.0))
    }
}

pub(super) fn driver<B>(backend: B, sample: Word) -> Driver<B> {
    with_machine(Machine::default(), backend, sample)
}

pub(super) fn with_machine<B>(machine: Machine, backend: B, sample: Word) -> Driver<B> {
    use __vihaco_MachineInstruction::*;
    let module = Module {
        code: vec![
            V_execution__program__Branch(program::Branch(2)),
            V_execution__program__Halt(program::Halt),
            V_sensor__ops__Sample(ops::Sample(sample)),
            V_execution__program__Return(program::Return(1)),
        ],
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![],
                ret: vec![Type::U64],
            },
            local_count: 0,
            start_address: 0,
            end_address: 4,
            file: 0,
        }],
        strings: vec!["main".into()],
        main_function: Some(0),
        ..Default::default()
    };
    CheckedDriver::new(Loadable::new(Image::new(module), machine), backend)
}
