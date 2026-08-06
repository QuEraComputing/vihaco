---
layout: ../../layouts/Guide.astro
title: Building Components
slug: components
description: "Declare reusable runtime components with component!, define instruction products, and implement Execute per instruction."
---

# Building Components With `vihaco`

A component owns state and the behavior for one or more runtime instruction
products. The component declaration and the execution implementation are two
deliberate boundaries:

- `component!` declares the state type and the instruction product types.
- `Execute<I>` implements one product `I`, with its own message, effect, and
  fault types.

This lets a component expose operations with different input and output
contracts without forcing them through one large instruction enum.

## Declare a component

```rust
use vihaco::component;

component! {
    component Counter {
        value: i64,
    }

    instruction {
        Add(i64),
        Print,
    }
}
```

The macro creates `counter::Counter` and places the products in
`counter::instruction`: `Add(i64)` and `Print`. Named and tuple products are
also supported:

```rust
use vihaco::component;

component! {
    component RegisterFile {
        values: Vec<i64>,
    }

    instruction {
        Read { slot: usize },
        Write(usize, i64),
        Reset,
    }
}
```

The declaration is a catalog of runtime products. It does not define source
syntax, assign machine-wide device codes, or choose which products a composite
exposes.

## Implement `Execute<I>`

Execution is implemented per product. `Message` is a marker for owned,
runtime-supplied input; `NoMessage` is the standard input for an instruction
that needs none. `StepResult` keeps returned effects separate from whether the
operation completed or parked.

```rust
use eyre::Result;
use vihaco::{
    component, Effects, Execute, Execution, Message, StepResult,
};

component! {
    component Counter {
        value: i64,
    }

    instruction {
        Add(i64),
        Print,
    }
}

#[derive(Debug, Clone)]
pub struct Prefix(String);
impl Message for Prefix {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line(String);

impl Execute<counter::instruction::Add> for counter::Counter {
    type Message = ();
    type Effect = ();
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &counter::instruction::Add,
        _message: (),
    ) -> Result<StepResult<()>, Self::Fault> {
        self.value += instruction.0;
        Ok(StepResult {
            effects: Effects::none(),
            execution: Execution::Complete,
        })
    }
}

impl Execute<counter::instruction::Print> for counter::Counter {
    type Message = Prefix;
    type Effect = Line;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &counter::instruction::Print,
        message: Prefix,
    ) -> Result<StepResult<Line>, Self::Fault> {
        Ok(StepResult {
            effects: Effects::one(Line(format!("{}{}", message.0, self.value))),
            execution: Execution::Complete,
        })
    }
}
```

The `Execute` contract is:

```text
Execute<I>::execute(&mut self, &I, Message)
    -> Result<StepResult<Effect>, Fault>
```

`Effects` can contain zero, one, or many values. `Execution::Complete` tells a
parent that it may advance its program counter; `Execution::Parked` tells it
to retain the operation until an owned completion is available. The runtime
does not infer timing or scheduling from this value.

## Capabilities around execution

Components can expose reusable capabilities independently of instruction
execution:

- `Supply<M>` produces an owned message, often from a stack or queue.
- `Absorb<E>` consumes an owned effect, often by updating state.
- `Observe<E, R>` borrows an effect for diagnostics, tracing, or recording.
- `Handle<E, R>` is the composite-selected route for the one consumer that
  receives ownership of an effect.

These contracts keep a reusable component independent of the composite that
contains it. The composite decides which capability is used on each route.

## Planned extensions

The current runtime leaves resume/continuation dispatch and timing policy to
ordinary Rust in the parent runtime. A future macro layer is planned to make
those boundaries more convenient; until then, examples should implement them
explicitly and should treat `Execution::Parked` as a real runtime state.

Continue with [Defining Composites](/guide/composites) and
[Using Messages](/guide/messages).
