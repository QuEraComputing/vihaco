---
layout: ../../layouts/Guide.astro
title: Using Messages
slug: messages
description: "Resolve owned execution input at a composite route and pass it to Execute."
---

# Using Messages With `vihaco`

A message is owned, runtime-supplied input for one instruction execution. It
is separate from the instruction payload so a source program can name an
operation while the composite supplies current machine state, timing data,
capabilities, or values from another component.

Message types are ordinary Rust types. Implement the marker when the type is a
meaningful runtime message:

```rust
use vihaco::Message;

#[derive(Debug)]
struct BinaryOperands { lhs: i64, rhs: i64 }
impl Message for BinaryOperands {}
```

The component declares the message through its `Execute<I>` implementation;
the composite resolves it through one of the route clauses.

## The three message sources

```text
message none;                 // passes NoMessage
message from operand_stack;   // calls Supply<M>
message with resolve_message; // calls a composite method
```

`message from field` is useful when a reusable component already knows how to
produce the message. `message with method` is the right boundary when several
fields or machine policy must be combined:

```rust ignore
impl Calculator {
    fn resolve_add(
        &mut self,
        _instruction: &calculator::instruction::Add,
    ) -> eyre::Result<BinaryOperands> {
        Ok(BinaryOperands { lhs: 1, rhs: 2 })
    }
}
```

The resolver returns an owned value. That matters for parked operations: the
component must not retain a borrow into the composite while waiting for a
completion.

## Message, instruction, and effect

- The instruction is the runtime operation selected by a composite route.
- The message is resolved input for this execution attempt.
- The effect is owned output returned in `StepResult`.

Use instruction fields for values that are part of the encoded/runtime
operation. Use messages for values supplied by the current machine state.
Use effects for state changes or events that the parent must observe or route.

`message = ...` on the old component attribute is not part of the current
API. The message contract belongs to `Execute<I>`, and its source belongs to
the composite route.

Continue with [Defining a Composite](/guide/composites).
