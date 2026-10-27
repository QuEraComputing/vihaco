---
layout: ../../layouts/Guide.astro
title: Preparing messages
slug: messages
description: "Prepare runtime inputs with standard builders or explicit composite implementations."
---

# Preparing messages

A component knows what input it needs, but it doesn't need to know where that
input comes from. A message can hold operands, values from an instruction field,
information from program metadata, or data from another device. The component's
`Execute<I>::Message` tells the caller which type to supply.

## Standard preparation

`BuildMessage<F, I, Context, M>` prepares that input for instruction `I` at field
route `F`. Usually you can leave `M` at its default, the child's message type.
`Context` defaults to `()`; the program runner passes the borrowed module instead.

For standard messages, vihaco already knows how to prepare the input.
`NoMessage` needs nothing, and `PopFromStack<(A, B)>` takes typed operands from
the composite's program storage. Choose the message type on the component;
you don't need to implement the internal preparation hook.

Your step can call `build_message(instruction, context)` before calling the
device. The builder takes `&mut self` because it may consume operands. Its job
ends when the message is ready: your step still needs to execute the operation
and handle the result. Implementing a builder doesn't create that step.

## Custom messages

When preparation needs something specific to your application, give it a distinct
message type. Here the builder reads a prefix from the composite:

```rust
use vihaco::{BuildMessage, Execute, component, composite, dialect, field};

#[dialect]
mod display {
    #[instruction]
    pub struct Show;
}

struct Prefix(String);

#[component]
#[instructions { display }]
struct Display;

impl Execute<display::Show> for Display {
    type Message = Prefix;
    type Effect = String;
    type Error = eyre::Report;
    fn execute(&mut self, _: &display::Show, prefix: Prefix) -> eyre::Result<String> {
        Ok(prefix.0)
    }
}

#[composite]
struct Machine {
    #[device(1)]
    display: Display,
    prefix: String,
}

impl BuildMessage<field!(Machine::display), display::Show, (), Prefix> for Machine {
    fn build_message(&mut self, _: &display::Show, _: &()) -> eyre::Result<Prefix> {
        Ok(Prefix(self.prefix.clone()))
    }
}

fn main() -> eyre::Result<()> {
    let mut machine = Machine { display: Display, prefix: "result: ".into() };
    let message = machine.build_message(&display::Show, &())?;
    assert_eq!(machine.display.execute(&display::Show, message)?, "result: ");
    Ok(())
}
```

Name `M` explicitly in a custom impl, as this example does with `Prefix`. That
keeps the impl's trait identity separate from standard preparation. `M` must
still match the child's execution message; it can't change the associated type.

## Context and validation

If your builder needs read-only information from outside the machine, such as
function metadata or strings, give it a context type. `Dispatch` passes that
context to the step, and the step can pass it to the builder. The component
receives the prepared message.

The builder is also a place to check references and input requirements that
depend on how the runtime is organized. Pay attention to when you consume input:
an error won't restore earlier changes. Add error context when it helps identify
the operation or reference that failed.

Several instructions can share preparation through the
[message-kind pattern](/guide/patterns). For input that comes straight from
operands, start with the [standard stack protocols](/guide/stack).
