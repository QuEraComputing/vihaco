---
layout: ../../layouts/Guide.astro
title: Building Components
slug: components
description: "Components are the basic execution units in vihaco — an instruction type, an optional message, an optional effect, and one #[component(...)] impl that executes the instruction."
---

# Building Components With `vihaco`

Components are the basic execution units in `vihaco`.
You define:

- an instruction type
- an optional resolved message type
- an optional effect type
- one `#[component(...)]` impl that executes the instruction

This guide shows the current public authoring model for defining your own component.

If you want a focused guide to instruction enums, explicit instruction width, and nested composite-level wrappers, read [Defining Instructions With `vihaco`](/guide/instructions).
If you want a focused guide to resolved execution input and composite-side message generation, read [Using Messages With `vihaco`](/guide/messages).

## The Core Pieces

A component usually starts with two or three data types:

- an instruction enum with `#[derive(Instruction)]`
- a message type with `#[derive(Message)]` when execution needs pre-resolved input
- one or more plain Rust effect types when execution needs to return output

Use them this way:

- `Instruction`: the operation the component should execute
- `Message`: resolved execution input delivered into the component for that step
- `Effect`: value returned from execution and later interpreted by the runtime or delivered to observers

Example:

```rust
use eyre::Result;
use vihaco::{Effects, Instruction, Message, component};

#[derive(Debug, Clone, Instruction)]
pub enum CounterInst {
    Add(i64),
    Print,
}

#[derive(Debug, Clone, Message)]
pub struct PrintPrefix(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdoutEffect(pub String);

#[derive(Debug, Default)]
pub struct Counter {
    value: i64,
}
```

## Defining `#[component(...)]`

Component execution lives on an impl block annotated with `#[component(...)]`.

```rust
# use eyre::Result;
# use vihaco::{Effects, Instruction, Message, component};
# #[derive(Debug, Clone, Instruction)]
# pub enum CounterInst { Add(i64), Print }
# #[derive(Debug, Clone, Message)]
# pub struct PrintPrefix(pub String);
# #[derive(Debug, Clone, PartialEq, Eq)]
# pub struct StdoutEffect(pub String);
# #[derive(Debug, Default)]
# pub struct Counter { value: i64 }
#[component(instruction = CounterInst, message = PrintPrefix, effect = StdoutEffect)]
impl Counter {
    fn execute(&mut self, inst: CounterInst, msg: PrintPrefix) -> Result<Effects<StdoutEffect>> {
        match inst {
            CounterInst::Add(v) => {
                self.value += v;
                Ok(Effects::none())
            }
            CounterInst::Print => Ok(Effects::one(StdoutEffect(format!(
                "{}{}",
                msg.0, self.value
            )))),
        }
    }
}
```

The execution method shape is:

```rust ignore
fn execute(&mut self, inst: Inst, msg: Msg) -> eyre::Result<Effects<Effect>>
```

Important points:

- `Inst` must match the `instruction = ...` type
- `Msg` must match the `message = ...` type
- when `effect = ...` is omitted, the effect type defaults to `()`
- normal execution output is returned as `Effects<Effect>`

It is useful to keep the data flow straight:

- `Message` goes into a component
- `Effect` comes out of a component
- components consume `Message`
- runtimes and observers consume `Effect`

## When To Use `message = ()`

Use `message = ()` when the component can execute directly from its instruction and local state.

```rust
use eyre::Result;
use vihaco::{Effects, Instruction, component};

#[derive(Debug, Clone, Instruction)]
pub enum LampInst {
    On,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LampChanged(pub bool);

#[derive(Debug, Default)]
pub struct Lamp {
    on: bool,
}

#[component(instruction = LampInst, message = (), effect = LampChanged)]
impl Lamp {
    fn execute(&mut self, inst: LampInst, _msg: ()) -> Result<Effects<LampChanged>> {
        self.on = matches!(inst, LampInst::On);
        Ok(Effects::one(LampChanged(self.on)))
    }
}
```

Use a non-unit message when execution needs resolved data that should not be encoded directly in the instruction itself.

As a rule:

- use `Message` for step-local execution input
- use `Effect` for values the runtime should interpret or deliver after execution

## Execution Surface

Component execution depends only on explicit inputs and returned effects.

- `Instruction` and `Message` are the full inputs to `execute(...)`
- `Effects<Effect>` is the full output from `execute(...)`
- runtimes decide how to interpret returned effects after execution

## Design Guidance

- Put bytecode-visible execution variants in the instruction enum.
- Put resolved execution input in the message type.
- Put follow-up outputs in plain effect types.
- Keep the component responsible for its own state mutation.
- Use `effect = StepOutcome` when a component needs to return control-flow signals.

## Returning A Custom Effect

By default, `execute(...)` returns `Result<Effects<()>>`. When a component needs to return a real effect, use the `effect` parameter:

```rust
use vihaco::{Effects, Instruction, Message, component};
use vihaco_cpu::StepOutcome;

#[derive(Debug, Clone, Instruction)]
pub enum CpuInst {
    Nop,
    Halt,
}

#[derive(Debug, Clone, Message)]
pub struct CpuMsg;

pub struct CpuCore;

#[component(instruction = CpuInst, message = CpuMsg, effect = StepOutcome)]
impl CpuCore {
    fn execute(&mut self, inst: CpuInst, _msg: CpuMsg) -> eyre::Result<Effects<StepOutcome>> {
        match inst {
            CpuInst::Nop => Ok(Effects::one(StepOutcome::Continue)),
            CpuInst::Halt => Ok(Effects::one(StepOutcome::Halt)),
        }
    }
}
```

The `effect` parameter is optional. When omitted, the macro sets `type Effect = ()`. When present, the component's `GeneratedComponent::Effect` type matches what you specify.

**Important:** effects only matter when some runtime continues them. In practice:

- Hand-written runtime code can call `execute_generated` directly and extract the returned effects. For single-effect control flow, `expect_exactly_one_effect(...)` is the common helper.
- When a runtime needs to mix control-flow effects with other follow-ups, it usually defines a runtime-local sum-effect enum, gathers those values, and continues them in one place.
- Transitional `#[composite]` wiring generates the device dispatch and metadata; continuing returned effects to observers is something the hand-written runtime does (see [Defining A Composite With `vihaco`](/guide/composites)), and it does not interpret `StepOutcome` for you.

As a rule: use plain effect types for observer-delivered outputs, and use runtime-local sum-effect enums when a hand-written runtime needs extra per-step interpretation.

## What Comes Next

Once you have one or more components, the next step is to understand how observer types consume the returned effects.

Continue with [Observing Effects With `#[observe]`](/guide/observers).
