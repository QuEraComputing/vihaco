// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Warehouse's logical machine and routes. Timing and task placement live in the backend.

use crate::{
    framework::{
        AsyncApply, AsyncBuildMessage, AsyncHandleEffect, AsyncReusableMessageRecipe, AsyncSlot,
        Describe, Entry, FutureDriver, Route, RouteDeclaration, RouteDelivery, Run,
        apply_effect_at, async_apply_effect_at, async_build_message_at, async_handle_effect_at,
        async_prepare_message_at, edge, execute_async_instruction_at, execute_instruction_at,
        prepare_message_at, step_graph,
    },
    scheduled::{
        backends::Input,
        driver::{Module, Pause, StepContext},
        tasks::TimedSubmit,
    },
};
use eyre::{Result, ensure, eyre};
use std::{collections::VecDeque, future::Future};
use tokio::{sync::oneshot, task::yield_now};
use vihaco_runtime::{
    Apply, AsyncExecute, Execute, Field, HasInstructionSet, NoMessage, ReusableMessageRecipe, Step,
    component, composite, dialect, field,
};
use vihaco_stdlib::ExecutionState;

#[dialect]
pub(crate) mod labels {
    // Deliberately lacks Clone. Timed delivery retains the label, not this instruction.
    #[instruction]
    pub struct PrintLabel(pub u32);
}

#[dialect]
pub(crate) mod scans {
    #[instruction]
    #[derive(Clone)]
    pub struct Scan;

    #[instruction]
    #[derive(Clone)]
    pub struct Inspect;
}

#[component]
#[instructions { labels }]
#[derive(Default)]
pub(crate) struct Printer {
    issued: usize,
}

pub(crate) struct Label(u32);

impl Execute<labels::PrintLabel> for Printer {
    type Message = NoMessage;
    type Effect = Label;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &labels::PrintLabel, _: NoMessage) -> Result<Label> {
        self.issued += 1;
        Ok(Label(instruction.0))
    }
}

#[component]
#[instructions { scans }]
pub(crate) struct Scanner;

pub(crate) struct ScanMessage(oneshot::Receiver<u16>);
pub(crate) struct Reading(u16);

trait ScanInstruction {}
impl ScanInstruction for scans::Scan {}
impl ScanInstruction for scans::Inspect {}

impl<I: ScanInstruction> AsyncExecute<I> for Scanner {
    type Message = ScanMessage;
    type Effect = Reading;
    type Error = eyre::Report;

    async fn execute(&mut self, _: &I, message: ScanMessage) -> Result<Reading> {
        Ok(Reading(message.0.await?))
    }
}

pub use __vihaco_fields_Scanner as __vihaco_fields_LeasedScanner;
pub use __vihaco_inventory_Scanner as __vihaco_inventory_LeasedScanner;
type LeasedScanner = AsyncSlot<Scanner>;

#[derive(Default)]
struct Records {
    labels: Vec<u32>,
    readings: Vec<u16>,
}

impl Apply<Label> for Records {
    type Effect = ();
    type Error = eyre::Report;

    fn apply(&mut self, label: Label) -> Result<()> {
        self.labels.push(label.0);
        Ok(())
    }
}

impl AsyncApply<Reading> for Records {
    type Effect = ();
    type Error = eyre::Report;

    fn apply(&mut self, reading: Reading) -> impl Future<Output = Result<()>> + 'static {
        self.readings.push(reading.0);
        // This recording stage has an asynchronous acknowledgement.
        async {
            yield_now().await;
            Ok(())
        }
    }
}

#[derive(Default)]
struct ScanInputs(VecDeque<oneshot::Receiver<u16>>);

impl ScanInputs {
    fn prepare(&mut self) -> impl Future<Output = Result<ScanMessage>> + 'static {
        let input = self.0.pop_front();
        async move {
            yield_now().await;
            input
                .map(ScanMessage)
                .ok_or_else(|| eyre!("no scanner input wired"))
        }
    }
}

#[composite]
pub(crate) struct Warehouse {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState<Pause>,
    #[device(1)]
    printer: Printer,
    #[device(2)]
    scanner: LeasedScanner,
    records: Records,
    inputs: ScanInputs,
    minimum_reading: u16,
}

type PrinterField = field!(Warehouse::printer);
type ProgramField = field!(Warehouse::execution);
type ScannerField = field!(Warehouse::scanner);
pub(crate) type Instruction = <Warehouse as HasInstructionSet>::Runtime;

// Stand in for automatic Field generation on ordinary fields from the RFC.
macro_rules! project_field {
    ($marker:ident, $field:ident, $child:ty) => {
        struct $marker;
        impl Field<$marker> for Warehouse {
            type Child = $child;
            const SOURCE: &'static [&'static str] = &[stringify!($field)];
            fn get(&self) -> &Self::Child {
                &self.$field
            }
            fn get_mut(&mut self) -> &mut Self::Child {
                &mut self.$field
            }
        }
    };
}
project_field!(RecordsField, records, Records);
project_field!(InputsField, inputs, ScanInputs);

// The component's reusable message recipe works on any parent with scan inputs.
impl<M: Field<InputsField, Child = ScanInputs>, C: ?Sized>
    AsyncReusableMessageRecipe<M, ScannerField, scans::Scan, C> for ScanMessage
{
    type Error = eyre::Report;

    fn prepare(
        parent: &mut M,
        _: &scans::Scan,
        _: &C,
    ) -> impl Future<Output = Result<Self>> + 'static {
        parent.get_mut().prepare()
    }
}

// This instruction uses composite-specific preparation and delivery instead.
impl AsyncBuildMessage<ScannerField, scans::Inspect, Module<Self>> for Warehouse {
    type Message = ScanMessage;
    type Error = eyre::Report;

    fn build_message(
        &mut self,
        _: &scans::Inspect,
        _: &Module<Self>,
    ) -> impl Future<Output = Result<Self::Message>> + 'static {
        let minimum = self.minimum_reading;
        let input = self.inputs.prepare();
        async move {
            ensure!(minimum > 0, "invalid scan policy");
            input.await
        }
    }
}

impl AsyncHandleEffect<ScannerField, RecordsField, Reading> for Warehouse {
    type Effect = ();

    fn handle_effect(
        &mut self,
        reading: Reading,
        _: &(),
    ) -> impl Future<Output = Result<()>> + 'static {
        let minimum = self.minimum_reading;
        let accepted =
            (reading.0 >= minimum).then(|| AsyncApply::apply(&mut self.records, reading));
        async move {
            match accepted {
                Some(ack) => ack.await,
                None => Err(eyre!("scan reading below the warehouse minimum")),
            }
        }
    }
}

type RouteInput<'a, I> = Entry<'a, I, Module<Warehouse>>;

pub(crate) fn print_route<D: RouteDelivery<Warehouse>>() -> impl RouteDeclaration<
    labels::PrintLabel,
    Path: for<'a> Run<Warehouse, RouteInput<'a, labels::PrintLabel>, D, Output = ()>
              + Describe
              + 'static,
> {
    step_graph::<labels::PrintLabel>()
        .begin(prepare_message_at::<PrinterField, NoMessage>())
        .then(execute_instruction_at::<PrinterField>().named("print"))
        .then_via(
            edge().named("label_to_records"),
            apply_effect_at::<RecordsField>(),
        )
}

fn scan_route<D: FutureDriver<Warehouse> + RouteDelivery<Warehouse>>() -> Route<
    scans::Scan,
    impl for<'a> Run<Warehouse, RouteInput<'a, scans::Scan>, D, Output = ()> + Describe + 'static,
> {
    step_graph::<scans::Scan>()
        .begin(async_prepare_message_at::<ScannerField, ScanMessage>())
        .then(execute_async_instruction_at::<ScannerField>())
        .then(async_apply_effect_at::<RecordsField>())
        .finish()
}

fn policy_scan_route<D: FutureDriver<Warehouse> + RouteDelivery<Warehouse>>() -> Route<
    scans::Inspect,
    impl for<'a> Run<Warehouse, RouteInput<'a, scans::Inspect>, D, Output = ()> + Describe + 'static,
> {
    step_graph::<scans::Inspect>()
        .begin(async_build_message_at::<ScannerField>())
        .then(execute_async_instruction_at::<ScannerField>())
        .then(async_handle_effect_at::<RecordsField>())
        .finish()
}

impl<B: TimedSubmit> Step<PrinterField, labels::PrintLabel, StepContext<'_, Self, B>>
    for Warehouse
{
    fn step(
        &mut self,
        instruction: &labels::PrintLabel,
        context: &StepContext<'_, Self, B>,
    ) -> Result<()> {
        context.start(self, &print_route().finish(), instruction)
    }
}

impl<B: TimedSubmit> Step<ScannerField, scans::Scan, StepContext<'_, Self, B>> for Warehouse {
    fn step(
        &mut self,
        instruction: &scans::Scan,
        context: &StepContext<'_, Self, B>,
    ) -> Result<()> {
        context.start(self, &scan_route(), instruction)
    }
}

impl<B: TimedSubmit> Step<ScannerField, scans::Inspect, StepContext<'_, Self, B>> for Warehouse {
    fn step(
        &mut self,
        instruction: &scans::Inspect,
        context: &StepContext<'_, Self, B>,
    ) -> Result<()> {
        context.start(self, &policy_scan_route(), instruction)
    }
}

impl<I: 'static, B: TimedSubmit, Message> Step<ProgramField, I, StepContext<'_, Self, B>>
    for Warehouse
where
    ExecutionState<Pause>: Execute<I, Message = Message, Effect = (), Error = eyre::Report>,
    Message: ReusableMessageRecipe<Self, ProgramField, I, Module<Self>> + 'static,
{
    fn step(&mut self, instruction: &I, context: &StepContext<'_, Self, B>) -> Result<()> {
        let route = step_graph::<I>()
            .begin(prepare_message_at::<ProgramField, Message>())
            .then(execute_instruction_at::<ProgramField>())
            .finish();
        context.start(self, &route, instruction)
    }
}

impl Warehouse {
    pub(crate) fn fixture() -> (Self, Vec<Input>) {
        let (input, receivers) = [(30, 40), (40, 41)]
            .into_iter()
            .map(|(delay, sample)| {
                let (port, receiver) = oneshot::channel();
                (
                    Input {
                        delay,
                        sample,
                        port,
                    },
                    receiver,
                )
            })
            .unzip();
        let machine = Self {
            execution: ExecutionState::default(),
            printer: Printer::default(),
            scanner: AsyncSlot::new(Scanner),
            records: Records::default(),
            inputs: ScanInputs(receivers),
            minimum_reading: 10,
        };
        (machine, input)
    }

    pub(crate) fn printed(&self) -> usize {
        self.printer.issued
    }
    pub(crate) fn labels(&self) -> &[u32] {
        &self.records.labels
    }
    pub(crate) fn readings(&self) -> &[u16] {
        &self.records.readings
    }
}
