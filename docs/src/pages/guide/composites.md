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
direct child SST section through the generated loader. A composite that owns
program data implements `LoadOwnSstSection` in ordinary Rust.

The composite macro can also declare structural composites with no
`runtime` block. Those composites still provide fields, device
metadata, and section wiring, while their event loop or parent dispatch remains
hand-written.

## Surface syntax and program loading

An executable composite can own the source grammar for its machine program.
The `syntax` block declares surface instructions, while the `runtime` block
declares the runtime routes they lower to:

```rust ignore
composite Machine {
    error = eyre::Report;

    #[device(0x01)]
    cpu: Cpu,

    #[program]
    program: ProgramImage<MachineInstruction, MachineContext, Value, ()>,
}

syntax {
    #[pattern = "'machine::halt"]
    Halt => runtime Halt;
    #[pattern = "'machine::load $0"]
    Load(u64) => lower_load;
}

runtime {
    Halt(Halt) => cpu { message none; }
    LoadConstant(u64) => cpu { message none; }
}
```

Direct `runtime` mappings are intended for unit surface instructions. A named
lowerer handles payloads and may expand one surface instruction into several
runtime instructions:

```rust ignore
impl machine::syntax::Resolver for Machine {
    fn lower_load(
        &mut self,
        value: u64,
    ) -> Result<Vec<machine::runtime::Instruction>> {
        Ok(vec![machine::runtime::Instruction::LoadConstant(value)])
    }
}
```

The generated parser is available as
`machine::syntax::Instruction::parser()`. A parsed module can be resolved and
installed with an explicit context:

```rust ignore
let parsed = machine::syntax::ParsedModule::parse_section(section)?;
machine.load_parsed(parsed, ContextHandle::new(MachineContext))?;
```

For an SST section, provide the surface-type and header types explicitly:

```rust ignore
machine.load_source::<SurfaceType, Header>(section)?;
```

`load_parsed` constructs a fresh module, lowers every function, records
function metadata, selects `main`, installs the module and context, and resets
the program counter. Malformed input or a lowering failure returns an error.

## Custom program containers

`ProgramImage` is the standard program container. A composite author only
needs to mark its program field with `#[program]`. A library author who needs
custom storage or metadata can implement `BuildProgramModule` and
`InstallProgramModule` for another container. The builder controls module
creation, instruction appending, string interning, function metadata,
constants, and final validation; generated `load_parsed` uses those operations
without depending on `LocalModule` directly.

This keeps source resolution independent from the representation used by a
particular host VM.

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
