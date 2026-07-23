---
layout: ../../layouts/Guide.astro
title: Guides
slug: index
description: Guide-style documentation for building on top of vihaco — instructions, parsers, messages, components, observers, and composites.
---

# Guides

These guides explain how to build on top of `vihaco`: how to define an
instruction set, parse source text into it, resolve execution input, execute
components, observe their effects, and compose everything into a machine.

For the type-by-type API reference, see the generated [rustdoc](/reference).

## Recommended reading order

1. [Defining Instructions With `vihaco`](/guide/instructions)
   Start with instruction enums and width inference.
   - [Advanced Instruction Usage](/guide/instructions-advanced)
     Explicit opcodes, explicit widths, and machine-level wrapper instructions.
2. [Parser Integration for Component Instructions](/guide/parser)
   The `Parse` trait and the legacy parser attributes retained for existing
   instruction enums.
   - [Pattern Parser Generator](/guide/parser-patterns)
     The recommended parser-authoring workflow for new syntax: declarative,
     compile-time-checked patterns for instruction, value, and type enums and
     structs.
   - [Advanced Parser Customization](/guide/parser-advanced)
     Module-level orchestration: device headers, the `ParsedModule` two-pass
     design, `Resolve` impls, sugar expansion, string interning, and label
     resolution.
3. [Using Messages With `vihaco`](/guide/messages)
   How a runtime resolves execution input and supplies messages to components.
4. [Building Components With `vihaco`](/guide/components)
   Connect instructions, messages, effects, and `#[component(...)]`.
5. [Observing Effects With `#[observe]`](/guide/observers)
   How `#[observe]` works — on standalone observers and on components that also
   react to effects.
6. [Defining A Composite With `vihaco`](/guide/composites)
   Compose components and observers with the transitional `#[composite]`
   wiring.
