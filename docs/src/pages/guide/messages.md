---
layout: ../../layouts/Guide.astro
title: Preparing messages
slug: messages
description: "Choose composite-specific message construction and delegate to upstream reusable recipes."
---

# Preparing messages

A component knows what input it needs, but it doesn't need to know where that
input comes from. A message can hold operands, values from an instruction field,
information from program metadata, or data from another device. The component's
`Execute<I>::Message` tells the caller which type to supply.

## Composite-specific construction

`BuildMessage<F, I, Context, M>` is the composite-specific construction
interface. The composite author implements it to decide how that machine builds
input for instruction `I` at field route `F`. Usually callers can leave `M` at
its default, the child's message type. `Context` defaults to `()`; the program
runner passes the borrowed module instead.

The body of `build_message` can read machine-specific state, apply application
policy, or explicitly call a message's `ReusableMessageRecipe::prepare`.
Recipes provide reusable logic; they do not automatically implement
`BuildMessage` for a composite.

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

Name `M` explicitly when separating generic builders for different message
shapes, as this example does with `Prefix`. `M` must still match the child's
execution message; it can't change the associated type. A custom builder can
also construct a message owned by another crate, even when that message has a
reusable recipe. The composite chooses whether to use it.

## Reusable recipes from message authors

`ReusableMessageRecipe<Parent, Route, I, Context>` is how upstream message
authors provide a generic way to build their messages over arbitrary machines
with specific capabilities. Implement it on the message type, with bounds on
`Parent` describing capabilities such as operand-stack access. The recipe can
use instruction data and a shared read-only context without depending on a
specific composite's fields or layout.

Machines use a reusable recipe by explicitly calling it **inside the body of
their `BuildMessage::build_message` implementation**. This keeps the decision
with the composite author. Here `upstream` represents a separate message library;
its recipe needs only `HasOperandStack<I>` and word-valued `PopStack` access:

```rust
use vihaco::{
    BuildMessage, Execute, ExecutionState, PushStack, ReusableMessageRecipe,
    component, composite, dialect, field,
};

mod upstream {
    use vihaco::{HasOperandStack, PopStack, ReusableMessageRecipe};

    pub struct Pair(pub u64, pub u64);

    impl<C, F, I, Context: ?Sized> ReusableMessageRecipe<C, F, I, Context> for Pair
    where
        C: HasOperandStack<I>,
        C::Source: PopStack<u64>,
    {
        fn prepare(machine: &mut C, _: &I, _: &Context) -> eyre::Result<Self> {
            let stack = machine.operand_stack(2);
            let rhs = stack.pop()?;
            let lhs = stack.pop()?;
            Ok(Self(lhs, rhs))
        }
    }
}

#[dialect]
mod arithmetic {
    #[instruction]
    pub struct Add;
}

#[component]
#[instructions { arithmetic }]
struct Calculator;

impl Execute<arithmetic::Add> for Calculator {
    type Message = upstream::Pair;
    type Effect = u64;
    type Error = eyre::Report;

    fn execute(&mut self, _: &arithmetic::Add, pair: upstream::Pair) -> eyre::Result<u64> {
        pair.0.checked_add(pair.1).ok_or_else(|| eyre::eyre!("addition overflow"))
    }
}

#[composite]
struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    calculator: Calculator,
}

impl<I> BuildMessage<field!(Machine::calculator), I, (), upstream::Pair> for Machine
where
    Calculator: Execute<I, Message = upstream::Pair>,
{
    fn build_message(&mut self, instruction: &I, context: &()) -> eyre::Result<upstream::Pair> {
        <upstream::Pair as ReusableMessageRecipe<
            Self, field!(Machine::calculator), I, (),
        >>::prepare(self, instruction, context)
    }
}

fn main() -> eyre::Result<()> {
    let mut machine = Machine { execution: ExecutionState::new(), calculator: Calculator };
    // Operand access requires a running invocation, as in the first-machine example.
    use vihaco::module::{FunctionInfo, LocalModule, Signature};
    let program: LocalModule<(), vihaco::Value, vihaco::Type> = LocalModule {
        code: vec![()], main_function: Some(0),
        functions: vec![FunctionInfo {
            name: 0, signature: Signature { params: vec![], ret: vec![] },
            local_count: 0, start_address: 0, end_address: 1, file: 0,
        }], ..Default::default()
    };
    machine.execution.start(&program, &[])?;
    machine.execution.push(20_u64);
    machine.execution.push(22_u64);
    let message = machine.build_message(&arithmetic::Add, &())?;
    assert_eq!(machine.calculator.execute(&arithmetic::Add, message)?, 42);
    assert!(machine.execution.stack().is_empty());
    Ok(())
}
```

`Route` lets recipes select a device or require device-specific capabilities.
`Context` may be unsized and can carry program metadata or other read-only data.
The machine remains mutable so a recipe can consume operands. A recipe can work
across many composites; it does not need a per-composite implementation from the
message's author.

## Standard recipes and migration

Vihaco supplies `ReusableMessageRecipe` implementations for `NoMessage`,
`PopFromStack` tuples and batches, and stdlib execution messages. Composite
builders explicitly delegate to them, just as in the example above. See the
[first machine](/guide/first-machine) for delegation shared across the standard
message types of a device.

A builder generic over every message type opts that whole device into recipes.
If some messages need custom construction, use separate builders with explicit
message types, as in the [message-kind pattern](/guide/patterns), instead of
overlapping a general delegating implementation with a specialized one.

When migrating from automatic preparation, add these delegating `BuildMessage`
implementations to your composite. `AutomaticMessage` and the automatic
`BuildMessage` blanket implementation have been removed, and `PrepareMessage`
is now named `ReusableMessageRecipe`. Existing custom `BuildMessage`
implementations can keep their construction logic. If you previously implemented
the hidden recipe traits, rename `PrepareMessage` and remove `AutomaticMessage`.

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
