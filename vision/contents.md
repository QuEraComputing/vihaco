# Vihaco Vision Contents

This directory describes the in-progress vihaco architecture and the reference machine used to
validate it. The current-direction documents below should be read together: each owns a distinct
part of the design, while the demo provides the integration target.

## Architecture

Read these documents in order when following the instruction rewrite from its type model through
runtime execution:

1. [`instruction-model.md`](./instruction-model.md) defines surface and runtime instruction
   products, `Instruction` and `Execute<I>`, component responsibilities, composite selection, and
   generated machine instruction sums.
2. [`types-and-values.md`](./types-and-values.md) defines author-owned data models, scalar parser
   and encoding support, surface/runtime type and value staging, cross-component compatibility,
   explicit conversion, and future bytecode encoding.
3. [`execution-pipeline.md`](./execution-pipeline.md) defines surface resolution and the
   route-specific runtime stages of message resolution, component execution, and effect handling.
4. [`runtime-drivers.md`](./runtime-drivers.md) defines step outcomes, program drivers, clock and
   driver roles, program-counter ownership, parking, resumption, and fault boundaries.
5. [`stack-machine-policy.md`](./stack-machine-policy.md) applies the ownership model to native
   stack operations, arithmetic, locals, heap allocation, printing, calls, and control flow.
6. [`sst-resolution.md`](./sst-resolution.md) defines pattern-based SST parsing, surface-to-runtime
   resolution, canonical syntax ownership, and the generated composite parser.
7. [`macro-generation.md`](./macro-generation.md) separates what instruction, component,
   composite, and effect-wiring macros generate from what machine authors write.
8. [`design-tradeoffs.md`](./design-tradeoffs.md) records the alternatives considered and the
   architecture's observability, debugging, and error-model consequences.
9. [`implementation-plan.md`](./implementation-plan.md) defines test coverage, migration phases,
   focused architecture fixtures, deferred questions, and acceptance criteria.

## Reference Machine and Timing

- [`demo.md`](./demo.md) is the end-to-end integration target: two reusable CPU composites with
  different local clock ratios exchange arithmetic results through a reusable communication
  component.
- [`clock.md`](./clock.md) defines the timeline model needed by that demo, including global and
  local clocks, deterministic scheduling, driver interaction, parking, communication timing, and
  reset behavior. The material above its divider records the earlier questions that motivated the
  current design.

## Earlier Working Notes

- [`vision.md`](./vision.md) is an early, incomplete architecture sketch. It provides historical
  context but includes proposals superseded by the current-direction documents.
- [`traits.md`](./traits.md) is an earlier first-class-traits draft. Its instruction ownership and
  capability ideas are exploratory rather than the current implementation plan.

When these earlier notes conflict with the architecture, reference-machine, or timing documents
above, the current-direction documents take precedence.
