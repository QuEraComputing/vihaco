---
layout: ../../layouts/Guide.astro
title: Advanced Instruction Usage
slug: instructions-advanced
description: Explicit opcode assignment, explicit instruction widths, and machine-level wrapper instructions that compose several component instruction sets.
---

# Advanced Instruction Usage

This guide covers explicit opcode assignment, explicit instruction widths, and machine-level wrapper instructions.

For the basics of defining instruction enums and how width inference works, see [Defining Instructions With `vihaco`](/guide/instructions).

## Setting An Explicit Opcode

Use `#[opcode = ...]` on a variant when you want to choose the encoded opcode value yourself instead of using the default variant-order assignment.

```rust
use vihaco::Instruction;

#[derive(Debug, Clone, Instruction)]
#[instruction(width = 16)]
pub enum BranchInst {
    #[opcode = 0x10]
    Jump(u32),
    #[opcode = 0x11]
    Select(u32, u32),
}
```

This is useful when:

- the bytecode format needs stable numeric opcode values
- you want specific opcode numbers for compatibility or tooling
- you want to leave gaps for future instructions

If you do not need that control, leaving opcodes inferred from variant order is the simpler default.

## Setting An Explicit Width

Use `#[instruction(width = ...)]` when you want the encoded record size to stay fixed even if the enum could be smaller.

A fixed-width device instruction type is a common case — for example a signal generator that takes a channel address and a `Play`:

```rust
use vihaco::Instruction;

#[derive(Debug, Clone, Instruction)]
#[instruction(width = 8)]
pub enum SignalInst {
    Poly(u32),
    Play,
}
```

This says that every encoded `SignalInst` record is `8` bytes wide.

Conceptually:

```text
SignalInst::Play
=> [opcode for Play][0][0][0][0][0][0][0]
```

```text
SignalInst::Poly(addr)
=> [opcode for Poly][encoded address bytes...][zero padding if needed]
```

This is useful when you want a stable instruction record size at a component boundary.

## When To Leave Width Inferred

Leave width inferred when:

- you want the enum width to naturally track its largest payload
- the instruction type is only used as a reusable building block inside a larger wrapper enum
- you do not need a fixed external record size

Set an explicit width when:

- the instruction format should always occupy a fixed number of bytes
- you want smaller variants padded up to a known record size
- the component already has a width contract you want to preserve as the enum evolves

## Machine-Level Wrapper Instructions

`vihaco` also supports instruction enums that wrap other instruction enums.
This is how a machine can expose several component instruction sets through one outer instruction type.

A machine that drives a CPU plus a signal generator can wrap both:

```rust
use vihaco::Instruction;
use vihaco_cpu as cpu;
# #[derive(Debug, Clone, Instruction)]
# #[instruction(width = 8)]
# pub enum SignalInst {
#     Poly(u32),
#     Play,
# }

#[derive(Debug, Clone, Instruction)]
pub enum MachineInst {
    Cpu(cpu::Instruction),
    Signal(SignalInst),
}
```

Each outer variant identifies which nested instruction family is being used.
The nested instruction then becomes the payload of that outer variant.

Conceptually:

```text
MachineInst::Signal(SignalInst::Play)
=> [opcode for outer Signal variant][encoded SignalInst][padding if needed]
```

This keeps composition straightforward:

- each component keeps its own instruction type
- the machine exposes one outer instruction type
- the wrapper enum handles outer dispatch without forcing every inner instruction type to be rewritten

> When you use the [`#[composite]`](/guide/composites) attribute, this outer wrapper enum is generated for you (as `<MachineName>Instruction`). Writing it by hand, as above, is the same shape — useful when you want full control over the wrapper.

## How Nested Widths Compose

For wrapper enums, the outer instruction width is computed from the outer enum, not by changing the inner types.

That means:

- each inner instruction keeps its own width
- a nested instruction payload contributes its full encoded width as payload
- the outer enum width is `1` opcode byte plus the largest nested payload used by any variant
- smaller nested payloads are padded inside the outer record

For example, imagine:

- `cpu::Instruction` is `16` bytes wide
- `SignalInst` is `8` bytes wide

Then the outer machine instruction width becomes `17` bytes:

- `1` byte for the outer machine opcode
- `16` bytes for the largest nested payload

So a smaller instruction such as `MachineInst::Signal(...)` still occupies the full outer width once encoded.

Conceptually:

```text
MachineInst::Signal(signal_inst)
=> [opcode for Signal][encoded signal instruction][zero padding...]
```

This is what makes nested instruction composition deterministic:

- decoding always reads one full outer instruction record
- the outer opcode decides which nested instruction type should decode the payload
- the nested type decodes only the bytes it understands

## Practical Guidance

- Use `#[opcode = ...]` when opcode numbers are part of the bytecode contract.
- Use `#[instruction(width = ...)]` when record size is part of the component contract.
- Wrap inner instruction enums in an outer machine enum instead of flattening all instructions into one giant type.
- Let the outer enum own machine-visible composition and padding behavior.

## What Comes Next

`#[derive(Instruction)]` covers bytecode and runtime semantics; source-text parsing is owned by an orthogonal `#[derive(vihaco_parser::Parse)]` on the same enum. See [Parser Integration for Component Instructions](/guide/parser) for the parser-side workflow and [Advanced Parser Customization](/guide/parser-advanced) for module-level orchestration (headers, sugar, labels).

After defining an instruction type, the next step is usually to attach it to a component impl with `#[component(...)]`.

See [Building Components With `vihaco`](/guide/components) for the execution side of that model.
