---
layout: ../../layouts/Guide.astro
title: Building Components
slug: components
description: "Components are the basic execution units in vihaco — a component! declaration defines its state and instruction surface, while #[dispatch(...)] supplies execution."
---

# Building Components With `vihaco`

Components are the basic execution units in `vihaco`.
You define:

- a component with `component!`, including its state and instruction surface
- an optional resolved message type
- an optional effect type
- one `#[dispatch(...)]` impl that executes the component's runtime instruction

This guide shows the current public authoring model for defining your own component.

If you want a focused guide to instruction enums, explicit instruction width, and nested composite-level wrappers, read [Defining Instructions With `vihaco`](/guide/instructions).
If you want a focused guide to resolved execution input and composite-side message generation, read [Using Messages With `vihaco`](/guide/messages).

## The Core Pieces

A component usually starts with a `component!` declaration and one or two supporting data types:

- a `component!` declaration containing the component state and instructions
- a message type with `#[derive(Message)]` when execution needs pre-resolved input
- one or more plain Rust effect types when execution needs to return output

Use them this way:

- `component!`: the component state plus its syntax and runtime instruction types
- `Message`: resolved execution input delivered into the component for that step
- `Effect`: value returned from execution and later interpreted by the runtime or delivered to observers

Example:

```rust
use eyre::Result;
use vihaco::{component, dispatch, Effects, Message};

component! {
    #[derive(Debug, Default)]
    pub component Counter {
        value: i64,
    }

    instruction {
        Add(i64),
        Print,
    }
}

#[derive(Debug, Clone, Message)]
pub struct PrintPrefix(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdoutEffect(pub String);

use counter::runtime::Instruction as CounterInst;
use counter::Counter;
```

## Defining `#[dispatch(...)]`

Component execution lives on an impl block annotated with `#[dispatch(...)]`.

```rust
# use eyre::Result;
# use vihaco::{component, dispatch, Effects, Message};
# component! {
#     #[derive(Debug, Default)]
#     pub component Counter { value: i64, }
#     instruction { Add(i64), Print, }
# }
# use counter::runtime::Instruction as CounterInst;
# #[derive(Debug, Clone, Message)]
# pub struct PrintPrefix(pub String);
# #[derive(Debug, Clone, PartialEq, Eq)]
# pub struct StdoutEffect(pub String);
#[dispatch(instruction = counter::runtime::Instruction, message = PrintPrefix, effect = StdoutEffect)]
impl counter::Counter {
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
use vihaco::{component, dispatch, Effects};

component! {
    #[derive(Debug, Default)]
    pub component Lamp {
        on: bool,
    }

    instruction {
        On,
        Off,
    }
}

use lamp::runtime::Instruction as LampInst;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LampChanged(pub bool);

#[dispatch(instruction = lamp::runtime::Instruction, message = (), effect = LampChanged)]
impl lamp::Lamp {
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

## Component Instruction Types

`component!` defines the component and records the two instruction types that make up a component's
public instruction surface. It implements `vihaco::HasInstructionSet` with:

- `Runtime = component_module::runtime::Instruction`
- `Syntax = component_module::syntax::Instruction`

For example, a component named `Counter` exposes
`counter::runtime::Instruction` and `counter::syntax::Instruction`. The first
is used by execution and bytecode-facing composition; the second implements
`Parse` and `SurfaceInstruction` for source text.

The generated component syntax uses the component's snake_case name as its dialect head; its parser accepts instruction
mnemonics such as `counter.add`. When the component is placed in a
`#[composite]`, the composite adds the device field name (or a configured
alias) as an outer instruction head, producing source such as `counter_a::counter.add`.

The `component!` macro also marks the generated type as a `Component`, which
identifies an executable device. The attribute form, `#[dispatch(...)]`, is intentionally separate. It wires
an execution implementation onto the component's generated runtime instruction
type; `component!` supplies the `HasInstructionSet` and `Component` implementations.

## Design Guidance

- Put bytecode-visible execution variants in the instruction enum.
- Put resolved execution input in the message type.
- Put follow-up outputs in plain effect types.
- Keep the component responsible for its own state mutation.
- Use `effect = StepOutcome` when a component needs to return control-flow signals.

## Returning A Custom Effect

By default, `execute(...)` returns `Result<Effects<()>>`. When a component needs to return a real effect, use the `effect` parameter:

```rust
use vihaco::{component, dispatch, Effects, Message};
use vihaco_cpu::StepOutcome;

component! {
    pub component CpuCore {}
    instruction {
        Nop,
        Halt,
    }
}

use cpu_core::runtime::Instruction as CpuInst;

#[derive(Debug, Clone, Message)]
pub struct CpuMsg;

#[dispatch(instruction = cpu_core::runtime::Instruction, message = CpuMsg, effect = StepOutcome)]
impl cpu_core::CpuCore {
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
- Transitional `#[composite]` wiring generates component instruction types; continuing returned effects to observers is something the hand-written runtime does (see [Defining A Composite With `vihaco`](/guide/composites)), and it does not interpret `StepOutcome` for you.

As a rule: use plain effect types for observer-delivered outputs, and use runtime-local sum-effect enums when a hand-written runtime needs extra per-step interpretation.

## What Comes Next

Once you have one or more components, the next step is to understand how observer types consume the returned effects.

Continue with [Observing Effects With `#[observe]`](/guide/observers).
