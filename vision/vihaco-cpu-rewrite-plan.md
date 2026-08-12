# vihaco-cpu Rewrite Migration Plan

## Goal

Update `vihaco-cpu` to work with the instruction-pipeline rewrite on
`rob/instruction-pipeline-rewrite`, while preserving the CPU's existing public
shape and behavior.

This is an integration migration, not a CPU architecture refactor. The
following remain conceptually unchanged:

- `CPU` remains the stack-machine component.
- The CPU's individual runtime instruction products move into `component!`.
  The containing composite owns the machine-level instruction sum,
  encoding, and dispatch.
- `SurfaceInstruction`, `SurfaceType`, and `SurfaceValue` remain the source
  syntax model.
- Typed messages remain the message boundary. `FunctionInfo` and `Print` are
  supplied or resolved by the containing composite.
- Existing program-counter, call/return, stack/frame, heap, arithmetic,
  comparison, printing, reset, and control-flow behavior is preserved.
- Existing parser spellings and runtime instruction ordering must remain
  stable unless a compatibility issue is identified and explicitly accepted.

The target is to make the existing CPU fit the rewrite's concepts and APIs,
not to split the CPU into smaller components yet. The CPU exposes individual
instruction structs and `Execute<I>` implementations; it does not own a
grouped instruction enum or machine-level dispatch.

The `component!` invocation must become the single declaration site for the
CPU component's structural information:

- all CPU component state fields;
- all runtime instruction product information; and
- all CPU syntax information, including syntax types, values, instruction
  patterns, and the CPU namespace.

Handwritten Rust remains responsible for execution behavior, operation
implementation, resolution policy where the macro does not generate it, and
tests. The declaration should not be duplicated across separate state structs,
runtime enums, or parser-only declarations.

## Current state

The branch already contains the rewritten module, syntax, runtime, and
component/composite infrastructure. `vihaco-cpu` still uses the older direct
execution boundary:

- The CPU component exposes individual runtime instruction products generated
  by `component!`; it does not define the encoded instruction sum.
- `SurfaceInstruction` is parsed separately and carries symbolic operands.
- `CPU` implements one `Execute<I>` instance per instruction product, with
  typed messages and effects.
- `CPU` stores execution state including pending/current program-counter data.
- The CPU crate currently compiles with `cargo check -p vihaco-cpu`.

The main migration risk is therefore API and ownership alignment, especially
around source resolution, program loading, and control-flow effects—not the
operation implementations themselves.

## Target shape

Keep the existing CPU implementation, but adapt its boundaries to the rewrite:

```text
existing SurfaceInstruction
        |
        v
rewrite parser / ParsedModule / Resolve
        |
        v
individual CPU runtime instruction products
        |
        v
rewrite Execute<I> / StepResult / Effects
        |
        v
existing CPU operation and program-counter behavior
```

The structural target is a single `component!` declaration containing the
existing CPU shape, expressed without changing its semantics:

```rust
vihaco::component! {
    component CPU {
        // Existing CPU state, including stack, frames, heap, spans,
        // program-counter state, and return values.
    }

    instruction {
        // Individual runtime instruction products and their payloads.
    }

    syntax {
        // The existing SurfaceType, SurfaceValue, and SurfaceInstruction
        // information, including the `cpu::` patterns.
    }
}
```

The exact supported syntax block should follow the current rewrite macro
grammar. If the macro cannot yet express one of the existing CPU declarations,
that is an implementation prerequisite or a narrowly scoped macro enhancement;
the CPU migration should not create a permanent parallel declaration instead.

If the rewrite requires a composite wrapper for executable loading, add the
smallest compatibility composite necessary to host the existing CPU. That
wrapper may own the rewritten `ProgramImage` and generated loading plumbing,
but it must delegate instruction semantics to the unchanged `CPU`.

## Work plan

### 1. Establish compatibility constraints

Before changing code, record the behavior that must not change:

- Runtime instruction variant order and derived opcode values.
- Surface instruction names, namespaces, separators, and malformed-input
  rejection behavior.
- `CPUMessage` validation rules for `Print` and `FunctionInfo`.
- `StepOutcome` values and their meanings.
- Pending program-counter behavior for branches, calls, and returns.
- Stack/frame layout and heap allocation/deallocation behavior.
- Existing public exports from `vihaco-cpu`.

Use the existing tests as the baseline. Add characterization tests first where
behavior is currently implicit rather than changing semantics during the
migration.

### 2. Move CPU structure into `component!` and adapt the execution boundary

Move the existing CPU declaration information into one `component!` invocation,
then update execution to the rewritten runtime contracts:

- Declare individual runtime instruction products through the macro; do not
  split CPU operations into separate components.
- Declare the CPU state fields, runtime instruction payloads, and surface syntax
  in the same macro invocation.
- Implement `Execute<I>` for each generated instruction product. The containing
  composite supplies the grouped instruction enum and dispatches its payloads
  to these implementations.
- Return the rewritten `StepResult` and `Execution` values.
- Preserve the appropriate typed effect channels, including `StepOutcome` for
  control-flow instructions and `PrintEffect` for printing.
- Keep message resolution in the containing composite and operation behavior in
  the CPU's individual `Execute<I>` implementations.

Do not introduce per-operation component products, split the CPU into ALU,
stack, heap, or control-flow components, or move program-counter semantics
into a new architecture as part of this work.

### 3. Complete macro-owned syntax and module resolution

With CPU syntax declared in `component!`, connect the generated or macro-owned
syntax products to the rewritten typed syntax/module pipeline:

- Identify the current rewrite entry point for parsing a `ParsedModule`.
- Implement or adapt the CPU-side `ModuleSyntax`/`Resolve` integration using
  the syntax information from the macro invocation.
- Lower symbolic labels and function references into the individual CPU runtime
  instruction products expected by the CPU.
- Preserve CPU type/value conversion behavior represented by the macro-owned
  syntax types and values.
- Preserve string interning, constants, function metadata, and source-symbol
  behavior.

Resolution state should remain outside the instruction operation methods. The
CPU should receive the same resolved runtime values it receives today.

### 4. Adapt executable loading if required

If the rewritten loader expects a composite-owned program container:

- Add a minimal CPU-facing wrapper or compatibility composite.
- Use `ProgramImage` or the appropriate rewritten container.
- Route loaded instructions to the existing `CPU`.
- Preserve program-counter initialization and advancement behavior.
- Preserve the CPU's handling of `FunctionInfo`, labels, source spans, and
  return values.

Do not redesign scheduling or introduce a new machine execution loop unless
the current rewrite API makes an adapter impossible. Any such addition should
be narrowly scoped and tested against the existing behavior.

### 5. Preserve and update public API coverage

Review all exports in `crates/vihaco-cpu/src/lib.rs` and keep the existing
names available where practical. If an API must change because of the rewrite:

- Prefer a compatibility alias or forwarding implementation.
- Mark old names deprecated only when the replacement is usable.
- Document any unavoidable breaking change.
- Update dependent demos and doctests in the same change.

### 6. Verification

Run focused tests after each workstream, then the full repository checklist:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p vihaco-cpu --all-targets
cargo test --workspace --all-targets
cargo test --workspace --doc
hawkeye check
```

The final change should include regression coverage for both the rewritten
integration points and unchanged CPU behavior.

## Parallel subagent strategy

The work can be split into mostly independent workstreams. Each subagent
should work from the same baseline commit, avoid broad formatting or unrelated
cleanup, and report changed files, assumptions, and verification results.

### Recommended workstreams

#### Agent A: Compatibility inventory and characterization tests

Scope:

- Audit current `vihaco-cpu` exports and dependent usages.
- Record opcode, parser, message, and control-flow compatibility requirements.
- Add characterization tests for behavior not already covered.

Files primarily touched:

- `crates/vihaco-cpu/src/lib.rs`
- `crates/vihaco-cpu/src/instruction.rs`
- `crates/vihaco-cpu/src/component.rs`
- New CPU tests, if needed.

This workstream should land first or provide its test patch to the integration
agent. It should not alter CPU semantics.

#### Agent B: Runtime execution adapter

Scope:

- Adapt `Execute`, `StepResult`, `Execution`, and `Effects` usage to the
  rewritten API.
- Preserve typed messages, `StepOutcome`, and all operation behavior in the
  individual `Execute<I>` implementations.
- Add focused tests for message validation and effect/control-flow results.

Files primarily touched:

- `crates/vihaco-cpu/src/component.rs`
- `crates/vihaco-cpu/src/outcome.rs`

This agent should not modify syntax resolution or redesign the CPU state.

#### Agent C: Component declaration and syntax/resolution adapter

Scope:

- Move all CPU state, runtime instruction information, and syntax information
  into one `component!` invocation.
- Map the macro-owned syntax products into the rewritten parser/module
  abstractions.
- Implement the smallest CPU-side resolution layer needed to produce the
  individual CPU runtime instruction values.
- Preserve symbolic operand and parser behavior.

Files primarily touched:

- `crates/vihaco-cpu/src/lib.rs`
- `crates/vihaco-cpu/src/instruction.rs` (removing duplicated declarations or
  retaining only handwritten conversion/test code)
- `crates/vihaco-cpu/src/data.rs` (moving state declaration into the macro)
- New CPU resolution module, if appropriate.
- `crates/vihaco-cpu/Cargo.toml` only if a dependency adjustment is required.

This agent should coordinate with Agent B on generated names and with Agent D
on the exact loader/composite entry point, but can develop parser and lowering
tests independently.

#### Agent D: Loading/composite integration spike

Scope:

- Determine whether `vihaco-cpu` needs a minimal rewritten composite or loader
  adapter.
- Prototype the smallest integration with `ProgramImage`, generated loading,
  and the existing CPU.
- Document any unresolved ownership or API constraints.

Files primarily touched:

- `crates/vihaco-cpu/src/lib.rs`
- New loading/composite module, if required.
- Integration tests.

This should begin as a short spike. If no wrapper is required, the agent
should provide evidence and leave the code unchanged.

#### Agent E: Dependent examples and documentation

Scope:

- Find all `vihaco-cpu` consumers in demos, README, and doctests.
- Update only the API references required by the migration.
- Add or update a small usage example showing the rewritten integration.

Files primarily touched:

- `demos/`
- `docs/`
- `README.md`

This workstream should wait for the execution and loading API shapes to settle,
but the usage inventory can happen immediately.

### Dependency graph

```text
Agent A: compatibility tests ───────────────┐
                                             v
Agent B: execution adapter ─────────────> Integration
Agent C: syntax/resolution adapter ─────> Integration ───> Agent E: docs/examples
Agent D: loading/composite spike ───────┘
```

Agents B, C, and D can investigate in parallel. Their implementation branches
should be merged in this order:

1. Agent A’s characterization tests.
2. Agent B’s runtime adapter.
3. Agent C’s syntax/resolution adapter.
4. Agent D’s loader/composite changes, resolving conflicts with the preceding
   adapters.
5. Agent E’s dependent examples and documentation.

If Agent D discovers that a composite wrapper is unnecessary, merge its design
finding rather than speculative code.

## Integration protocol

The coordinating agent should:

1. Create one branch or worktree per workstream from the same baseline.
2. Require each agent to run focused tests before handoff.
3. Merge narrow commits in dependency order.
4. Resolve conflicts by preserving the existing CPU behavior and public API,
   not by accepting whichever branch was merged last.
5. Run `cargo fmt --all` only after the logical merge is complete.
6. Run the full workspace checklist, including doctests and SPDX checks.
7. Review the final diff specifically for accidental instruction reordering,
   changed parser spellings, or newly introduced CPU decomposition.

## Explicit non-goals

This migration does not:

- Split `CPU` into multiple components.
- Change the CPU's behavior or semantic ownership of its existing operations.
- Maintain separate authoritative declarations for CPU state, runtime
  instruction information, or syntax information outside `component!`.
- Redesign the stack/frame/heap representation.
- Change instruction semantics or scheduling policy.
- Remove the existing CPU message model without a compatibility adapter.
- Introduce unrelated fixes, cleanup, or performance work.
