---
layout: ../../layouts/Guide.astro
title: Executing one instruction
slug: execution
description: "Follow dispatch, input preparation, device execution, and explicit effect handling."
---

# Executing one instruction

A component can execute an operation on its own. A step is where you connect
that operation to the rest of your machine: get the input ready, call the device,
and decide what happens to its result. You can read other devices along the way.

## Responsibilities of the execution traits

The trait names describe different parts of that work:

| Trait | Who supplies it | What it does |
|---|---|---|
| `Dispatch<Context>` | Composite macro | Chooses a route and calls its step |
| `Step<F, I, Context>` | You, on the composite | Connects input preparation, execution, and result handling |
| `BuildMessage<F, I, Context, M>` | Standard protocols or you | Prepares the child's required message |
| `Execute<I>` | You, on the component | Executes a borrowed instruction |
| `Apply<E>` | Effect/storage protocols or you | Applies one owned effect to a destination |

The component's associated types define its execution inputs and outputs.
`Step` and `Dispatch` return `eyre::Result<()>`, so a step can propagate or
convert errors along the way. `Apply::apply` returns `()`; write fallible result
handling directly in the step when you need it.

## Trace the addition

In [your first machine](/guide/first-machine), dispatch chooses
`field!(Machine::calculator)` and `arithmetic::Add`. The step also receives a
borrowed `Program` module as its context. It then does the following:

1. `BuildMessage` takes two operands and returns `PopFromStack<(u64, u64)>`.
2. `Execute` checks the addition and returns `PushToStack<(u64,)>`.
3. `Apply` puts the answer in `ExecutionState`.

The runner takes care of moving to the next instruction. The calculator just
needs its two numbers; it doesn't access module metadata or execution storage.

## Share a step by contract

The calculator's step is generic over `I`. Its bounds say that the device can
execute `I`, the composite can build its message, and execution state can apply
its result. You can reuse that step for any instruction meeting those bounds.

Keep separate steps when instructions need different result handling. Reducing
boilerplate isn't worth hiding that difference. Sometimes a shared effect
contract makes one step a good fit; sometimes specific steps are clearer.

You can also construct a message directly instead of calling `BuildMessage`.
A step might update another device, validate more data, or perform several
follow-up actions rather than apply a result to the stack. You still need to
write the step: a builder only prepares input, and `Execute` only runs the component.

## Context and borrowing

`Dispatch`, `Step`, and `BuildMessage` share a borrowed context. In the runner,
that's the module, held separately from the mutable machine. Instructions also
borrow the image, so execution doesn't need to clone them. `Execute` receives
the borrowed instruction and the prepared message.

For a typed tool or interactive command, you can call a chosen `Step` directly
without constructing an aggregate instruction. Make sure its requirements are
already satisfied. A direct step call won't set up an invocation or manage
instruction progression for you.

## Errors are visible where they occur

Preparing input, executing the component, or handling a result can fail.
Earlier operand consumption, device changes, and applied effects stay in place.
Propagate failures from result handling and keep useful `eyre` context.
Before retrying a step, check whether doing so would repeat work that already
succeeded.

[Handling effects](/guide/effects) covers the choices for result handling.
