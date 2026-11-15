---
layout: ../../layouts/Guide.astro
title: Images and Loadables
slug: loadables
description: "Own program data beside a mutable machine and manage invocation lifecycles."
---

# Images and Loadables

A Loadable keeps a program together with the machine that runs it. While the
machine changes its devices and invocation state, the program's code and
metadata stay stable.

## Module, image, and owner

There are three types involved in owning a program:

| Type | What it holds |
|---|---|
| `LocalModule<I, V, Ty, Info>` | Runtime code, functions, labels, constants, strings, entry selection, source symbols, and application metadata |
| `Verified<LocalModule<I, V, Ty, Info>>` | A module that passed load-time verification |
| `Image<I, V, Ty, Info>` | The verified module as owned program data |
| `Loadable<Image<I, V, Ty, Info>, M>` | The image paired with composite `M`, with methods to run and manage it |

First build a valid module by parsing and lowering source or decoding a binary
program. Verify it with `Verified::new(module)`, which checks every function
against the machine's typing rules and returns a `Rejection` listing every
problem if it fails. Then wrap the verified module in `Image::new(verified)` and
pair it with a machine using `Loadable::new(image, machine)`. These constructors
don't parse or verify the program for you, but `Image::new` only accepts a
`Verified` module, so nothing runs unverified by accident.

`Verified::assume_verified(module)` skips verification explicitly. Use it in
tests that exercise runtime checks with deliberately ill-typed programs, or for
modules a trusted toolchain has already verified. See
[Verification](/guide/verification).

An Image has no program counter, frames, or operands; those live in the
machine's `ExecutionState`. `Loadable::new` resets that invocation state when
it pairs the objects, keeping the device state intact.

## Execute and inspect

Use `execute(arguments)` to enter the image's selected main function, and
`resume()` to continue an invocation against the same image.

You can inspect the pair with `image()` (and the module with
`image().module()`) and `machine()`, or configure the machine through
`machine_mut()`. If you change invocation state directly, keep it
consistent with the paired image.

When you need the objects back, `into_parts()` consumes the Loadable and returns
its image and machine without resetting either. An outer runtime can own
several Loadables with different instruction sets and provide application
methods for running them.

## Reset and replace

`reset()` abandons the invocation and keeps the image and device state. A
computation device can still have its heap, for example, and a recorder can still
have its history. Reset those devices too if that's what your application needs.

To install a program you've already constructed and verified, call
`replace_image(image)`.
An active invocation blocks replacement, including after an execution error that
leaves it resumable. Call `reset()` first if you want to abandon it.

On success, replacement resets invocation state and keeps the devices. If the
call rejects the replacement, it leaves the current image and execution state
alone.

Build the replacement before trying to install it. Construction happens outside
the Loadable, so a construction error won't roll back changes your custom loader
made to its own context or devices.

## Composition and independent programs

A nested composite shares the enclosing image and invocation. An independently
loaded program has its own image and machine. To own several independent
programs, put each Loadable in an ordinary Rust field in an outer runtime, and
select their source sections when constructing them.

You choose how one loaded machine calls another. That might be a method on the
outer application or on a suitable component. Nesting sections in a file doesn't
connect execution or forward effects between them.

Next, see how [verification](/guide/verification) checks programs before they
run.
