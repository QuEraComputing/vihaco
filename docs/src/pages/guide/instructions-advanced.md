---
layout: ../../layouts/Guide.astro
title: Advanced Instruction Usage
slug: instructions-advanced
description: How composites assign opcodes, encoding widths, and machine-level instruction routes across component products.
---

# Advanced Instruction Usage

This guide covers the composite-owned parts of an instruction set: opcode
assignment, encoding widths, machine-local instruction variants, and routing
across component products.

For the component-side product model, see [Defining Instructions With `vihaco`](/guide/instructions).

## Setting An Explicit Opcode

Assign opcodes to the variants of the machine-local instruction representation
owned by the composite. Component products do not carry opcode attributes.

The exact opcode declaration belongs to the composite's machine encoding
configuration. It is not an attribute on a component product.

```text
MachineInstruction::Jump(Branch { target: ... })
    => machine opcode 0x10
MachineInstruction::Select(ConditionalBranch { ... })
    => machine opcode 0x11
```

This is useful when:

- the bytecode format needs stable numeric opcode values
- you want specific opcode numbers for compatibility or tooling
- you want to leave gaps for future instructions

If you do not need stable values, letting the composite assign them from route
order is the simpler default.

## Setting An Explicit Width

Set the width on the composite-owned encoding when you want the encoded record
size to stay fixed. This is a machine-format decision, not a component-product
decision.

A fixed-width device instruction type is a common case — for example a signal generator that takes a channel address and a `Play`:

```text
MachineInstruction::Poly(Poly { channel: ..., value: ... })
    => fixed-width machine record
MachineInstruction::Play(Play)
    => the same fixed-width record, with padding as required
```

This says that every encoded machine record is `8` bytes wide. The width is a
machine-format decision; the `Poly` and `Play` products remain ordinary
component structs.

This is useful when you want a stable instruction record size at a composite
boundary.

## When To Leave Width Inferred

Leave width inferred when:

- you want the enum width to naturally track its largest payload
- the composite owns the machine encoding and can assign the width there
- you do not need a fixed external record size

Set an explicit width when:

- the instruction format should always occupy a fixed number of bytes
- you want smaller variants padded up to a known record size
- the machine format has a width contract you want to preserve as routes evolve

## Machine-Level Wrapper Instructions

`composite!` builds the machine-local instruction sum from products supplied by
its component routes. This is how a machine exposes several component
instruction families through one outer instruction type.

A machine that drives a CPU plus a signal generator can wrap both:

```rust,ignore
use vihaco::composite;

composite! {
    composite Machine {
        error = eyre::Report;
        cpu: cpu::CPU,
        signal: Signal,
    }

    runtime {
        CpuBranch(cpu::runtime::instruction::Branch) => cpu {
            message none;
        }
        SignalPlay(signal::runtime::instruction::Play) => signal {
            message none;
        }
    }
}
```

Each route becomes a variant in the generated machine-local instruction enum.
The component product is the payload, and the route target identifies the
component whose `Execute<I>` implementation receives it.

Conceptually, a machine record contains the composite route discriminator
followed by the encoded product payload and any required padding.

This keeps composition straightforward:

- each component keeps independent product structs
- the composite exposes one machine-local instruction type
- the composite handles encoding, routing, and dispatch without rewriting the
  component products

> A `composite!` declaration generates the machine-local runtime sum as
> `<MachineName>Instruction`, with one explicitly declared route per product.

## How Nested Widths Compose

For composite-owned instruction sums, the machine encoding width is computed
from the machine routes, not by changing the component products.

That means:

- component products remain independent of encoding width
- each route contributes its payload width to the machine encoding
- the machine width is chosen from the largest route payload, plus any machine
  opcode/header bytes
- smaller route payloads are padded according to the machine format

For example, imagine:

- the CPU branch route has a payload width determined by its product
- the signal play route is a unit product

The machine instruction width is then determined by the composite's selected
record format and largest route payload:

- the machine opcode/route discriminator
- the largest payload required by any routed product

So a smaller product such as `Play` still occupies the full machine record
width once encoded.

This is what makes nested instruction composition deterministic:

- decoding always reads one full outer instruction record
- the composite route discriminator decides which product type receives the
  payload
- the composite's encoder/decoder defines how many bytes that payload uses

## Practical Guidance

- Use route-level opcode metadata when opcode numbers are part of the machine
  bytecode contract.
- Use composite-level width metadata when record size is part of the machine
  format.
- Add products to `composite!` routes instead of creating component-side
  instruction enums.
- Let the generated machine instruction enum own machine-visible composition,
  encoding, padding, and dispatch.

## What Comes Next

Component syntax and machine encoding are separate concerns. A composite may
lower parsed surface instructions into its generated machine instruction enum,
then route the product to `Execute<I>`. See [Pattern Parser Integration for
Component Instructions](/guide/parser) and [Module Parsing and Resolution](/guide/parser-advanced).

After defining an instruction type, implement `Execute<I>` for the relevant
component product and select it from a `composite!` route.

See [Building Components With `vihaco`](/guide/components) for the execution side of that model.
