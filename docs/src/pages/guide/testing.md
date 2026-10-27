---
layout: ../../layouts/Guide.astro
title: Testing and extending
slug: testing
description: "Check behavior at component, integration, and complete-program boundaries."
---

# Testing and extending

Start by testing behavior where you define it. You can check a component's
computation without loading any source files. A loaded-program test can then
check that the routing, metadata, and result handling all work together.

## Test each boundary

| Boundary | What to check |
|---|---|
| Component execution | Results, state changes, and instruction-specific errors |
| Message preparation | Input order, reference lookup, validation, and input consumed before an error |
| Step policy | The destination, effect application, and propagated errors |
| Program execution | Entry arguments, calls/returns, branch targets, protected locals, and results |
| Loading | Preserved metadata, checked declarations, resolved symbols, and source ownership |
| Loadable lifecycle | Reset, replacement restrictions, and retained device state |
| Binary format | Round trips, malformed input rejection, and agreement with source execution |

For a shared builder or step, check the behavior it promises rather than
recreating its body in the test. Use different operand values for operations
where order matters, so swapping them can't pass unnoticed.

## Add an instruction

Define the type in its dialect, give it a source spelling, and document its
behavior and runtime inputs/results. Each component declaring the dialect needs
an `Execute<I>` impl for it.

If syntax and runtime operands differ, add lowering. Then check whether your
existing builders and steps cover the instruction. Add preparation or result
handling where needed, and update any persistent codec and its compatibility
policy separately.

## Add a device or backend

Declare the device's dialect support and implement execution. Give it a unique
code in the composite, then provide steps for its routes. A backend that meets
an existing wrapper contract can reuse the composite's input and result handling.

## Add an independently loaded program

Define its composite and context, build and validate its module, and pair its
Image and machine in a Loadable owned by an outer runtime. Decide how the
application starts it and handles its output. Keep invocation reset and device
reset as separate choices.

## Keep documentation executable

From a vihaco checkout, run the guide's Rust blocks and examples with:

```sh
cargo test -p vihaco-doctests --features program-image-execution --all-targets
cargo test -p vihaco-doctests --features program-image-execution --doc
```

Make complete examples runnable. For a smaller excerpt, you can hide setup lines
with rustdoc's `# ` prefix while still checking the whole example. Save `ignore`
for deliberately illustrative fragments. Use the [API reference](/reference)
when you need an exact trait or method signature.
