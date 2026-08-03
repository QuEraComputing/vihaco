# Vihaco Vision Contents

This directory describes the in-progress vihaco architecture. The concrete reference-machine
documents now live beside the example under `demos/examples/`; the documents below should be read
together, with the demo providing the integration target.

## Architecture

Read these documents in order when following the instruction rewrite from its type model through
runtime execution:

1. [`types-and-values.md`](./types-and-values.md) defines author-owned data models, scalar parser
   and encoding support, surface/runtime type and value staging, cross-component compatibility,
   explicit conversion, and future bytecode encoding.
2. [`execution-pipeline.md`](./execution-pipeline.md) defines surface resolution and the
   route-specific runtime stages of message resolution, component execution, and effect handling.
3. [`stack-machine-policy.md`](./stack-machine-policy.md) applies the ownership model to native
   stack operations, arithmetic, locals, heap allocation, printing, calls, and control flow.
4. [`sst-resolution.md`](./sst-resolution.md) defines pattern-based SST parsing, surface-to-runtime
   resolution, canonical syntax ownership, and the generated composite parser.
5. [`macro-generation.md`](./macro-generation.md) separates what instruction, component,
   composite, and effect-wiring macros generate from what machine authors write.
6. [`demo-vihaco-concepts.md`](../demos/examples/demo-vihaco-concepts.md) explains every contract
   in the demo's `vihaco` layer with independent examples, including execution, message supply,
   effect routing, suspension, route identity, and the planned effect-fanout macro.

## Reference Machine and Timing

- [`demo.md`](../demos/examples/demo.md) is the end-to-end integration target: two reusable CPU
  composites with different local clock ratios exchange arithmetic results through a reusable
  communication component under a non-executing clock-driven root.
- [`clock.md`](./clock.md) defines the timeline model needed by that demo, including global and
  local clocks, deterministic root event dispatch, parking, communication timing, and reset
  behavior.
