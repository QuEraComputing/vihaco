# Design Tradeoffs, Observability, and Errors

This document records the alternatives behind the selected architecture and the resulting
diagnostic and observability boundaries.

## Comparison of Alternatives

The selected component-bound model sits between two simpler designs. Comparing ownership rather
than syntax makes the tradeoff clear.

### Component-Wide Instruction Enum

In the component-wide model, state ownership, instruction availability, and dispatch all move
together:

```text
Component owns:
    state + whole instruction enum + whole dispatch

Composite owns:
    collection of components
```

Its strengths are:

- Simple implementation.
- One match performs component dispatch.
- Straightforward single-enum routing.
- Familiar Rust enum ergonomics.

Its costs appear at composition boundaries:

- Including a component includes every instruction it supports.
- Unsupported instruction/message combinations may be representable.
- Effects and messages are often coarse enums.
- Component instruction sets are difficult to reuse selectively.
- The machine runtime instruction set is determined accidentally by struct membership.

### Pure Staged Instructions

The pure staged model moves all machine state access out of instructions:

```text
Instruction:
    Message -> Result

Machine:
    owns all state resolution and effects
```

Its strengths are:

- Maximum semantic reuse.
- Very easy unit testing.
- Explicit dataflow.
- Strong separation from runtime architecture.
- Excellent observability and simulation potential.

Its costs appear in stateful machines:

- Components risk becoming passive storage.
- Local invariant-preserving operations need excessive wiring.
- Simple mutations may require artificial effects.
- The composite carries substantial orchestration code.
- Stateful operations can be awkward or inefficient.

### Selected Component-Bound Instruction

The selected component-bound model keeps local state transitions with their owner while making
machine admission and cross-component dataflow explicit:

```text
Component implements Execute<Instruction>
Composite selects and routes Instruction
```

Its strengths are:

- Selective machine instruction sets.
- Component invariant ownership.
- Typed per-operation messages, effects, and faults.
- Efficient owner-local mutation.
- Explicit cross-component wiring.
- Machine-specific surface parsing and runtime routing.

Its costs are:

- Component-bound operations are less portable than pure operations.
- Direct mutations are not automatically visible as effects.
- Duplicate routes require generated route identities.
- Macro and diagnostic complexity increases.
- Cross-component instructions require explicit message/effect staging.
- Tests for stateful instructions need component fixtures.

This is the default because it places each responsibility at the narrowest stable ownership
boundary. Pure operations remain available through stateless executors, and cross-component
operations deliberately use message and effect staging.

## Observability and Debugging

Direct component mutation means not every state change naturally appears in the effect stream. The
architecture does not force artificial command effects solely for observability. The composite
provides step-level hooks, and the driver provides orchestration-level hooks, for:

- Instruction start and completion.
- Selected route identity.
- Component target.
- Resolved message metadata without exposing sensitive values.
- Emitted effects.
- Execution outcome and faults.
- Modeled start and completion time.

A component may emit fact events after direct mutation when those events are part of its public
model. Step tracing records route execution; driver tracing records instruction selection,
program-counter changes, parking, wakeups, and modeled time. These hooks remain separate from
semantic effects so enabling diagnostics does not change execution.

## Error Model

Failures retain the stage and route in which they occurred. Each `Execute<I>` implementation has a
typed component fault, and the composite converts it into the machine error:

```rust
MachineFault: From<<Target as Execute<I>>::Fault>
```

Pattern parsing, module resolution, runtime message resolution, effect handling, and driver
orchestration may also fail. Their diagnostic context identifies:

- The source instruction and location for parse or module-resolution failures.
- The unresolved label or symbol and the relevant module/function when resolution fails.
- The machine instruction variant.
- The route.
- The target component field.
- The current program position when available.
- The failed stage: parse, module resolve, message resolve, execute, handle, or schedule.

Conversions preserve the original source chain so machine-level context does not erase the
component or parser failure.
