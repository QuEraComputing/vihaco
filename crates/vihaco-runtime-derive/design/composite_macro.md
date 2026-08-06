# `composite!` Macro Design

## Status

Phase-one implementation plan. This document records the agreed runtime-only scope for the
author-facing `composite!` macro. Surface parsing, module resolution, bytecode, and scheduling
remain later work.

## Purpose

`composite!` declares a composite's fields and the runtime instruction routes that the composite
supports. It generates the machine-local runtime instruction sum and the repetitive dispatch that
connects messages, components, effects, observers, and handlers.

The composite owns route selection and cross-component policy. Reusable components own their local
invariants and implement execution or reusable capabilities such as `Supply` and `Absorb`.

The first implementation targets the runtime model demonstrated by the two-CPU example. The
example's root event loop, timing policy, program-counter bookkeeping, and resume flow remain
ordinary author-written Rust.

## Phase-one goals

The macro should:

- declare the composite struct;
- declare an explicit composite error type in the macro input;
- preserve existing `#[device(...)]` and `#[loadable]` metadata and validation;
- generate a public `<Composite>Instruction` enum for executable composites;
- generate private route marker types and route-specific trait implementations;
- resolve messages using `none`, `from`, or a composite-owned resolver method;
- execute selected component instructions through `Execute<I>`;
- observe effects in declaration order;
- consume each effect through exactly one handler;
- support reusable `Absorb<E>` delegation and composite-owned custom handlers;
- normalize component, observer, and handler errors into the composite error; and
- support structural composites that omit `runtime_instructions` entirely.

The generated execution boundary is an inherent method:

```rust
fn execute_generated(
    &mut self,
    instruction: &Self::Instruction,
) -> Result<Execution, Self::Error>;
```

It is private by default. The generated instruction enum is public so runtime roots and facade
resolution code can construct it.

## Explicit non-goals

Phase one does not:

- generate surface instruction parsers;
- generate or implement `Resolve` for parsed modules;
- generate bytecode codecs;
- fetch instructions or own a program counter;
- generate resume or continuation dispatch;
- generate timing or scheduling policy;
- define a universal public machine-execution trait; or
- add a `ResolveMessage` trait.

Message resolution is deliberately composite-owned. `Supply<M>` is a reusable component
capability, while a resolver that reads several composite fields or applies machine-specific
ordering belongs in a named method on the composite.

## Runtime foundation

`GeneratedComponent` is removed. The runtime API becomes the per-instruction model used by the
demo:

```rust
pub trait Execute<I> {
    type Message;
    type Effect;
    type Fault;

    fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault>;
}

pub struct StepResult<E> {
    pub effects: Effects<E>,
    pub execution: Execution,
}

pub enum Execution {
    Complete,
    Parked,
}

pub struct NoMessage;

pub trait Supply<M> {
    type Fault;

    fn supply(&mut self) -> Result<M, Self::Fault>;
}

pub trait Absorb<E> {
    type Fault;

    fn absorb(&mut self, effect: E) -> Result<(), Self::Fault>;
}

pub trait Observe<E, R> {
    type Error;

    fn observe(&mut self, effect: &E) -> Result<(), Self::Error>;
}

pub trait Handle<E, R> {
    type Error;

    fn handle(&mut self, effect: E) -> Result<(), Self::Error>;
}
```

The exact module placement and visibility of these traits belongs to the runtime crate, but the
macro must generate paths that work through both the `vihaco` facade and `vihaco-runtime`.

## Author-facing syntax

The initial declaration shape is:

```rust
composite! {
    composite Cpu {
        error = CpuFault;

        pub operand_stack: Stack,
        pub alu: ArithmeticUnit,
        pub channel: ChannelEndpoint<i64, SharedTransport<i64>>,
        pub debug: DebugTrace,
        pub program: Vec<CpuInstruction>,
        pub pc: usize,
    }

    runtime_instructions {
        IntegerAdd(Add) => alu {
            message from operand_stack;
            effects {
                observe debug;
                absorb with operand_stack;
            }
        }

        Recv(Recv) => channel {
            message none;
            effects {
                observe debug;
                handle with handle_receive;
            }
        }
    }
}
```

The `runtime_instructions` block is optional. If it is omitted, the composite takes no
instructions: the macro generates the struct and metadata/section wiring, but no instruction enum
and no `execute_generated` method.

### Composite declaration

The macro owns the struct declaration. Fields preserve user visibility and ordinary Rust types,
generics, and `where` clauses where supported by the parser and code generator. `error = E` is
required for executable composites and names the error type at the generated dispatch boundary.

Existing field metadata remains supported:

```rust
#[device(0x01, alias = "cpu")]
cpu: Cpu,

#[device(0x02)]
fpga: Fpga,
```

`#[loadable]` continues to identify device fields that participate in generated bytecode/SST
section loading. `#[program]` may remain accepted as a marker for later program plumbing, but has
no phase-one execution semantics.

### Runtime route declaration

Each route has the form:

```text
VariantName(PayloadType) => target_field { ... }
```

The variant name is explicit and becomes both the public instruction-enum variant and the basis of
the private route marker name. Explicit names are required because the same runtime payload may be
selected by multiple routes.

The payload type is passed unchanged to `Execute<PayloadType>`. The macro does not create a
component-wide instruction enum or insert implicit conversions.

Each route requires exactly one message clause and one effect handler. It may list zero or more
observers:

```text
message none;
message from field;
message with resolver_method;

effects {
    observe observer_a, observer_b;
    absorb with destination_field;
}
```

The handler alternatives are exclusive:

```text
absorb with field;
handle with composite_method;
```

The old `to` spelling is not part of the design and should be rejected as a normal macro syntax
error.

## Generated route behavior

For every route, the macro generates a private route marker, route-aware trait implementations,
and a dispatch arm. Conceptually, a route such as:

```rust
IntegerAdd(Add) => alu {
    message from operand_stack;
    effects {
        observe debug;
        absorb with operand_stack;
    }
}
```

generates behavior equivalent to:

```rust
let message = Supply::<BinaryOperands>::supply(&mut self.operand_stack)
    .map_err(Into::<CpuFault>::into)?;
let result = self.alu.execute(instruction, message)
    .map_err(Into::<CpuFault>::into)?;

for effect in result.effects {
    Observe::<ValueResult, routes::IntegerAdd>::observe(
        &mut self.debug,
        &effect,
    )
    .map_err(Into::<CpuFault>::into)?;

    Handle::<ValueResult, routes::IntegerAdd>::handle(self, effect)
        .map_err(Into::<CpuFault>::into)?;
}

Ok(result.execution)
```

The generated `Handle<E, Route>` implementation on the composite forwards an `absorb with field`
route to the target field's `Absorb<E>` implementation:

```rust
fn handle(&mut self, effect: E) -> Result<(), FieldFault> {
    self.destination_field.absorb(effect)
}
```

For `handle with method`, the generated wrapper calls a method implemented by the composite:

```rust
impl Cpu {
    fn handle_receive(
        &mut self,
        effect: ReceiveEffect<i64>,
    ) -> Result<(), ReceiveFault> {
        // Composite-owned routing and policy.
        Ok(())
    }
}
```

The method receives only the owned effect. It does not receive the route marker or instruction;
those are dispatch details. The macro converts its error into the declared composite error.

Observers are named fields and are called in declaration order. They borrow the effect and never
consume or clone it. The single handler receives ownership exactly once. An empty observer list is
valid.

## Message resolution

### `message none`

The generated arm passes `NoMessage` and does not call a resolver.

### `message from field`

The generated arm forwards to the named component capability:

```rust
let message = Supply::<Message>::supply(&mut self.field)?;
```

The message is owned before component execution returns, so a parked operation does not retain a
borrow into the composite.

### `message with method`

The named method is implemented on the composite and receives the instruction payload:

```rust
impl Cpu {
    fn resolve_add_message(
        &mut self,
        instruction: &Add,
    ) -> Result<BinaryOperands, CpuFault> {
        // Read and combine composite state.
        todo!()
    }
}
```

The macro calls the method uniformly even when the method does not need the instruction. This
keeps route-specific resolution explicit without introducing a route-parameterized
`ResolveMessage` trait.

## Structural composites

A structural composite may contain clocks, fabrics, devices, or other runtime state but omit
`runtime_instructions`:

```rust
composite! {
    composite HeterogeneousMachine {
        error = CpuFault;

        clock: GlobalClock<MachineEvent>,
        transport: SharedTransport<i64>,

        #[device(0x01, alias = "cpu_a")]
        cpu_a: Cpu,
        #[device(0x02, alias = "cpu_b")]
        cpu_b: Cpu,
    }
}
```

This generates the composite declaration, device metadata, and existing section wiring only. The
root event loop, event enum, child selection, timing ratios, resume handling, and deadlock policy
remain hand-written by the author.

## Validation and diagnostics

The parser/code generator should reject at macro expansion time:

- non-struct or malformed composite declarations;
- duplicate public route variant names;
- duplicate device codes, source symbols, aliases, or loadable section names;
- invalid loadable names and loadable fields without devices;
- routes with missing or duplicate message clauses;
- routes with missing or multiple effect handlers;
- duplicate observer fields;
- unknown composite fields, observers, targets, or handler methods where statically detectable;
- unsupported `to` syntax; and
- invalid route or generated identifier names.

Trait and conversion requirements that depend on resolved Rust types should be expressed through
normal compiler errors with useful generated spans where possible. In particular, compilation must
type-check:

- `Execute<PayloadType>` on the selected target field;
- `Supply<Message>` for `message from`;
- `Observe<Effect, Route>` for every observer;
- `Absorb<Effect>` for `absorb with`;
- the composite method named by `message with` or `handle with`; and
- `Into<CompositeError>` for component, resolver, observer, and handler failures.

Generated route markers, route implementations, and handler wrappers remain private. Authors use
the public instruction enum and the generated execution boundary, not generated route internals.

## Implementation sequence

1. Add the runtime contracts and migrate existing runtime tests away from `GeneratedComponent`.
2. Remove `GeneratedComponent` and update facade/runtime re-exports and examples.
3. Define a `syn` input model for the composite declaration, field metadata, route clauses,
   message clauses, observers, and the two handler forms.
4. Reuse the existing device/source-symbol/loadable validation in
   `attr_composite.rs` where applicable.
5. Generate the composite struct and public instruction enum for executable composites.
6. Generate private route markers and route-specific `Observe`/`Handle` wrappers.
7. Generate message resolution and `execute_generated` match arms.
8. Add focused success tests and trybuild diagnostics for malformed declarations and missing
   bounds.
9. Convert the demo's generated-looking `Cpu` section to `composite!` and preserve its manual
   resume/timing code.
10. Convert `HeterogeneousMachine` to a structural `composite!` declaration while preserving its
    root event loop.
11. Run formatting, clippy, workspace tests, doctests, and SPDX checks.

## Test strategy

### Macro tests

Cover at least:

- a no-message route;
- `message from` through `Supply`;
- `message with` through a composite method;
- multiple routes sharing an instruction or effect type;
- observers in declaration order;
- `absorb with` delegation;
- `handle with` composite-owned handling;
- heterogeneous route errors normalized into the composite error;
- a structural composite with no runtime instruction block; and
- generic composite and field types where supported.

### Compile-fail tests

Pin diagnostics for duplicate routes, duplicate observers, missing clauses, mutually exclusive
handlers, unsupported `to`, unknown fields, missing `Execute`/`Supply`/`Observe`/`Absorb` bounds,
and missing error conversions. Update line-sensitive `.stderr` fixtures when diagnostics change.

### Demo acceptance

The migrated demo must retain its existing behavior:

- `CpuA` parks on receive;
- `CpuB` computes and sends;
- the root schedules the wakeup at the receiver's next local boundary;
- `CpuA` resumes and computes `60`; and
- the deterministic global execution trace remains unchanged.

The macro owns CPU route dispatch only. The root continues to own event scheduling and resume
coordination.

## Review candidates

After migration, review the placeholder [machine_macro.rs] implementation and remove or replace it
once `composite!` is real. The demo's `Resume`, timing, route, message, effect, and execution
contracts are referenced by the vision and are not deletion candidates merely because some are
outside phase-one generation. Any genuinely unused concept should first be marked for review and
only deleted after a separate decision.

## Later phases

Future work may add:

- generated surface instruction sums and parsers;
- `Resolve` integration and module-wide source resolution;
- generated program/fetch/step and completion plumbing;
- generated resume and continuation routes;
- bytecode encoding and loading for generated runtime instruction sums;
- a shared public machine execution trait if multiple roots need one; and
- a `ResolveMessage` abstraction if repeated runtime-root use demonstrates that named methods are
  insufficient.
