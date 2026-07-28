# Instruction and Data-Model Rewrite Verification and Migration

This document turns the architecture into test coverage, migration phases, implementation
questions, and acceptance criteria.

## Testing Strategy

Tests follow the same boundaries as the architecture. Narrow tests establish each product and trait
relationship; route and end-to-end tests prove that generation composes them without widening the
machine's public instruction set.

### Surface Instruction Tests

Each surface instruction is tested for:

- Pattern parse round trip for the canonical dialect-qualified form.
- Generated default pattern equivalence where a default is allowed.
- Tuple-index and named-field binding order.
- Nested value/type field parsers.
- Preservation of unresolved names, labels, and symbolic operands.
- Invalid source syntax rejection.

### Surface Value and Type Tests

Author-defined value and type products are tested for:

- Composition from vihaco's scalar and lexical parsers.
- Module parameter and return types using the author-selected surface type.
- Typed literal variants rejecting invalid type/literal pairings where the grammar expresses the
  pairing.
- Unresolved literal text preserving the source needed by resolution.
- Out-of-range scalar input returning a parse error without panicking.
- A surface product participating in parsed modules without implementing runtime bytecode traits.

### Resolution Tests

Each `Resolve<SurfaceInstruction, SurfaceType, Header>` implementation is tested for:

- Successful lowering to the expected runtime instruction or instruction sequence.
- Label and symbol replacement with the correct program-image indices.
- Errors for missing, duplicate, or invalid targets.
- Sugar expansion order.
- Machine-specific validation that requires module context.
- Author-defined surface type and literal lowering.
- Explicit source-language conversion insertion.

The `ConditionalBranch` reference case anchors the boundary: `@foo` survives parsing as a source
label and becomes a fixed-width `InstructionIndex` only during module resolution.

### Runtime Instruction Tests

Each runtime instruction is tested for:

- Construction with fully resolved values.
- Validation of resolved indices and identifiers where applicable.
- Confirmation that no unresolved source-level names remain.

### Component Execution Tests

Each `Execute<I>` implementation is tested for:

- Successful local state transition.
- Fault behavior.
- Message/instruction pairing.
- Emitted effects.
- Documented partial mutation behavior.

### Composite Route Tests

Each composite route test establishes that:

- The surface instruction is present in the machine surface sum.
- The resolved runtime instruction is present in the machine runtime sum.
- The expected field is selected.
- Message data comes from the correct components.
- Effects reach the correct handlers.
- Duplicate instruction types routed to different fields remain distinct.
- Optional route metadata reaches the configured driver.
- Only explicitly selected surface instruction patterns are accepted.
- Prefix-related mnemonics select the correct route regardless of route declaration order.

### Compile-Fail Tests

Compile-fail coverage proves that invalid relationships cannot be generated. It rejects:

- A selected instruction unsupported by its target component.
- Duplicate public variant names.
- Missing message wiring.
- Missing effect handlers.
- Incompatible message or effect types.
- Cross-component value types that differ without an explicit adapter.
- A suspending effect without a continuation-capable handler.
- A selected surface instruction that does not implement `Parse`.
- A machine surface sum with no applicable
  `Resolve<MachineSurfaceInstruction, MachineSurfaceType, Header>` implementation.
- Attempting to route a surface instruction directly to component execution.
- Invalid pattern field mappings and unsupported pattern literals.

### End-to-End Tests

End-to-end machines cover:

- A stack-local instruction.
- A pure arithmetic instruction using stack resolution and handling.
- A heap operation spanning stack and heap.
- A control-flow effect.
- An effect handled by both a stateful component and a diagnostic component.
- A parked receive and resumed continuation.
- A sequential driver with a driver-owned cursor.
- A timeline driver coordinating a global clock with child clocks.
- A machine-owned program counter changed by a modeled hardware component.
- A nested composite exposing only selected operations.
- Pattern parsing into a surface instruction, module resolution into a runtime instruction, and
  runtime message resolution before execution.
- One machine using a scalar directly without defining a value enum.
- One author-defined heterogeneous value carrier crossing stack, heap, and channel boundaries.

## Migration Plan

Migration proceeds from the semantic relationships outward. Manual instruction and execution types
establish the model first; generation follows only after the required relationships are concrete.

### Phase 1: Establish Surface, Runtime, and Data-Model Boundaries

1. Establish distinct surface and runtime instruction types.
2. Decide the final names for surface instructions, runtime instructions, and their generated
   machine sums.
3. Remove vihaco's built-in guest `Value` and `Type` enums.
4. Provide fallible `Parse` implementations for the supported scalar source forms.
5. Distinguish identifier, symbol, quoted-string, and unresolved-literal helpers.
6. Parameterize parsed function signatures over an author-selected surface type.
7. Keep the surface-instruction marker independent of runtime instruction/bytecode traits.
8. Use the pattern parser generator for all instruction, value, and type surface syntax.
9. Make `Resolve<SurfaceInstruction, SurfaceType, Header>` the explicit lowering boundary.
10. Add a reference branch instruction whose surface form contains labels and whose runtime form
    contains resolved `InstructionIndex` values.
11. Test that the generated machine surface sum resolves into a module containing only variants
    from the generated runtime sum and author-defined constant/type products.

### Phase 2: Introduce Per-Instruction Component Execution

1. Add the `Execute<I>` relationship.
2. Add `NoMessage`, `NoEffect`, and typed fault conventions.
3. Implement several manual examples before designing ergonomic macros.
4. Start with stack-native `Push`, `Drop`, and `Dup`.
5. Add one pure operation such as `Add`.
6. Add one cross-component operation such as `Allocate`.

### Phase 3: Generate Explicit Composite Routes

1. Extend or replace `#[composite]` with explicit instruction selection.
2. Generate a machine surface-instruction sum and a machine runtime-instruction sum from the
   selected routes.
3. Generate the pattern-based machine parser from only the selected surface instructions.
4. Generate the outer runtime dispatch match.
5. Support the same runtime instruction type routed to multiple fields.
6. Require the resolver's output module to use the selected machine runtime sum.

### Phase 4: Add Message Resolution and Effect Wiring

1. Generate `NoMessage` and `NoEffect` defaults only when no explicit policy is present.
2. Add route-local runtime message resolver methods.
3. Add route-local effect handling.
4. Support deterministic delivery of one effect to multiple typed handlers.
5. Define deterministic ordering for multiple and follow-up effects.

### Phase 5: Add Drivers, Timing, and Suspension

1. Establish the one-instruction `Step` boundary and its owned outcome.
2. Add a sequential driver with an explicitly owned program cursor.
3. Add `Complete` and `Parked` driver semantics.
4. Add owned continuation registration for `Receive`.
5. Reject borrowed continuation state.
6. Add a timeline driver that owns global time and consumes driver-facing scheduling requests.
7. Demonstrate a child clock as an ordinary component and handler.
8. Demonstrate a machine-owned program counter controlled by a modeled hardware component.
9. Test reset generations and stale completions.

### Phase 6: Migrate Existing Components

1. Split each component-wide instruction enum into individual surface and runtime instruction
   structs.
2. Group source files by semantic family: stack, arithmetic, heap, control flow, I/O, and runtime
   metadata.
3. Give every surface instruction its canonical `#[syntax_class(instruction, head = ...)]` and
   `#[pattern = ...]` declarations.
4. Move special field grammars into local value/type syntax types where practical.
5. Represent sugar, interning inputs, labels, and other unresolved operands explicitly in surface
   instruction types.
6. Replace old `Value`/`Type` dependencies with scalars, generics, library newtypes, or an
   author-defined data model as appropriate.
7. Implement `Resolve` to lower those forms into executable runtime instructions.
8. Move component-local mutations to `Execute<I>` implementations.
9. Move cross-component reads into runtime message resolution.
10. Move cross-component writes and scheduling into effect handling.

### Phase 7: Remove Automatic Instruction Inheritance

1. Stop generating one machine variant per component instruction enum.
2. Require explicit route selection for new composites.
3. Deprecate the component-wide `GeneratedComponent::Instruction` association.
4. Remove adapters after downstream code and documentation have migrated.

### Phase 8: Establish Resolved Bytecode Encoding

1. Separate surface parsing traits from runtime encoding and decoding traits.
2. Implement portable codecs for supported fixed-width scalars and generic containers.
3. Preserve one global context and the recursive section frame, local header, local payload, child
   table, and child-offset structure.
4. Add author-defined codec coverage for one scalar-only section and one heterogeneous data-model
   section in the same file.
5. Generate explicit stable route opcodes scoped to each section's machine runtime-instruction sum.
6. Decide whether section schema identities live in fixed framing or author headers, and test
   mismatches at the section path that selected the decoder.
7. Encode variable-sized local instruction records with checked lengths and exact payload
   consumption.
8. Validate unique expected child names, parent-relative offsets, containment, and non-overlap.
9. Reject `usize`, implicit Rust discriminants, invalid tags, invalid indices, and trailing payload
   data at the wire boundary.
10. Prove that recursive SST resolution and bytecode decoding establish equivalent per-section
    invariants.

## Additional Architecture Coverage

[`demo.md`](./demo.md) is the only end-to-end reference runtime. It exercises nested composites,
heterogeneous clocks, arithmetic reuse, cross-device communication, suspension, and timeline
driving as one coherent machine.

The demo does not need to contain every operation used to validate the instruction architecture.
The remaining boundaries are better established through focused component tests, route tests, and
small conformance fixtures:

| Coverage case | Architectural boundary | Test scope |
|---|---|---|
| `Push` and `Drop` | Owner-local stack mutation requires no self-directed effect, while composite selection still controls instruction availability | Component and route tests |
| `Load` | A component-local load may mutate one combined stack/frame owner, while split storage uses message resolution and effect handling | Alternative route fixtures |
| `Allocate` | Values move from a stack to a heap and a reference returns through typed cross-component stages | Focused composite fixture |
| `ConditionalBranch` | SST labels survive parsing, resolve to runtime program indices, and update either a driver-owned or machine-owned program counter | Resolver and control-flow fixture |
| `Call` | Program metadata, call-stack mutation, frame construction, return placement, and program-counter policy remain distinct responsibilities | Focused control-flow fixture |
| `Print` | Value acquisition remains separate from output delivery, and one effect may reach output and diagnostic handlers | Focused handler fixture |
| Simple sequential execution | `step` remains usable without a clock, and a driver-owned cursor can advance a resolved program | Small end-to-end fixture |

These cases do not need to be assembled into a second general-purpose machine. Their purpose is to
prove individual boundaries that the two-CPU demo does not exercise directly. The sequential
fixture is an implementation milestone and a fast test harness, not another reference runtime.

## Questions to Revisit After the First Implementation

Several API choices depend on evidence from the first implementation:

1. The final names for surface instructions, runtime instructions, and their generated sums.
2. How one-to-many lowering is represented while `Resolve` builds the runtime module.
3. Whether execution should eventually return something other than `Effects<E>`.
4. Whether pure operations use zero-sized executor components or a dedicated adapter.
5. How fact events emitted after direct mutation are distinguished from command effects.
6. Whether canonical dialect heads are always fixed by surface instruction types or may be wrapped
   by an explicit machine-local surface instruction type.
7. Which validation belongs in pattern parsing and which belongs in `Resolve`.
8. Whether repeated author data-model parameters justify a common packaging trait.
9. Whether generic tooling eventually requires self-describing type schemas in bytecode.

Borrow-specific APIs and macro shorthand follow the same rule: they are introduced in response to
concrete compiler friction or repeated boilerplate, not as prerequisites for the architecture.

These questions do not change the central ownership decision:

> Components own their state and per-instruction execution; composites own instruction admission,
> route dispatch, cross-component dataflow, and effect routing; drivers own program iteration,
> readiness, scheduling, and modeled time; either a driver or one modeled component owns
> program-counter transitions. Data-model authors own semantic values and types; vihaco supplies
> scalar, staging, composition, and encoding infrastructure.

## Acceptance Criteria

The rewrite has established the architecture when all of the following are true:

- Adding a component field does not automatically add runtime instructions.
- A composite can select two runtime instructions from a component that executes ten.
- A composite can admit a surface form without assuming a one-to-one runtime counterpart.
- The same instruction can be routed to two component instances without trait conflicts.
- An unsupported surface instruction is rejected by the generated pattern parser.
- An individual surface instruction struct can derive its canonical parser with
  `#[syntax_class(instruction, head = ...)]` and `#[pattern = ...]`.
- The generated machine parser admits only selected surface instruction patterns.
- Pattern parsing, `Resolve`, runtime message resolution, execution, and effect handling remain
  distinct stages.
- Vihaco exports no required guest `Value` or `Type` enum.
- Parsed function signatures use an author-selected surface type.
- A scalar-only machine does not need to define a value enum.
- Author-defined heterogeneous values can cross compatible component boundaries.
- Mismatched boundary types require an explicit conversion instruction, adapter, or handler.
- A surface `ConditionalBranch` can contain `@foo`, while its runtime counterpart contains only a
  resolved fixed-width `InstructionIndex`.
- Runtime instructions contain no unresolved source labels, names, or sugar.
- Only runtime instructions are dispatched to components.
- Native stack mutation requires no artificial self-directed effect.
- Arithmetic can be reused without knowing about stack layout.
- Heap allocation can move values across stack and heap through typed stages.
- Effects are routed deterministically and with route provenance.
- A receive instruction can park and resume without retaining borrows.
- The same composite can be run by a simple sequential driver or a timeline driver.
- Calling `step` directly does not require a program, program counter, or clock.
- Program storage and cursor state can have different owners.
- A machine-owned program counter can be advanced by modeled hardware without competing with
  driver-owned advancement.
- Driver-facing scheduling requests cross the step boundary as owned state.
- Nested composites expose only their selected public instruction set.
- Compile errors identify the route and missing component/message/effect relationship.
- Existing diagnostic-handler and loader concepts can integrate without becoming the semantic
  owner of instruction execution.
- Bytecode round trips author-defined instructions, constants, and types without depending on Rust
  layout, variant order, or pointer width.
- One bytecode file can load a root composite and heterogeneous nested sections whose owners use
  different instruction, constant, type, header, and opcode schemas.
