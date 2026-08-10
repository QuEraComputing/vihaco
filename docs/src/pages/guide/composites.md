---
layout: ../../layouts/Guide.astro
title: Defining a Composite
slug: composites
description: "Compose components with composite!, select runtime routes, resolve messages, and deliver effects."
---

# Defining a Composite With `vihaco`

A composite is the machine-specific composition root. It owns component
instances and declares the routes that connect a public machine instruction to
a component product, a message source, observers, and one effect handler.

## A routed composite

```rust
use eyre::Result;
use vihaco::{
    composite, Absorb, Effects, Execute, Execution, Message, Observe, StepResult,
    Supply,
};

struct Stack(Vec<i64>);
impl Supply<(i64, i64)> for Stack {
    type Fault = eyre::Report;
    fn supply(&mut self) -> Result<(i64, i64), Self::Fault> {
        let rhs = self.0.pop().ok_or_else(|| eyre::eyre!("underflow"))?;
        let lhs = self.0.pop().ok_or_else(|| eyre::eyre!("underflow"))?;
        Ok((lhs, rhs))
    }
}

#[derive(Clone)]
struct Add;
struct Arithmetic;
struct Value(i64);
impl Execute<Add> for Arithmetic {
    type Message = (i64, i64);
    type Effect = Value;
    type Fault = eyre::Report;
    fn execute(&mut self, _: &Add, (lhs, rhs): (i64, i64)) -> Result<StepResult<Value>> {
        Ok(StepResult {
            effects: Effects::one(Value(lhs + rhs)),
            execution: Execution::Complete,
        })
    }
}
impl Absorb<Value> for Stack {
    type Fault = eyre::Report;
    fn absorb(&mut self, value: Value) -> Result<()> { self.0.push(value.0); Ok(()) }
}
struct Trace;
impl<R> Observe<Value, R> for Trace {
    type Effect = ();
    type Error = eyre::Report;
    fn observe(&mut self, _: &Value) -> Result<Effects<()>> { Ok(Effects::none()) }
}

composite! {
    composite Calculator {
        error = eyre::Report;

        #[device(0x01, alias = "alu")]
        arithmetic: Arithmetic,
        stack: Stack,
        trace: Trace,
    }

    runtime {
        Add(Add) => arithmetic {
            message from stack;
            effects {
                observe trace;
                absorb with stack;
            }
        }
    }
}
```

The generated public `CalculatorInstruction::Add(Add)` is the machine-local
runtime sum. `execute_generated` resolves the message, calls
`Execute<Add>`, invokes observers in declaration order, and passes each effect
to exactly one handler. `absorb with stack` delegates to `Stack::absorb`; use
`handle with method` when routing policy belongs to the composite.

## Route clauses

Every route names a payload and target:

```text
Variant(Payload) => field {
    message none;
    effects {
        observe observer_a, observer_b;
        handle with composite_method;
    }
}
```

Message sources are deliberately explicit:

- `message none` passes `NoMessage`.
- `message from field` calls `Supply<M>` on that field.
- `message with method` calls a composite method with the instruction payload.

Effect handlers are exclusive:

- `absorb with field` calls `Absorb<E>` on a component field.
- `handle with method` calls a composite method with owned `E`.

The declared `error = E` type is the normalization boundary for component,
message, observer, and handler failures.

## Devices and loading

`#[device(code, alias = "name")]` contributes device metadata and source-symbol
aliases. Codes must be unique. `#[loadable]` marks a device that receives a
direct child bytecode/SST section through the generated loader. A composite
that owns program data implements `LoadOwnBytecodeSection` or
`LoadOwnSstSection` in ordinary Rust.

The composite macro can also declare structural composites with no
`runtime` block. Those composites still provide fields, device
metadata, and section wiring, while their event loop or parent dispatch remains
hand-written.

## Runtime boundaries

The macro does not fetch instructions, own a program counter, generate a clock,
or generate continuation/resume dispatch. A runtime root can call
`execute_generated`, inspect `Execution`, update its own program state, and
schedule the next owned event. The demo shows this pattern with a CPU child and
a global event loop.

Those conveniences are planned for a later API extension. Documentation and
examples that need timing or parked operations should continue to show the
explicit parent-owned loop until that extension is implemented.

See [Building Components](/guide/components), [Using Messages](/guide/messages),
and [Observing Effects](/guide/observers) for the individual contracts.
