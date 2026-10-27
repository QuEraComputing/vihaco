---
layout: ../../layouts/Guide.astro
title: Stack inputs and outputs
slug: stack
description: "Transfer typed operands without coupling components to invocation storage."
---

# Stack inputs and outputs

If your component needs values from the operand stack, you can describe their
types and let vihaco move them for you. The component gets owned values and
returns its results without borrowing invocation storage.

## Fixed input and output shapes

Use `PopFromStack<(A, B)>` for two inputs and `PushToStack<(R,)>` for one result.
The trailing comma in `(R,)` matters: it makes a one-element tuple. Tuples can
hold different types and support zero through sixteen values. For output, you
can also use an array of any length whose elements all have the same type.

For `(lhs, rhs)`, the builder pops `rhs` first, then `lhs`, and puts them in the
message in their declared order. Output goes onto the stack left to right, so
the last value ends up on top. Empty tuples consume or insert nothing, as do
empty output arrays.

```text
stack before: [20, 22]  ← top
message:       (20, 22)
result:        (42,)
stack after:  [42]      ← top
```

The composite's `#[program]` device connects this preparation to operand storage.
Remember the attribute: a field called `execution`, or even a field holding an
`ExecutionState`, doesn't make that connection on its own.

## Variable input counts

When you need a variable number of values of the same type, use
`PopFromStack<StackBatch<T>>` and implement `StackInputCount<I>` on the component.
Its `input_count` method returns either:

- `InputCount::Immediate(n)` for a count you obtain from the instruction.
- `InputCount::FromStack` to pop a count stored above the values.

The batch owns its values and keeps their original stack order. You can use a
slice, iterate over it, or keep it in the component. Mixed tuple/batch messages
and dynamic batch outputs don't have standard protocols; use a custom message
or handle the result in your step when you need those.

## Values and storage

`StackValue` connects the types your component computes with to their storage
representations. You can work with typed values without manipulating encoded
words. Boolean input treats any nonzero word as true, and Boolean output uses
`0` or `1`. Custom value types can supply their own storage conversions.

If you're extending storage itself, `PopStack`, `PushStack`, and `HasOperandStack`
are the relevant contracts. For ordinary component and composite work, choose
the input/output types and call preparation and effect application from a step.

## Failure and effect application

The builder takes inputs one at a time. If a later pop fails, earlier successful
pops stay consumed. A failure during execution doesn't restore prepared operands
either. Variable-count inputs follow this same rule.

After execution succeeds, your step calls `Apply::apply` on the destination to
insert a `PushToStack` result. Returning that wrapper alone won't insert it.
If something later fails, values you've already applied stay in place.

[Writing composites](/guide/composites) shows where you connect these devices.
