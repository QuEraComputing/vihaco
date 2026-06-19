---
layout: ../../layouts/Guide.astro
title: Observing Effects
slug: observers
description: How #[observe] works — declaring Observe<EffectType> impls on standalone observers and on components that also react to delivered effects.
---

# Observing Effects With `#[observe]`

`vihaco` separates execution from effect delivery:

- components execute instructions and return effects
- `#[observe]` lets any type react to delivered effect types
- a runtime wires effect delivery together

This guide explains what `#[observe]` is for and how to use it, both on standalone observer types and on components alike.

## What `#[observe]` Looks Like

`#[observe(EffectType)]` goes on an impl block. It declares which delivered effect types the type handles and generates the `Observe<EffectType>` trait impl.

```rust
use eyre::Result;
use vihaco::{Effects, observe};

#[derive(Debug, Clone)]
pub struct StdoutEffect(pub String);

#[derive(Debug, Default)]
pub struct StdoutCollector {
    lines: Vec<String>,
}

#[observe(StdoutEffect)]
impl StdoutCollector {
    fn observe_stdout_effect(&mut self, effect: &StdoutEffect) -> Result<Effects<()>> {
        self.lines.push(effect.0.clone());
        Ok(Effects::none())
    }
}
```

For a plain observer, the handler method:

- takes `&mut self`
- takes `&EffectType`
- returns `Result<Effects<FollowUpEffect>, Error>`
- must be named `observe_<snake_case_effect_type>` such as `observe_stdout_effect`

The macro generates an `Observe<StdoutEffect>` trait impl that delegates to the handler method.

Observer handlers can also synthesize follow-up effects. If a handler returns values instead of `Effects::none()`, the runtime continues them in declared depth-first order.

### Multiple Handlers Per Effect

You can define multiple handler methods for the same effect type by adding a suffix after the base name:

```rust
# use eyre::Result;
# use vihaco::{Effects, observe};
# #[derive(Debug, Clone)]
# pub struct ChannelFrame;
# #[derive(Debug, Default)]
# pub struct Oscilloscope { samples: Vec<ChannelFrame> }
#[observe(ChannelFrame, effect = ())]
impl Oscilloscope {
    fn observe_channel_frame_capture(&mut self, effect: &ChannelFrame) -> Result<Effects<()>> {
        self.samples.push(effect.clone());
        Ok(Effects::none())
    }

    fn observe_channel_frame_log(&mut self, effect: &ChannelFrame) -> Result<Effects<()>> {
        println!("frame received: {:?}", effect);
        Ok(Effects::none())
    }
}
```

All methods matching `observe_<snake_case>` or `observe_<snake_case>_*` are called when the effect is delivered.

### Multiple Effect Types

A single `#[observe]` block can handle multiple delivered effect types:

```rust
# use eyre::Result;
# use vihaco::{Effects, observe};
# #[derive(Debug, Clone)]
# pub struct StdoutEffect(pub String);
# #[derive(Debug, Clone)]
# pub struct ChannelSample { pub value: f64 }
# #[derive(Debug, Default)]
# pub struct MultiObserver;
#[observe(StdoutEffect, ChannelSample, effect = ())]
impl MultiObserver {
    fn observe_stdout_effect(&mut self, effect: &StdoutEffect) -> Result<Effects<()>> {
        let _ = effect;
        Ok(Effects::none())
    }

    fn observe_channel_sample(&mut self, effect: &ChannelSample) -> Result<Effects<()>> {
        let _ = effect;
        Ok(Effects::none())
    }
}
```

The macro generates a separate `Observe<T>` impl for each listed effect type.

## When To Declare `effect = ...`

An `#[observe(...)]` block defaults to a `()` follow-up effect type. Declare an explicit follow-up type with `effect = ...` once the boundary does typed continuation work instead of a simple `Effects<()>` handoff. In practice, write `effect = CompositeEffect` on the `#[observe(...)]` block when any of these are true:

- the same `#[observe(...)]` block handles multiple delivered effect types
- the delivered effect has multiple matching handler methods
- any handler returns typed follow-up effects instead of `Effects<()>`

That keeps continuation explicit and allows each child observer to return its own local follow-up type as long as it converts into the composite effect with `Into`.

```rust
use eyre::Result;
use vihaco::{Effects, Observe, observe};

#[derive(Debug, Clone)]
pub struct ChannelFrame;

#[derive(Debug, Clone)]
pub struct FrameRendered;

#[derive(Debug, Clone)]
pub enum RuntimeEffect {
    Rendered(FrameRendered),
}

impl From<FrameRendered> for RuntimeEffect {
    fn from(value: FrameRendered) -> Self {
        Self::Rendered(value)
    }
}

#[derive(Default)]
pub struct Display;

impl Observe<ChannelFrame> for Display {
    type Effect = FrameRendered;
    type Error = eyre::Report;

    fn observe(&mut self, effect: &ChannelFrame) -> Result<Effects<Self::Effect>> {
        let _ = effect;
        Ok(Effects::one(FrameRendered))
    }
}

#[derive(Default)]
pub struct Runtime {
    display: Display,
}

#[observe(ChannelFrame, effect = RuntimeEffect)]
impl Runtime {
    fn observe_channel_frame(&mut self, effect: &ChannelFrame) -> Result<Effects<RuntimeEffect>> {
        Ok(Observe::<ChannelFrame>::observe(&mut self.display, effect)?.map(Into::into))
    }
}
```

## The Observe Trait

`Observe` is effect-only:

```rust ignore
pub trait Observe<E: 'static> {
    type Effect: 'static;
    type Error;

    fn observe(&mut self, effect: &E) -> Result<Effects<Self::Effect>, Self::Error>;
}
```

Observers receive only the delivered effect. If an observer needs extra data, use one of these two patterns:

- put the needed data into a staged follow-up effect
- store the needed state inside the observing component and update it through earlier effects

## Standalone Observers

The simplest use of `#[observe]` is on a type that only reacts to delivered effects with no instructions or messages of its own:

```rust
# use eyre::Result;
# use vihaco::{Effects, observe};
# #[derive(Debug, Clone)]
# pub struct StdoutEffect(pub String);
#[derive(Debug, Default)]
pub struct StdoutCollector {
    lines: Vec<String>,
}

#[observe(StdoutEffect)]
impl StdoutCollector {
    fn observe_stdout_effect(&mut self, effect: &StdoutEffect) -> Result<Effects<()>> {
        self.lines.push(effect.0.clone());
        Ok(Effects::none())
    }
}
```

A composite owns such an observer as an ordinary field. The `#[observe]` derive gives the field type an `Observe<StdoutEffect>` impl; the runtime delivers effects to it by calling that impl (see [Wire It Together](#wire-it-together) below):

```rust ignore
use vihaco::composite;

#[composite]
#[derive(Debug, Default)]
pub struct WaveComposite {
    #[device(0x00, alias = "wave")]
    wave: WaveGenerator,

    // A plain field; the runtime delivers StdoutEffect to it explicitly.
    stdout: StdoutCollector,
}
```

`#[composite]` is transitional scaffolding for the generated device wiring (the outer instruction enum and the device metadata). The underlying model is still ordinary component execution plus typed effect observation, continued by hand-written runtime code.

## Components That Also Observe

`#[observe]` is not limited to standalone observer types. A component that executes instructions can also observe delivered effects.

The important design shift is that the observer sees the effect directly. If it needs post-processed data, an earlier step should emit a richer staged effect rather than relying on borrowed context.

```rust ignore
use eyre::Result;
use vihaco::{Effects, component, observe};

pub struct ChannelFrame {
    pub channel: u32,
    pub value: f64,
}

pub struct FrameRendered {
    pub frame: ChannelFrame,
    pub markers: Vec<[f64; 2]>,
}

pub enum DisplayOutcome {
    Ready(f64),
}

#[component(instruction = DisplayInst, message = DisplayMsg, effect = DisplayOutcome)]
impl Display {
    fn execute(
        &mut self,
        inst: DisplayInst,
        msg: DisplayMsg,
    ) -> Result<Effects<DisplayOutcome>> {
        let _ = (inst, msg);
        Ok(Effects::none())
    }
}

#[observe(FrameRendered)]
impl Display {
    fn observe_frame_rendered(
        &mut self,
        effect: &FrameRendered,
    ) -> Result<Effects<()>> {
        let _ = effect;
        Ok(Effects::none())
    }
}
```

A runtime can stage that richer effect explicitly:

1. a `Renderer` observes `ChannelFrame`
2. it updates local render state
3. it emits `FrameRendered { frame, markers }`
4. `Display` observes `FrameRendered`

That keeps all continuation explicit in the effect types.

## Delivery Ordering

Effect delivery is performed by the runtime, and the convention is to follow the composite's field order and the continuation graph.

For multiple follow-up effects returned as `Effects::Many(...)`, continue them left-to-right and depth-first. That means:

- the first follow-up effect is fully continued before the second begins
- ordering should usually be expressed through staged effect types
- the types should make the stages visible, regardless of how the wiring is written

## A Complete Example

The example below shows the full picture:

- a component that returns effects
- a standalone observer
- a component that also observes

### Define The Types

```rust
use eyre::Result;
use vihaco::{Effects, Instruction, Message, component, observe};

#[derive(Debug, Clone, Instruction)]
pub enum WaveInst {
    SetAmplitude(f64),
    Play,
}

#[derive(Debug, Clone, Message)]
pub struct PlayMsg {
    pub when_ns: u64,
    pub channel_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StdoutEffect(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelSample {
    pub when_ns: u64,
    pub channel_id: u32,
    pub value: f64,
}
```

### The Producing Component

```rust
# use eyre::Result;
# use vihaco::{Effects, Instruction, Message, component};
# #[derive(Debug, Clone, Instruction)]
# pub enum WaveInst { SetAmplitude(f64), Play }
# #[derive(Debug, Clone, Message)]
# pub struct PlayMsg { pub when_ns: u64, pub channel_id: u32 }
# #[derive(Debug, Clone, PartialEq)]
# pub struct ChannelSample { pub when_ns: u64, pub channel_id: u32, pub value: f64 }
#[derive(Debug, Default)]
pub struct WaveGenerator {
    amplitude: f64,
}

#[component(instruction = WaveInst, message = PlayMsg, effect = ChannelSample)]
impl WaveGenerator {
    fn execute(&mut self, inst: WaveInst, msg: PlayMsg) -> Result<Effects<ChannelSample>> {
        match inst {
            WaveInst::SetAmplitude(v) => {
                self.amplitude = v;
                Ok(Effects::none())
            }
            WaveInst::Play => Ok(Effects::one(ChannelSample {
                when_ns: msg.when_ns,
                channel_id: msg.channel_id,
                value: self.amplitude,
            })),
        }
    }
}
```

### A Standalone Observer

```rust
# use eyre::Result;
# use vihaco::{Effects, observe};
# #[derive(Debug, Clone)]
# pub struct StdoutEffect(pub String);
#[derive(Debug, Default)]
pub struct StdoutCollector {
    lines: Vec<String>,
}

#[observe(StdoutEffect)]
impl StdoutCollector {
    fn observe_stdout_effect(&mut self, effect: &StdoutEffect) -> Result<Effects<()>> {
        self.lines.push(effect.0.clone());
        Ok(Effects::none())
    }
}
```

### A Component That Also Observes

```rust
# use eyre::Result;
# use vihaco::{Effects, Instruction, component, observe};
# #[derive(Debug, Clone, PartialEq)]
# pub struct StdoutEffect(pub String);
# #[derive(Debug, Clone, PartialEq)]
# pub struct ChannelSample { pub when_ns: u64, pub channel_id: u32, pub value: f64 }
#[derive(Debug, Default)]
pub struct Recorder {
    samples: Vec<ChannelSample>,
    count: usize,
}

#[derive(Debug, Clone, Instruction)]
pub enum RecorderInst {
    GetCount,
}

#[component(instruction = RecorderInst, message = (), effect = StdoutEffect)]
impl Recorder {
    fn execute(&mut self, inst: RecorderInst, _msg: ()) -> Result<Effects<StdoutEffect>> {
        match inst {
            RecorderInst::GetCount => Ok(Effects::one(StdoutEffect(format!(
                "recorded {} samples",
                self.count
            )))),
        }
    }
}

#[observe(ChannelSample)]
impl Recorder {
    fn observe_channel_sample(&mut self, effect: &ChannelSample) -> Result<Effects<()>> {
        self.samples.push(effect.clone());
        self.count += 1;
        Ok(Effects::none())
    }
}
```

### Wire It Together

`#[composite]` generates the device wiring; the runtime executes a component and then delivers its effects to the matching observers by calling their `Observe` impls.

```rust ignore
use vihaco::{GeneratedComponent, Observe, composite};

#[composite]
#[derive(Debug, Default)]
pub struct WaveComposite {
    #[device(0x00, alias = "wave")]
    wave: WaveGenerator,

    #[device(0x01, alias = "recorder")]
    recorder: Recorder,

    // Plain observer field — delivered to by hand below.
    stdout: StdoutCollector,
}

impl WaveComposite {
    fn play(&mut self, msg: PlayMsg) -> eyre::Result<()> {
        // 1. WaveGenerator executes Play and returns a ChannelSample.
        let samples = self.wave.execute_generated(WaveInst::Play, msg)?;
        // 2. Deliver each ChannelSample to the Recorder (which observes it).
        for sample in samples {
            Observe::<ChannelSample>::observe(&mut self.recorder, &sample)?;
        }
        Ok(())
    }

    fn report(&mut self) -> eyre::Result<()> {
        // 3. Recorder executes GetCount and returns a StdoutEffect...
        let lines = self.recorder.execute_generated(RecorderInst::GetCount, ())?;
        // 4. ...which the runtime delivers to the StdoutCollector.
        for line in lines {
            Observe::<StdoutEffect>::observe(&mut self.stdout, &line)?;
        }
        Ok(())
    }
}
```

The flow:

1. `WaveGenerator` executes `Play` and returns a `ChannelSample`.
2. The runtime delivers that `ChannelSample` to `Recorder`.
3. `Recorder` updates local state and returns `Effects::none()` from its observer handler.
4. When `Recorder` later executes `GetCount`, it returns a `StdoutEffect`, and the runtime delivers that effect to `StdoutCollector`.

## Design Guidance

- Use standalone `#[observe]` when a type only reacts to effects.
- Use `#[observe]` alongside `#[component]` when a device needs to react to effects from other components.
- Make effect types plain standalone Rust types.
- Prefer putting `#[observe]` on the real consumer type, not a forwarding wrapper.
- If a type is conceptually a log sink, recorder, projection, renderer, or simulation consumer with no instructions of its own, model it as a standalone observer.
- Prefer staged follow-up effects over hidden cross-field delivery context.

## What Comes Next

After understanding `#[observe]`, the next step is to see how composite wiring ties instruction dispatch and effect continuation together.

Continue with [Defining A Composite With `vihaco`](/guide/composites).
