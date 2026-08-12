---
layout: ../../layouts/Guide.astro
title: Defining Instructions
slug: instructions
description: "How component instruction products become a composite-owned instruction set with encoding, routing, and dispatch."
---

# Defining Instructions With `vihaco`

In the rewritten component model, a component declares individual runtime
instruction products with `component!`. Each product is a struct and each
component implements `Execute<I>` for the products it supports. Components do
not build the machine-wide instruction enum and do not own machine opcodes or
dispatch.

`composite!` assembles products from one or more components into the
machine-local instruction enum. The composite also owns route selection,
encoding/decoding, and dispatch to the selected component's `Execute<I>`
implementation.

This guide shows:

- how to declare individual component instruction products
- how to implement `Execute<I>` for those products
- how a composite assembles and routes them

For explicit opcode overrides, explicit widths, and machine-level wrapper instructions, see [Advanced Instruction Usage](/guide/instructions-advanced).

If you are new to the component model, read [Building Components With `vihaco`](/guide/components) first.

## Component Instruction Products

The component side contains only the operation's product struct:

```rust ignore
component! {
    component Counter {
        value: i64,
    }

    runtime {
        instruction {
            Add(i64),
            Print,
        }
    }
}
```

This generates `counter::runtime::instruction::Add` and
`counter::runtime::instruction::Print` as independent structs. They are the
types passed to `Execute<I>`:

```rust ignore
impl Execute<counter::runtime::instruction::Add> for counter::Counter {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &counter::runtime::instruction::Add,
        _message: NoMessage,
    ) -> Result<StepResult<NoEffect>, Self::Fault> {
        self.value += instruction.0;
        vihaco::complete!()
    }
}
```

The product itself has no opcode. A `composite!` route supplies the machine
variant and associates it with the component product:

```rust ignore
composite! {
    composite Machine {
        error = eyre::Report;
        cpu: counter::Counter,
    }

    runtime {
        Add(counter::runtime::instruction::Add) => cpu {
            message none;
        }
    }
}
```

`MachineInstruction::Add` is the machine-local encoded instruction, with
`Add` as its payload. Its opcode and width belong to the composite's encoding
contract, not to the component product.

## Practical Guidance

- Keep component products focused on operation data and execution behavior.
- Implement `Execute<I>` once for each product/component pairing.
- Put bytecode-visible variants, opcodes, widths, and dispatch in the
  `composite!` declaration or its generated machine representation.

## What Comes Next

For explicit opcode assignment, explicit widths, and machine-level wrapper instructions, see [Advanced Instruction Usage](/guide/instructions-advanced).

Source-text parsing is separate from runtime products. A component may expose
local syntax vocabulary, while the composite mounts namespaces and lowers
surface instructions to its machine-local runtime instruction enum. See
[Pattern Parser Integration for Component Instructions](/guide/parser) and
[Module Parsing and Resolution](/guide/parser-advanced).

After defining products, implement `Execute<I>` for each product as described in
[Building Components](/guide/components).

See [Building Components With `vihaco`](/guide/components) for the execution side of that model.
