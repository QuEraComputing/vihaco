---
layout: ../../layouts/Guide.astro
title: The mental model
slug: mental-model
description: "Understand instruction vocabulary, devices, runtime policy, and program ownership."
---

# The mental model

When you build a vihaco machine, you choose some instructions and write devices
that know how to execute them. You can give the same instructions to different
devices, or reuse a device in machines that supply its inputs and handle its
outputs differently.

Addition is a small example. An addition instruction can have no encoded fields
at all: its component receives two numbers in a message and returns their sum as
an effect. The surrounding machine decides to take the numbers from the current
invocation's stack and put the result back there. The component just works with
the numbers.

## The building blocks

These names come up throughout the guide:

| Concept | What it does |
|---|---|
| Dialect | Defines a vocabulary of named instructions and what they mean |
| Instruction | Describes an operation and any operands encoded in the program |
| Component | Owns device state and implements instructions through `Execute<I>` |
| Message | Supplies input when an instruction runs |
| Effect | Carries a successful execution result back to the caller |
| Composite | Owns devices and connects their execution through steps you write |
| Module | Holds code and tables such as functions, strings, and labels |
| Image | Owns a module as program data |
| ExecutionState | Holds one invocation's operands, locals, frames, and control state |
| Loadable | Pairs an image with the composite that executes it |

## From source to program data

Before a program can run, its source becomes an image:

```text
source text
    → parsed syntax instructions and header declarations
    → validated metadata and symbolic lowering
    → LocalModule containing runtime instructions and metadata
    → Image
```

Parsing checks how instructions are spelled. Lowering turns source operands,
such as function names and quoted strings, into the operands execution needs.
Metadata describes the functions' signatures, reserved locals, instruction
ranges, and which function to enter. Your loader prepares all of this before
execution starts.

## From instruction to state change

Once the image is ready, the runner borrows its instructions and sends them
through the machine:

```text
runner borrows an instruction from the image
    → Dispatch selects a device route
    → authored Step prepares input, executes, and handles the result
        → BuildMessage prepares the component's message
        → Execute mutates the component and returns its effect
        → Apply or custom handling continues the effect
```

You write the steps and decide what happens to their results. A step can use
`BuildMessage` to prepare input, or construct the message itself. It might put a
result on the stack, collect a record, configure another device, or perform
another action. Vihaco doesn't generate a step or choose an effect-delivery
policy for you.

## Ownership makes the pieces fit

The image lives beside the mutable machine. That lets execution borrow code
and metadata while changing components, without cloning instructions.
`Loadable<Image<Instruction>, Machine>` is a typical way to own the pair.

Inside the machine, `ExecutionState` owns invocation storage and control flow.
A CPU component computes with the values you give it; the program counter and
operand stack live in execution state. Other components keep their own device
state too. Resetting an invocation and resetting a device are separate operations.

[Your first machine](/guide/first-machine) puts these pieces together.
