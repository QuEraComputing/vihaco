---
layout: ../../layouts/Guide.astro
title: Writing composites
slug: composites
description: "Declare devices and routes, then supply the policy that connects their execution."
---

# Writing composites

Once you have devices, a composite brings them together. Its declaration tells
vihaco where to send instructions. The steps you write decide how the devices
work with the rest of the machine.

## Declare the devices

```rust
use vihaco::composite;

#[composite]
struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    completed_runs: usize,
}
```

Give each device an explicit `u8` code that's unique within its parent.
Ordinary fields such as `completed_runs` hold application state and add no
instructions. You can mark one device `#[program]` to select invocation-state
access and connect operand storage.

To give a device another accepted source spelling, use an alias such as
`#[device(1, alias = "alu")]`. Field routes still use the Rust field name.
The device's code, name, and aliases each have their own job.

## Route to direct or nested devices

`field!(Machine::execution)` names a direct route as a type. You can keep going
through a nested composite with a route such as
`field!(Machine::controller::calculator)`. Generated `Field<F>` implementations
tell you the final child type and let you borrow it with `get` or `get_mut`.

Two fields can hold the same component type. They are separate destinations,
and you can give them different step behavior. Nested composites contribute
their devices' full declared capabilities to the enclosing instruction set.

## Generated instruction sets and dispatch

You no longer need to manually define an instruction enum for the whole machine.
`#[composite]` lifts the subdevices' declared instructions into the parent and
flattens them into a combined instruction set, including routes through nested
composites. Each instruction retains its destination route, so two devices that
support the same dialect remain distinct destinations.

Use `<Machine as HasInstructionSet>::Runtime` for the combined executable
instruction type, where `Machine` is your composite type, and
`<Machine as HasInstructionSet>::Syntax` for its source form:

```rust
use vihaco::{HasInstructionSet, composite};

#[composite]
struct Machine {
    #[device(0)]
    execution: vihaco_stdlib::ExecutionState,
}

type Instruction = <Machine as HasInstructionSet>::Runtime;
type SourceInstruction = <Machine as HasInstructionSet>::Syntax;
```

Use those public associated types in your code so you don't have to rely on
generated enum names or variants.

In source, routes look like `field::nested_field::dialect::mnemonic`. Aliases
add accepted spellings for a device without creating new instruction identities.
A component's separate executable association doesn't narrow the capabilities
it contributes to its parent.

For every included route and instruction, supply a `Step<F, I, Context>`.
Once those steps are available, `#[composite]` provides `Dispatch<Context>`.
Vihaco checks coverage when you use dispatch; declaring the struct alone doesn't
check that you've supplied all its execution behavior.

## The enclosing composite owns the policy

When an outer composite dispatches to a nested device, it selects the outer
composite's `Step`. If you want to reuse the inner composite's step behavior,
delegate to it explicitly. Nesting the structs won't do that automatically.

You also choose message preparation and effect handling, and construct images
outside the dispatchable composite. A [Loadable](/guide/loadables) pairs an image
with its machine.

There are a few naming details to keep in mind as you split code into modules.
Declaration discovery needs named child types. Renaming a child import or type
alias across a module boundary requires matching reexports of its generated
inventory and field companion namespaces. Bare generic parameters and associated
child types can't supply an inferred inventory. Direct named imports are a good
starting point.

For standard device fields, use the defining crate, such as
`vihaco_stdlib::ExecutionState` or `vihaco_cpu::CPU`. A facade may reexport the
type without its companion namespaces. You can still use the facade's type name
in ordinary bounds and method calls.

Next, follow [one instruction through execution](/guide/execution).
