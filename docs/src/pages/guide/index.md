---
layout: ../../layouts/Guide.astro
title: Vihaco Author’s Guide
slug: index
description: "Build dialects, components, composites, and independently loaded programs with vihaco."
---

# Vihaco Author’s Guide

This guide shows you how to build a virtual machine with vihaco. You'll define
its instructions, write the devices that execute them, and connect those devices
into a machine. Then you'll load a program and run it.

**Version:** these docs cover the new program-image protocols in the current
vihaco source. Use the setup below for the examples, and keep all vihaco
dependencies on the same revision.

```toml
[package]
name = "my-machine"
version = "0.1.0"
edition = "2024"

[dependencies]
vihaco = { git = "https://github.com/QuEraComputing/vihaco", features = ["program-image-execution"] }
vihaco-stdlib = { git = "https://github.com/QuEraComputing/vihaco", features = ["program-image-execution"] }
eyre = "0.6"
```

Most of the declarations and execution APIs you'll use come from `vihaco`.
`vihaco-stdlib` provides the program dialect and loading examples. If you'd like
a ready-made computation device, add `vihaco-cpu` from the same revision.
You'll also need a Rust toolchain that supports the workspace's edition and
dependencies.

## Read the guide

| Chapter | What you'll learn |
|---|---|
| [The mental model](/guide/mental-model) | Meet the pieces and see how a program moves through them |
| [Your first machine](/guide/first-machine) | Write a component and run it in a complete program |
| [Dialects and instructions](/guide/instructions) | Define the operations your components will implement |
| [Writing components](/guide/components) | Give a device state and implement `Execute` |
| [Preparing messages](/guide/messages) | Supply runtime input with `BuildMessage` |
| [Stack inputs and outputs](/guide/stack) | Move typed operands into and out of a component |
| [Writing composites](/guide/composites) | Connect devices, including nested ones, and route instructions |
| [Executing one instruction](/guide/execution) | Follow `Dispatch`, `Step`, `BuildMessage`, `Execute`, and `Apply` |
| [Handling effects](/guide/effects) | Decide what happens to a component's results |
| [Running programs](/guide/program-execution) | Work with frames, control flow, entry, and recovery |
| [Images and Loadables](/guide/loadables) | Own programs, replace images, and reset execution |
| [Parsing source](/guide/parser) | Give instructions a source spelling |
| [Loading SST programs](/guide/parser-advanced) | Read sections, prepare metadata, and resolve symbols |
| [Binary formats](/guide/instructions-advanced) | Encode programs and plan for compatibility |
| [Larger machines](/guide/patterns) | Share message preparation and steps, and swap backends |
| [Testing and extending](/guide/testing) | Check behavior and add new pieces |

## Find your starting point

If you're new to vihaco, start with the mental model and first machine. After
that, you can follow the chapters in order or focus on the part you're building.

For a component, read about dialects, execution, and the messages and effects
that connect it to a machine. For a composite, keep going through routing,
steps, and program ownership. If you're writing a source or binary frontend,
the parsing, metadata, and loading chapters cover that work.

Keep the [pattern reference](/guide/parser-patterns) nearby for parser grammar
details. The [API reference](/reference) covers individual Rust types and methods.
