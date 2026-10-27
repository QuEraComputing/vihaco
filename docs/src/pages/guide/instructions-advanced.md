---
layout: ../../layouts/Guide.astro
title: Binary formats
slug: instructions-advanced
description: "Separate container framing, instruction encoding, and complete program metadata."
---

# Binary formats

> **Warning:** The bytecode container and format are currently unstable and may
> change. Don't rely on compatibility between versions.

A binary format needs to describe both the sections in a file and the complete
module you want to execute. Reading the container is the first part; you still
need to construct a runnable image from its contents.

## Container and section data

`BytecodeFile<C>::from_bytes(bytes)` reads the container and gives you a root
section. Your `C` implements `BytecodeGlobalContext` to read global data and
`SectionNameResolver` to look up child-section names stored as indices.
Use `child(name)` or `children()` to choose independently loaded units.

Section views expose header bytes, instruction bytes, and child sections.
Read your header representation with `decode_header::<H>()`. If `I` implements
the required bytecode traits, `decode_instructions::<I>()` can decode its
instruction stream.

You'll need to supply any instruction codecs those helpers require. Typed
composition and persistent encoding are separate: a composite declaration
doesn't give you a stable hierarchical wire format.

## Define instruction identities deliberately

The ABI's `Instruction` derive and bytecode traits let you define encoded types.
Your wire types can differ from the types you execute. Choose how to represent
device routes, opcodes, operand widths, and payloads, then decode them into the
module's runtime instruction type.

Generated inventory positions and hidden enum variant order aren't persistent
compatibility contracts. If your codec needs access to a particular
representation, keep that dependency in the codec, away from component execution
and step behavior.

## Preserve the whole program

Code is only part of what a program needs. Keep its function signatures and
ranges, reserved locals, entry selection, labels, constants, strings, source
symbols, and application configuration too. Check references and table bounds
before pairing the decoded result with a machine. An instruction round trip
alone doesn't show that the decoded program will behave the same way.

Choose a format version and decide when to change it. Reassigning an opcode,
changing a payload, or interpreting metadata differently can break compatibility
even when the Rust types still compile. Reject unsupported versions clearly,
or write an explicit conversion policy.

## Verify source and binary agreement

Load equivalent source and binary programs, then compare their results, control
flow, and retained metadata. Include malformed lengths, bad references,
unsupported opcodes, numeric limits, and floating-point bit patterns in your
tests. Keep exact wire layouts with the codec's reference documentation.

Next, see some [patterns for larger machines](/guide/patterns).
