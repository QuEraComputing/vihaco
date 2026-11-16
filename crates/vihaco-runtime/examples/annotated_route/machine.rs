// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! User-authored waveform controller: message preparation, calibrated pulse
//! execution, DAC delivery, and monitoring. Timing varies across three routes.

use super::{
    annotations::{ComputeTime, ConstNs, DeliveryLatency, RuntimeTime, TraceLabel},
    framework::{Computation, Context, Receive, Route, Wire, deliver_to, route_through},
};
use eyre::{Result, ensure, eyre};
use std::time::Duration;
use vihaco_runtime::{BuildMessage, Execute, Field, Step, component, composite, dialect, field};

#[dialect]
#[vihaco(crate = ::vihaco_runtime)]
pub(super) mod commands {
    // Operands: output channel and amplitude multiplier. Runtime variants also
    // carry execution duration/delivery latency in nanoseconds for this demonstration.
    #[instruction]
    #[derive(Clone)]
    pub struct Fire(pub u32, pub u32);

    #[instruction]
    #[derive(Clone)]
    pub struct FireRuntime(pub u32, pub u32, pub u64, pub u64);

    #[instruction]
    #[derive(Clone)]
    pub struct FireMixed(pub u32, pub u32, pub u64);
}

trait PulseInstruction {
    fn channel(&self) -> u32;
    fn amplitude(&self) -> u32;
}
macro_rules! pulse_operands {
    ($($instruction:ty),+ $(,)?) => {$(
        impl PulseInstruction for $instruction {
            fn channel(&self) -> u32 { self.0 }
            fn amplitude(&self) -> u32 { self.1 }
        }
    )+};
}
pulse_operands!(commands::Fire, commands::FireRuntime, commands::FireMixed);

struct ChannelConfig {
    samples: Vec<f64>,
    gain: f64,
    offset: f64,
}

// Runtime inputs resolved from the machine, not encoded in the instruction.
// Owned snapshots allow calibration/waveform storage to change after submission.
pub(super) struct PulseMessage {
    samples: Vec<f64>,
    gain: f64,
    offset: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Waveform {
    pub(super) channel: u32,
    pub(super) samples: Vec<f64>,
}

#[component]
#[vihaco(crate = ::vihaco_runtime)]
#[instructions { commands }]
#[derive(Default)]
pub(super) struct Source {
    pub(super) executions: usize,
}

impl<I: PulseInstruction> Execute<I> for Source {
    type Message = PulseMessage;
    type Effect = Waveform;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &I, message: PulseMessage) -> Result<Waveform> {
        let scale = f64::from(instruction.amplitude()) * message.gain;
        let samples: Vec<_> = message
            .samples
            .into_iter()
            .map(|sample| sample * scale + message.offset)
            .collect();
        ensure!(
            samples.iter().all(|sample| sample.is_finite()),
            "non-finite pulse sample"
        );
        self.executions += 1;
        Ok(Waveform {
            channel: instruction.channel(),
            samples,
        })
    }
}

#[derive(Default)]
pub(super) struct Dac {
    pub(super) outputs: Vec<Waveform>,
}
impl Receive<Waveform> for Dac {
    fn receive(&mut self, waveform: Waveform) -> Result<()> {
        // Model a DAC with an output range of [-2, 2].
        ensure!(
            waveform
                .samples
                .iter()
                .all(|sample| (-2.0..=2.0).contains(sample)),
            "waveform exceeds DAC output range"
        );
        self.outputs.push(waveform);
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct Measurement {
    pub(super) channel: u32,
    pub(super) sample_count: usize,
    pub(super) peak: f64,
}

#[derive(Default)]
pub(super) struct Monitor {
    pub(super) records: Vec<Measurement>,
}
impl Receive<Waveform> for Monitor {
    fn receive(&mut self, waveform: Waveform) -> Result<()> {
        self.records.push(Measurement {
            channel: waveform.channel,
            sample_count: waveform.samples.len(),
            peak: waveform
                .samples
                .iter()
                .map(|sample| sample.abs())
                .fold(0.0, f64::max),
        });
        Ok(())
    }
}

#[composite]
#[vihaco(crate = ::vihaco_runtime)]
pub(super) struct Machine {
    #[device(1)]
    pub(super) source: Source,
    pub(super) dac: Dac,
    pub(super) monitor: Monitor,
    channels: Vec<ChannelConfig>,
    pub(super) preparations: usize,
}
// Pure effect receivers have no instruction inventory. The current composite
// macro generates Field routes for instruction devices only, so these two
// explicit accessors bridge ordinary machine state into the prototype topology.
pub(super) struct DacField;
pub(super) struct MonitorField;
impl Field<DacField> for Machine {
    type Child = Dac;
    const SOURCE: &'static [&'static str] = &["dac"];
    fn get(&self) -> &Dac {
        &self.dac
    }
    fn get_mut(&mut self) -> &mut Dac {
        &mut self.dac
    }
}
impl Field<MonitorField> for Machine {
    type Child = Monitor;
    const SOURCE: &'static [&'static str] = &["monitor"];
    fn get(&self) -> &Monitor {
        &self.monitor
    }
    fn get_mut(&mut self) -> &mut Monitor {
        &mut self.monitor
    }
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            source: Source::default(),
            dac: Dac::default(),
            monitor: Monitor::default(),
            channels: vec![ChannelConfig {
                samples: vec![0.25, -0.5, 1.0],
                gain: 0.5,
                offset: 0.125,
            }],
            preparations: 0,
        }
    }
}

impl<I: PulseInstruction> BuildMessage<field!(Machine::source), I, (), PulseMessage> for Machine {
    fn build_message(&mut self, instruction: &I, _: &()) -> Result<PulseMessage> {
        let channel = usize::try_from(instruction.channel())?;
        let config = self
            .channels
            .get(channel)
            .ok_or_else(|| eyre!("unknown channel {channel}"))?;
        ensure!(!config.samples.is_empty(), "empty pulse waveform");
        ensure!(
            config.gain.is_finite() && config.offset.is_finite(),
            "invalid calibration"
        );
        ensure!(
            config.samples.iter().all(|sample| sample.is_finite()),
            "invalid waveform sample"
        );
        let message = PulseMessage {
            samples: config.samples.clone(),
            gain: config.gain,
            offset: config.offset,
        };
        self.preparations += 1;
        Ok(message)
    }
}

// Interpreter capabilities used by this machine's three timing variants.
// Existing interpreters qualify automatically; no manual marker impl is needed.
pub(super) trait MachineContext:
    Context<Machine, ComputeTime, Computation>
    + Context<Machine, DeliveryLatency, Wire>
    + Context<Machine, TraceLabel, Wire>
{
}

impl<C> MachineContext for C where
    C: Context<Machine, ComputeTime, Computation>
        + Context<Machine, DeliveryLatency, Wire>
        + Context<Machine, TraceLabel, Wire>
{
}

// Each route exposes four computation nodes and three payload-transfer edges.
impl<C: MachineContext> Step<field!(Machine::source), commands::Fire, C> for Machine {
    fn step(&mut self, instruction: &commands::Fire, context: &C) -> Result<()> {
        route_through::<field!(Machine::source)>()
            .build_message::<commands::Fire>()
            .annotate::<ComputeTime<ConstNs<2>>>()
            .forward_message()
            .annotate::<DeliveryLatency<ConstNs<1>>>()
            .execute()
            .annotate::<ComputeTime<ConstNs<3>>>()
            .fan_out((
                deliver_to::<DacField>()
                    .annotate::<DeliveryLatency<ConstNs<7>>>()
                    .handle_effect()
                    .annotate::<ComputeTime<ConstNs<4>>>(),
                deliver_to::<MonitorField>()
                    .annotate_with(TraceLabel("monitor delivery"))
                    .handle_effect()
                    .annotate::<ComputeTime<ConstNs<1>>>(),
            ))
            .start(self, context, instruction)
    }
}
impl<C: MachineContext> Step<field!(Machine::source), commands::FireRuntime, C> for Machine {
    fn step(&mut self, instruction: &commands::FireRuntime, context: &C) -> Result<()> {
        route_through::<field!(Machine::source)>()
            .build_message::<commands::FireRuntime>()
            .annotate_with(ComputeTime(RuntimeTime(Duration::from_nanos(2))))
            .forward_message()
            .annotate_with(DeliveryLatency(RuntimeTime(Duration::from_nanos(1))))
            .execute()
            .annotate_with(ComputeTime(RuntimeTime(Duration::from_nanos(
                instruction.2,
            ))))
            .fan_out((
                deliver_to::<DacField>()
                    .annotate_with(DeliveryLatency(RuntimeTime(Duration::from_nanos(
                        instruction.3,
                    ))))
                    .handle_effect()
                    .annotate_with(ComputeTime(RuntimeTime(Duration::from_nanos(4)))),
                deliver_to::<MonitorField>()
                    .annotate_with(TraceLabel("monitor delivery"))
                    .handle_effect()
                    .annotate_with(ComputeTime(RuntimeTime(Duration::from_nanos(1)))),
            ))
            .start(self, context, instruction)
    }
}
impl<C: MachineContext> Step<field!(Machine::source), commands::FireMixed, C> for Machine {
    fn step(&mut self, instruction: &commands::FireMixed, context: &C) -> Result<()> {
        route_through::<field!(Machine::source)>()
            .build_message::<commands::FireMixed>()
            .annotate::<ComputeTime<ConstNs<2>>>()
            .forward_message()
            .annotate::<DeliveryLatency<ConstNs<1>>>()
            .execute()
            .annotate::<ComputeTime<ConstNs<3>>>()
            .fan_out((
                deliver_to::<DacField>()
                    .annotate_with(DeliveryLatency(RuntimeTime(Duration::from_nanos(
                        instruction.2,
                    ))))
                    .handle_effect()
                    .annotate::<ComputeTime<ConstNs<4>>>(),
                deliver_to::<MonitorField>()
                    .annotate_with(TraceLabel("monitor delivery"))
                    .handle_effect()
                    .annotate::<ComputeTime<ConstNs<1>>>(),
            ))
            .start(self, context, instruction)
    }
}

impl Machine {
    pub(super) fn set_calibration_gain(&mut self, gain: f64) {
        self.channels[0].gain = gain;
    }
}
