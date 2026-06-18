---
layout: ../../layouts/Guide.astro
title: Using Messages
slug: messages
description: Message is the resolved execution input for a component — how a crate author defines a message type and how a composite resolves and supplies it during execution.
---

# Using Messages With `vihaco`

`Message` is the resolved execution input for a component.

This is the key logic to keep in mind:

- instructions tell a component what operation to perform
- messages provide execution input that the composite runtime resolves or generates
- effects are returned after execution and later interpreted by the runtime or delivered to observers

That means:

- components consume `Message`
- composites resolve or build `Message`
- observers do not consume `Message`
- observers do not receive a separate delivery context; any extra data should be staged into effects or owned locally

This guide focuses on both sides of that contract:

- how a crate author defines a message type
- how a composite author resolves and supplies messages during execution

If you have not read the instruction guide yet, start with [Defining Instructions With `vihaco`](/guide/instructions).

## What A Message Is For

Use a message when a component needs step-local execution input that should not live directly in the instruction encoding.

For example, a composite runtime may need to:

- look up runtime state
- pop values from a stack
- derive timing information
- validate access to a device before execution

The composite can do that work first, then pass the result into the component as a message.

That keeps responsibilities clean:

- `Instruction` is the bytecode-visible request
- `Message` is the resolved input to execute that request
- `Effect` is the value returned after execution

## A Small Message Type

Message types are usually plain Rust types annotated with `#[derive(Message)]`.

```rust
use vihaco::Message;

#[derive(Debug, Clone, Message)]
pub struct PlayMsg {
    pub when_ns: u64,
    pub channel_id: u32,
}
```

A component can then declare that message type in its `#[component(...)]` impl:

```rust
use eyre::Result;
use vihaco::{Effects, Instruction, Message, component};

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

#[derive(Debug, Clone)]
pub struct ChannelSample {
    pub when_ns: u64,
    pub channel_id: u32,
    pub value: f64,
}

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

The important thing is that `WaveGenerator` does not decide `when_ns` or `channel_id`.
It just consumes the already-resolved `PlayMsg`.

## Why The Composite Owns Message Resolution

The composite runtime is the right place to build messages because it owns the broader execution context.

That often includes:

- cross-component state
- scheduler or runtime state
- stacks, clocks, frames, or device metadata
- validation and access control

The component should not have to reconstruct that context on its own.

So the execution flow usually looks like this:

1. the composite receives or dispatches an instruction
2. the composite inspects runtime state and the instruction
3. the composite builds the message
4. the composite executes the component with `(instruction, message)`

That is the mental model to keep throughout the rest of this guide.

## A Small Composite-Author Example

`#[composite]` generates the device wiring (the outer instruction enum and device metadata), but message resolution is plain Rust that you write next to the composite: build the message from runtime context, then hand `(instruction, message)` to the component via the generated `execute_generated` method.

```rust
use eyre::Result;
use vihaco::{Effects, GeneratedComponent, Instruction, Message, component, composite};

#[derive(Debug, Clone, Instruction)]
enum DeviceInst {
    Pulse,
}

#[derive(Message)]
struct DeviceMsg(&'static str);

#[derive(Default)]
struct Device {
    seen: Vec<&'static str>,
}

#[component(instruction = DeviceInst, message = DeviceMsg)]
impl Device {
    fn execute(&mut self, inst: DeviceInst, msg: DeviceMsg) -> Result<Effects<()>> {
        match inst {
            DeviceInst::Pulse => {
                self.seen.push(msg.0);
                Ok(Effects::none())
            }
        }
    }
}

#[composite]
#[derive(Default)]
struct Pilot {
    #[device(0x02, alias = "pulse")]
    device: Device,
}

impl Pilot {
    // The composite owns message resolution, then executes the component.
    fn step(&mut self, inst: DeviceInst) -> Result<Effects<()>> {
        let msg = self.resolve_device(&inst)?;
        self.device.execute_generated(inst, msg)
    }

    fn resolve_device(&mut self, _inst: &DeviceInst) -> Result<DeviceMsg> {
        Ok(DeviceMsg("resolved"))
    }
}
```

This is the core composite-author contract:

- the component says which message type it needs
- the composite provides a resolver for that instruction family
- the resolver returns the message value the component will consume

In other words, the component defines the input shape, but the composite decides the actual input value for that step.

## A Richer Example: Resolving A Signal Message

A real runtime shows a richer version of the same idea. A signal-generator component expects a `SignalMessage`:

```rust ignore
use vihaco::{Effects, Message, component};

#[derive(Debug, Clone, Copy, PartialEq, Message)]
pub enum SignalMessage {
    None,
    Poly4([f64; 4]),
    Duration(u64),
}

#[component(instruction = SignalInst, message = SignalMessage)]
impl SignalGenerator {
    fn execute(&mut self, inst: SignalInst, msg: SignalMessage) -> eyre::Result<Effects<()>> {
        // component consumes a resolved message here
        let _ = (inst, msg);
        Ok(Effects::none())
    }
}
```

But the component does not know how to create `Poly4([f64; 4])` or `Duration(u64)` on its own.
That comes from composite-owned runtime state. The composite resolves the message first
(the exact accessors — a stack, a clock — depend on your runtime; the shape is what matters):

```rust ignore
fn resolve_signal(&mut self, inst: &SignalInst) -> eyre::Result<SignalMessage> {
    match inst {
        SignalInst::Poly(_addr) => {
            let p3: f64 = self.cpu.stack_pop()?.try_into()?;
            let p2: f64 = self.cpu.stack_pop()?.try_into()?;
            let p1: f64 = self.cpu.stack_pop()?.try_into()?;
            let p0: f64 = self.cpu.stack_pop()?.try_into()?;
            Ok(SignalMessage::Poly4([p0, p1, p2, p3]))
        }
        SignalInst::Play if self.signal.is_idle() => {
            let cycles: u64 = self.cpu.stack_pop()?.try_into()?;
            let duration_ns = cycles
                .checked_mul(self.clock.resolution_ns())
                .ok_or_else(|| eyre::eyre!("play duration overflow"))?;
            Ok(SignalMessage::Duration(duration_ns))
        }
        SignalInst::Play => Ok(SignalMessage::None),
    }
}
```

Then the runtime executes the component with that resolved value:

```rust ignore
use vihaco::GeneratedComponent;

let msg = self.resolve_signal(&signal_inst)?;
let effects = self.signal.execute_generated(signal_inst, msg)?;
assert_eq!(effects, Effects::one(()));
```

This example shows why composites own message resolution:

- the message depends on host stack state
- the message depends on clock resolution
- the message depends on whether the generator is idle
- the component can stay focused on execution once the message is ready

When a component returns a non-unit effect, hand-written runtimes normally either:

- extract exactly one control/data effect with `expect_exactly_one_effect(...)`, or
- lift the returned values into a runtime-local sum-effect enum and continue that effect set explicitly

## When To Use `message = ()`

Use `message = ()` when the component can execute directly from:

- the instruction itself
- the component's own local state

For example:

```rust
use eyre::Result;
use vihaco::{Effects, Instruction, component};

#[derive(Debug, Clone, Instruction)]
pub enum LampInst {
    On,
    Off,
}

#[derive(Debug, Default)]
pub struct Lamp {
    on: bool,
}

#[component(instruction = LampInst, message = ())]
impl Lamp {
    fn execute(&mut self, inst: LampInst, _msg: ()) -> Result<Effects<()>> {
        self.on = matches!(inst, LampInst::On);
        Ok(Effects::none())
    }
}
```

In this case there is nothing meaningful for the composite to resolve, so a unit message is the right fit.

## Composite Message Types

When an outer composite wraps inner components, it can also wrap their message types.

```rust
use vihaco::Message;

#[derive(Message)]
struct DemoMsg;

#[derive(Message)]
enum CompositeMsg {
    Inner(DemoMsg),
}
```

This pattern keeps the outer component or composite boundary explicit:

- outer instructions wrap inner instructions
- outer messages wrap inner messages
- routing stays visible in the outer type signatures

The same design rule applies here too: the outer composite layer decides which inner message variant to construct.

## `Instruction` Vs `Message` Vs `Effect`

A simple way to choose the right type is:

- use `Instruction` for bytecode-visible operations
- use `Message` for resolved execution input produced by the composite
- use `Effect` for returned values consumed after execution

Good candidates for `Message`:

- timing data derived from a runtime clock
- values popped from a stack before execution
- validated handles or resolved addresses
- execution-local context that should not be part of source syntax

Usually not a good fit for `Message`:

- the main operation being requested
- long-lived component state
- broadcast or runtime follow-up values that belong in the effect stream

## Practical Guidance

- Start with `message = ()` unless execution genuinely needs resolved input.
- If a component needs context from the wider runtime, prefer resolving that context into a message.
- Keep message types plain and specific to execution needs.
- Let composites do lookups, stack access, timing derivation, and validation before calling component execution.
- Keep effects separate from messages so post-execution output stays explicit.

## What Comes Next

Messages make the most sense alongside the surrounding component and composite model.

Continue with:

- [Building Components With `vihaco`](/guide/components)
- [Observing Effects With `#[observe]`](/guide/observers)
- [Defining A Composite With `vihaco`](/guide/composites)
