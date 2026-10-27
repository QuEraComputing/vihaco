---
layout: ../../layouts/Guide.astro
title: Essential concepts
slug: essentials
description: "The minimum needed to define instructions, implement components, and wire a VM."
---

# Essential concepts

[Your first machine](/guide/first-machine) puts all the pieces together. This
page explains the three things you author and how they fit. You can reuse
existing dialects and components wherever they meet your needs.

## Define instructions: the vocabulary

A **dialect** groups operations. Declare it with `#[dialect]`, and mark each
operation's struct with `#[instruction]`. Its fields are operands stored in the
program; runtime inputs belong in a message instead.

```rust
use vihaco::dialect;

#[dialect]
pub mod counter {
    #[instruction]
    pub struct Add(pub u64);
}
```

`counter::Add(5)` describes an operation with an encoded increment of five.
The dialect defines the vocabulary; a component supplies its behavior. See
[dialects and instructions](/guide/instructions) for the full contract and
[parsing source](/guide/parser) to customize source spellings.

## Define components: state and behavior

A **component** is a Rust struct that implements the operations in its declared
dialects. Add `#[component]` and `#[instructions { counter }]`, then implement
`Execute<counter::Add>` for the operation above. Each implementation chooses:

- `Message`: input supplied when the instruction runs, such as stack operands;
  use `NoMessage` when none is needed.
- `Effect`: the result, such as a scalar, a stack output, or `()`.
- `Error`: the failure type, usually `eyre::Report`.

`execute(&mut self, instruction: &I, message)` updates component state and returns
the effect in a `Result`. The caller supplies the message and handles the effect.
This lets the same component work with different operand storage and result
handling. Start with the complete counter in [writing components](/guide/components);
then use [messages](/guide/messages), [stack inputs and outputs](/guide/stack),
and [effects](/guide/effects) as needed.

## Wire a VM: devices and execution policy

A **composite** connects components as named device fields. Mark the struct with
`#[composite]` and each device with a unique `#[device(code)]`. A device may itself
be a composite. Mark one device `#[program]` when it provides invocation state
and operand storage, as `ExecutionState` does in the first machine.

You no longer need to manually define a machine instruction enum.
`#[composite]` lifts each subdevice's instructions and flattens them into the
machine's generated instruction set, preserving their destination routes.
For your composite type `Machine`, use these public types:

```rust
use vihaco::{HasInstructionSet, composite};

#[composite]
struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
}

type Instruction = <Machine as HasInstructionSet>::Runtime;
type SourceInstruction = <Machine as HasInstructionSet>::Syntax;
```

`Runtime` is the executable instruction type; `Syntax` is its parsed source
form. Adding device fields adds their declared instructions automatically.
Source instructions use device routes followed by dialect and mnemonic, such as
`execution::program::ret 1`.

You supply `Step<F, I, Context>` for the included routes and instructions. A step
typically calls `BuildMessage`, invokes the device's `Execute<I>`, and applies
or otherwise handles its effect. The macro supplies dispatch to those steps.
See [writing composites](/guide/composites) and
[executing one instruction](/guide/execution) for the details, including generic
steps that cover multiple instructions.

To run a whole program, construct a module with code and metadata, wrap it in an
`Image`, and pair it with your machine in a `Loadable`. See
[running programs](/guide/program-execution) and [Images and Loadables](/guide/loadables).
If a compiler produces the program, use the
[SST producer reference](/guide/sst-reference) for its current output contract.

Continue with [dialects and instructions](/guide/instructions) to build your vocabulary.
