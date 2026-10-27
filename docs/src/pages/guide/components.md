---
layout: ../../layouts/Guide.astro
title: Writing components
slug: components
description: "Implement each operation using explicit messages, effects, errors, and device state."
---

# Writing components

A component is where a device keeps its state and does its work. You give it
instructions and runtime input, and it returns a result. The surrounding
composite takes care of supplying that input and deciding what to do next.

## Declare support and implement execution

This counter adds an encoded increment to its own value:

```rust
// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{Execute, NoMessage, component, dialect};

#[dialect]
pub mod counter {
    #[instruction]
    pub struct Add(pub u64);
}

#[component]
#[instructions { counter }]
#[derive(Default)]
pub struct Counter(u64);

impl Execute<counter::Add> for Counter {
    type Message = NoMessage;
    type Effect = u64;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &counter::Add, _: NoMessage) -> eyre::Result<u64> {
        self.0 = self
            .0
            .checked_add(instruction.0)
            .ok_or_else(|| eyre::eyre!("counter overflow"))?;
        Ok(self.0)
    }
}

let mut counter = Counter::default();
assert_eq!(counter.execute(&counter::Add(5), NoMessage::new())?, 5);
# Ok::<(), eyre::Report>(())
```

`#[component]` keeps your ordinary Rust struct and marks it as a component.
`#[instructions { counter }]` says it supports the whole `counter` dialect.
For each instruction, you choose the associated types in `Execute<I>`:

| Associated type | What you choose |
|---|---|
| `Message` | The runtime input the caller must supply |
| `Effect` | The result of successful execution |
| `Error` | The failure type, usually `eyre::Report` |

Execution borrows the instruction and takes the message by value. This counter
uses `NoMessage` because the instruction already contains everything it needs.
Construct it with `NoMessage::new()`; it is a different type from `()`.

## Choose the component boundary

Keep the device's state and operation behavior in the component. Let the
composite decide where operands come from and where results go. Then you can
call the component directly in a test, or put it in a different composite,
without changing its execution code.

The `Calculator` in [your first machine](/guide/first-machine) receives
`PopFromStack<(u64, u64)>`. Despite the wrapper's name, the calculator gets owned
numbers with no stack borrow. Its `PushToStack<(u64,)>` result works the same
way: the caller chooses where and when to apply it.

## Effects are instruction-specific

An instruction's `Execute<I>::Effect` can be `()`, a scalar, a domain record,
a stack result, or `Effects<E>` for several outputs. You don't have to wrap
every result in `Effects<E>`. The method returns
`Result<Self::Effect, Self::Error>`, and each instruction can choose different
message and effect types.

Return an error when the operation can't fulfill its contract. Any changes made
before the error stay in place. If an instruction promises to leave state alone
on failure, check its inputs before changing state and test that promise.

## Reuse a vocabulary across devices

You can write several components for the same dialect. A generic wrapper can
also forward `Execute<I>` to an inner implementation, as long as its bounds
establish the required message, effect, and error types.

Vihaco checks dialect coverage under the component declaration's bounds. Those
bounds must therefore support every instruction the component advertises.

Next, [prepare the messages](/guide/messages) your component will receive.
