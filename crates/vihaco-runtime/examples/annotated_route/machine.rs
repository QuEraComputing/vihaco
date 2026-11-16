// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! User-authored waveform controller: message preparation, calibrated pulse
//! execution, DAC delivery, and monitoring. Timing varies across three routes.

use super::{
    contexts::{DynamicLatency, FixedLatencyNs, TraceLabel},
    framework::{Context, Receive, StaticContext, branch, route},
};
use eyre::{Result, ensure, eyre};
use std::time::Duration;
use vihaco_runtime::{BuildMessage, Execute, Field, Step, component, composite, dialect, field};

#[dialect]
#[vihaco(crate = ::vihaco_runtime)]
pub(super) mod commands {
    // Operands: output channel and amplitude multiplier. Runtime variants also
    // carry execution/delivery delays in nanoseconds for this demonstration.
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
struct DacField;
struct MonitorField;
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

// One topology: build Source's message, execute Fire once, propagate its effect.
impl<C> Step<field!(Machine::source), commands::Fire, C> for Machine
where
    C: StaticContext<Machine, FixedLatencyNs<3>>
        + StaticContext<Machine, FixedLatencyNs<7>>
        + Context<Machine, TraceLabel>,
{
    fn step(&mut self, instruction: &commands::Fire, context: &C) -> Result<()> {
        let topology = route::<Machine>()
            .at::<field!(Machine::source)>()
            .build_message::<commands::Fire>()
            .execute()
            .annotate::<FixedLatencyNs<3>>()
            .fan_out((
                branch()
                    .deliver_to::<DacField>()
                    .annotate::<FixedLatencyNs<7>>(),
                branch()
                    .deliver_to::<MonitorField>()
                    .annotate_with(TraceLabel("monitor delivery")),
            ));
        // Inspection reads structure and types; it performs no machine work.
        let description = topology.topology();
        assert_eq!(
            description
                .iter()
                .filter(|line| line.starts_with("execute instruction"))
                .count(),
            1
        );
        assert!(
            description
                .iter()
                .any(|line| line.starts_with("deliver effect to dac"))
        );
        assert!(
            description
                .iter()
                .any(|line| line.starts_with("deliver effect to monitor"))
        );
        assert_eq!(description.len(), 5);
        topology.start(self, context, instruction)
    }
}
impl<C> Step<field!(Machine::source), commands::FireRuntime, C> for Machine
where
    C: Context<Machine, DynamicLatency> + Context<Machine, TraceLabel>,
{
    fn step(&mut self, instruction: &commands::FireRuntime, context: &C) -> Result<()> {
        route::<Machine>()
            .at::<field!(Machine::source)>()
            .build_message::<commands::FireRuntime>()
            .execute()
            .annotate_with(DynamicLatency(Duration::from_nanos(instruction.2)))
            .fan_out((
                branch()
                    .deliver_to::<DacField>()
                    .annotate_with(DynamicLatency(Duration::from_nanos(instruction.3))),
                branch()
                    .deliver_to::<MonitorField>()
                    .annotate_with(TraceLabel("monitor delivery")),
            ))
            .start(self, context, instruction)
    }
}
impl<C> Step<field!(Machine::source), commands::FireMixed, C> for Machine
where
    C: StaticContext<Machine, FixedLatencyNs<3>>
        + Context<Machine, DynamicLatency>
        + Context<Machine, TraceLabel>,
{
    fn step(&mut self, instruction: &commands::FireMixed, context: &C) -> Result<()> {
        route::<Machine>()
            .at::<field!(Machine::source)>()
            .build_message::<commands::FireMixed>()
            .execute()
            .annotate::<FixedLatencyNs<3>>()
            .fan_out((
                branch()
                    .deliver_to::<DacField>()
                    .annotate_with(DynamicLatency(Duration::from_nanos(instruction.2))),
                branch()
                    .deliver_to::<MonitorField>()
                    .annotate_with(TraceLabel("monitor delivery")),
            ))
            .start(self, context, instruction)
    }
}
