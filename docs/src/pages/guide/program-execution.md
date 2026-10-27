---
layout: ../../layouts/Guide.astro
title: Running programs
slug: program-execution
description: "Understand invocation storage, program control, execution entry, and recovery."
---

# Running programs

With the devices and steps connected, the runner can borrow a module and send
its instructions through your composite's dispatch. The composite needs a
selected `ExecutionState` device and steps for its devices.

## Separate computation and program control

`ExecutionState` keeps the data for the current invocation: word storage,
reserved locals, call frames, a private program counter, and execution flags.
Its dialect, `vihaco_stdlib::program::program`, handles constants, local
loads/stores, stack operations, branches, calls, returns, and halt.

The standard `vihaco_cpu::CPU` computes with typed messages and returns effects.
It keeps device state such as its heap. The operand stack, frames, and PC belong
to execution state. In the [first machine](/guide/first-machine), your custom
calculator fills the computation role instead.

## Enter and continue an invocation

Call `execute(&module, &mut machine, arguments)` to enter the function selected
by `main_function`. It runs until a terminal instruction or an error. Supply
arguments as encoded words, with the count expected by the entry function.
For an owned image/machine pair, `Loadable::execute` makes this call for you.

To continue an existing invocation, use `resume(&module, &mut machine)`.
Pass the same unchanged module you used at entry. A Loadable keeps that pairing
for you; when using the free functions, you need to maintain it yourself.

You can start a halted invocation again. `ExecutionState::start` prepares its
entry frame. Calling `reset` abandons an invocation and keeps the allocated
storage for reuse. Use `stack()`, `frames()`, and `pc()` for read-only inspection.

## Frames and locals

A function's `local_count` reserves slots for its locals, including parameters.
Arguments occupy the first slots, and extra locals start at zero. Temporary
operands sit above that reserved region. Popping operands can't consume the
locals, and storing to a local can't enlarge the region.

Here's a frame with two parameters, four reserved local slots in total, and
two temporary operands. Stack indices increase downward in this diagram:

```text
Stack index         Contents                 Region
                    ┌──────────────────────┐
                    │ Earlier frames,      │
                    │ if any               │
                    ├──────────────────────┤
base + 0            │ Parameter 0 / local 0│ ┐
base + 1            │ Parameter 1 / local 1│ │ Reserved locals
base + 2            │ Local 2 (starts at 0)│ │ local_count = 4
base + 3            │ Local 3 (starts at 0)│ ┘
                    ├──────────────────────┤ ← operands_index()
base + 4            │ Operand 0            │ ┐ Temporary operands
base + 5            │ Operand 1            │ ┘ ← top of stack
                    └──────────────────────┘
                              ↓ Push adds a value at the end
```

`base` is the absolute stack index where this frame begins. Local indices are
relative to it, so local `2` lives at `base + 2`. The first operand starts at
`base + local_count`, which is what `Frame::operands_index()` returns. Pops can
remove temporary operands but stop at that boundary. With no operands, the
stack ends immediately after the reserved locals.

The frame record itself lives separately in `ExecutionState::frames()`. It
tracks `base`, `local_count`, the function index, and the return PC; those
bookkeeping fields don't occupy slots in the word stack shown here.

Calls use prepared metadata for the target, arity, and local requirements.
A return keeps the requested results and removes the callee's frame. After the
outermost function returns, you can read the results through `stack()`.

Construction validates function signatures and the supplied tables. Execution
checks its word-based operations, but it isn't a general static type checker
for arbitrary programs.

## Instruction progression

Before dispatch, the runner sets the next sequential position. Branches, calls,
and returns replace that position as needed. You don't need to increment the
PC in ordinary composite steps.

After a dispatch error or terminal instruction, inspection shows that
instruction's source position. The earlier work still happened: restoring the
position doesn't undo consumed messages, component execution, or applied effects.

## Recover according to application policy

An error can leave the invocation active. If you resume, it can attempt the
faulting instruction again. Check the remaining operands and device state before
doing that, since retrying isn't guaranteed to be safe.

Reset abandons the invocation, but doesn't undo device work. Vihaco doesn't roll
back the earlier changes for you.

[Images and Loadables](/guide/loadables) covers ownership and reset in more detail.
