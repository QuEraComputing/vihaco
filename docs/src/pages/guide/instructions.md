---
layout: ../../layouts/Guide.astro
title: Defining Instructions
slug: instructions
description: "How to define a component-local instruction enum with #[derive(Instruction)], how opcodes are assigned, and how encoding width is chosen."
---

# Defining Instructions With `vihaco`

Instruction types are the bytecode-visible operations in `vihaco`.
They are usually Rust enums annotated with `#[derive(Instruction)]`.

This guide shows:

- how to define a component-local instruction enum
- how opcodes are assigned by default
- how instruction width is chosen

For explicit opcode overrides, explicit widths, and machine-level wrapper instructions, see [Advanced Instruction Usage](/guide/instructions-advanced).

If you are new to the component model, read [Building Components With `vihaco`](/guide/components) first.

## A Small Instruction Enum

The smallest useful instruction type is just an enum with `#[derive(Instruction)]`.

```rust
use vihaco::Instruction;

#[derive(Debug, Clone, Instruction)]
pub enum CounterInst {
    Add(i64),
    Print,
}
```

Each variant becomes an opcode in the encoded instruction stream.
Tuple fields become payload bytes that follow the opcode.

Conceptually:

```text
CounterInst::Add(5) => [opcode for Add][encoded i64 payload]
CounterInst::Print  => [opcode for Print]
```

By default, `#[derive(Instruction)]` assigns opcodes in variant order starting at `0`.
That means the first variant gets opcode `0`, the second gets `1`, and so on.

In normal component code, this instruction type is the `instruction = ...` value on the component impl:

```rust
use eyre::Result;
use vihaco::{Instruction, component};

#[derive(Debug, Clone, Instruction)]
pub enum LampInst {
    On,
    Off,
}

#[derive(Debug, Default)]
pub struct Lamp {
    on: bool,
}

#[component(instruction = LampInst, message = ())]
impl Lamp {
    fn execute(&mut self, inst: LampInst, _msg: ()) -> Result<vihaco::Effects<()>> {
        self.on = matches!(inst, LampInst::On);
        Ok(vihaco::Effects::none())
    }
}
```

## How Width Is Chosen

Every instruction type has an encoded width in bytes.
That width includes:

- one opcode byte for the enum variant itself
- enough payload space for the largest variant in the enum

If you do not set a width explicitly, `#[derive(Instruction)]` computes it from the enum shape.

As a rule:

- a unit variant contributes only its opcode byte
- a tuple variant contributes its opcode byte plus the widths of its fields
- the enum width becomes the largest of those variant widths

For example:

```rust
use vihaco::Instruction;

#[derive(Debug, Clone, Instruction)]
pub enum InnerInst {
    Ping,
    Pong,
}

#[derive(Debug, Clone, Instruction)]
pub enum OuterInst {
    Idle,
    Inner(InnerInst),
}
```

`InnerInst` has width `1`, because each variant is only an opcode.
`OuterInst` has width `2`:

- `OuterInst::Idle` needs `1` byte
- `OuterInst::Inner(...)` needs `1` outer opcode byte plus the `1` byte used by `InnerInst`

So the final width is the maximum variant width, which is `2`.

## Practical Guidance

- Put bytecode-visible operations in an `#[derive(Instruction)]` enum.
- Let opcodes default to variant order unless you need specific encoded values.
- Start with inferred width unless you already need a fixed record size.

## What Comes Next

For explicit opcode assignment, explicit widths, and machine-level wrapper instructions, see [Advanced Instruction Usage](/guide/instructions-advanced).

`#[derive(Instruction)]` covers bytecode and runtime semantics; source-text parsing is owned by an orthogonal `#[derive(vihaco_parser::Parse)]` on the same enum. See [Parser Integration for Component Instructions](/guide/parser) for the parser-side workflow and [Advanced Parser Customization](/guide/parser-advanced) for module-level orchestration (headers, sugar, labels).

After defining an instruction type, the next step is usually to attach it to a component impl with `#[component(...)]`.

See [Building Components With `vihaco`](/guide/components) for the execution side of that model.
