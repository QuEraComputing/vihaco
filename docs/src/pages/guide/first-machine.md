---
layout: ../../layouts/Guide.astro
title: Your first machine
slug: first-machine
description: "Build a dialect, a computation device, authored steps, and an executable image."
---

# Your first machine

Our first machine has a small job: push `20` and `22`, add them, and return `42`.
You'll write the `Calculator`; the standard `ExecutionState` will handle the
program's operands and control flow.

Use the [dependency setup](/guide). You'll need two files: the program in
`src/first-machine.sst` and the Rust machine in `src/main.rs`.

## The SST program

> **SST's future:** SST may become unsupported or be superseded by a bytecode
> format plus a disassembler. It is not a promised long-term compiler target.
> See [emitting SST from a compiler](/guide/sst-reference) for the current contract.

Save this as `src/first-machine.sst`. The `.text(root)` block contains our four
instructions. The header describes the function and its tables so the loader
can check the program before running it.

```text
sst v1
.section(root):
    .header(root):
        metadata {
            strings: ["main", "first-machine.sst"],
            functions: [
                {
                    name: 0,
                    signature: { params: [], ret: [u64] },
                    local_count: 0,
                    start_address: 0,
                    end_address: 4,
                    file: 1,
                },
            ],
            labels: [],
            constants: [],
            main_function: 0,
            source_symbols: [],
            extra: {},
        };
    .header(root).
    .text(root):
        fn @main() -> u64 {
            execution::program::const_u64 20
            execution::program::const_u64 22
            calculator::arithmetic::add
            execution::program::ret 1
        }
    .text(root).
.section(root).
```

## The complete machine

Put this in `src/main.rs`, next to the SST file. `include_str!` reads that file
at compile time, so the example doesn't depend on your working directory.

```rust
# // Resolve the companion file from the documentation test crate.

# macro_rules! include_str {
#     ("first-machine.sst") => {
#         std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/examples/first-machine.sst"))
#     };
# }
// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{
    Apply, BuildMessage, Execute, ExecutionState, HasInstructionSet, Image, Loadable, NoContext,
    PopFromStack, PushToStack, SstFile, Step, Type, Value, component, composite, dialect, field,
    loader::MetadataLoweringContext,
    module::{LocalModule, ProgramMetadata},
    syntax::{HeaderDeclarations, ParsedModule, lower_functions_with_metadata},
};

#[dialect]
pub mod arithmetic {
    #[instruction]
    pub struct Add;
}

#[component]
#[instructions { arithmetic }]
#[derive(Default)]
pub struct Calculator {
    additions: usize,
}

impl Execute<arithmetic::Add> for Calculator {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &arithmetic::Add,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let sum = lhs
            .checked_add(rhs)
            .ok_or_else(|| eyre::eyre!("addition overflow"))?;
        self.additions += 1;
        Ok(PushToStack((sum,)))
    }
}

#[composite]
pub struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    calculator: Calculator,
}

type Instruction = <Machine as HasInstructionSet>::Runtime;

type SourceInstruction = <Machine as HasInstructionSet>::Syntax;

type Program = LocalModule<Instruction, Value, Type>;

impl<I> Step<field!(Machine::execution), I, Program> for Machine
where
    ExecutionState: Execute<I, Effect = (), Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::execution), I, Program>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
        let message =
            <Self as BuildMessage<field!(Machine::execution), I, Program>>::build_message(
                self,
                instruction,
                module,
            )?;
        self.execution.execute(instruction, message)
    }
}

impl<I> Step<field!(Machine::calculator), I, Program> for Machine
where
    Calculator: Execute<I, Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::calculator), I, Program>,
    ExecutionState: Apply<<Calculator as Execute<I>>::Effect>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
        let message =
            <Self as BuildMessage<field!(Machine::calculator), I, Program>>::build_message(
                self,
                instruction,
                module,
            )?;
        let effect = self.calculator.execute(instruction, message)?;
        self.execution.apply(effect);
        Ok(())
    }
}

pub fn main() -> eyre::Result<()> {
    let source = include_str!("first-machine.sst");
    let file = SstFile::<NoContext>::from_text(source)?;
    let ParsedModule { header, functions } = ParsedModule::<
        SourceInstruction,
        Type,
        HeaderDeclarations<ProgramMetadata>,
    >::parse_section(file.root())?;

    let mut declarations = header.into_iter();
    let metadata = declarations
        .next()
        .ok_or_else(|| eyre::eyre!("missing program metadata"))?;
    eyre::ensure!(
        declarations.next().is_none(),
        "expected one metadata declaration"
    );
    let mut context = MetadataLoweringContext::new(&metadata)?;
    let code = lower_functions_with_metadata(functions, &metadata, &mut context)?;
    let module = metadata.into_module(code)?;
    let machine = Machine {
        execution: ExecutionState::new(),
        calculator: Calculator::default(),
    };
    let mut loaded = Loadable::new(Image::new(module), machine);
    loaded.execute(&[])?;
    assert_eq!(loaded.machine().execution.stack(), &[42]);
    assert_eq!(loaded.machine().calculator.additions, 1);
    Ok(())
}
```

Run `cargo run`. The assertions check both the result, `42`, and that the
calculator performed one addition.

## What you authored

The `arithmetic` dialect gives the operation a name. `Calculator` declares that
it supports the dialect and implements `Execute<arithmetic::Add>`. Its message
holds two typed numbers, and its effect holds the answer. The calculator never
needs to access the stack itself.

`Machine` brings the devices together. The `#[program]` attribute selects
`execution` as the invocation state and connects it to standard operand
preparation. The two `Step` implementations describe how each device fits in:
program instructions change execution state directly, while calculator
instructions take operands, compute, and put the answer back.

The generic bounds on those steps let you use the same body for other
instructions that meet the same requirements.

## What vihaco supplies

The macros handle instruction parsing, field routes, and dispatch through the
combined instruction set. Standard builders move operands into `PopFromStack`,
and `Apply` inserts the `PushToStack` result. The runner fetches instructions and
moves to the next one; program instructions handle calls, branches, and returns.

`SstFile` reads the file's section structure. `ParsedModule` then parses the
root section's metadata declarations and function bodies, using the machine's
public syntax instruction type. That avoids relying on generated enum variant
names.

This example expects exactly one metadata declaration. Once it has that,
`MetadataLoweringContext` prepares symbol lookup and
`lower_functions_with_metadata` checks the function declarations against the
metadata and turns syntax instructions into executable ones.
[Loading SST programs](/guide/parser-advanced) builds on this with custom
headers and multiple sections.

## Why there is metadata

The metadata tells execution where the function is and what it expects.
`main_function: 0` selects the first function-table entry. That function covers
instructions `0..4`, returns one `u64`, and has no parameters or reserved locals.
Its name and source filename refer into `strings`.
`ProgramMetadata::into_module` checks and assembles the tables and code.

Finally, `Image::new` wraps the module, and `Loadable::new` pairs it with the
machine. Calling `execute(&[])` enters the selected function with no host
arguments.

[Essential concepts](/guide/essentials) summarizes the minimum needed to define
instructions, implement components, and wire a VM before you explore each topic.
