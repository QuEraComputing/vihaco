# CPU `runtime` Block Plan

## Current implementation status

The runtime-block implementation is now present in the working tree.

Implemented:

- `component!` parses an optional `runtime { ... }` block.
- Runtime blocks support type aliases, value aliases, and individual runtime
  instruction product declarations.
- The macro generates `runtime::Type`, `runtime::Value`, and individual
  `runtime::instruction::*` product structs.
- Syntax variants support unit, tuple, and named payloads, with optional
  parser patterns.
- `vihaco-cpu` has one `component!` invocation containing all CPU state, the
  complete runtime instruction declaration, runtime aliases, and surface
  syntax declarations.
- The handwritten CPU runtime instruction enum has been removed.
- CPU execution, display, conversions, and tests use the generated runtime
  instruction type.
- Per-instruction `Execute<I>` implementations are the canonical CPU boundary.
- A containing `composite!` owns the machine-local instruction sum, encoding,
  route dispatch, and message resolution.

Focused verification has passed:

```text
cargo test -p vihaco --test component_macro
cargo test -p vihaco-cpu --all-targets
git diff --check
```

The remaining items below are follow-up work and compatibility decisions, not
requirements to recreate the already-completed runtime-block migration.

## Goal

Update `component!` and `vihaco-cpu` so the CPU declaration has three explicit
parts in one invocation:

```rust
component! {
    component CPU { /* all CPU state */ }

    runtime {
        /* resolved runtime types, values, and individual instruction products */
    }

    syntax {
        /* surface types, values, and instruction patterns */
    }
}
```

The CPU remains monolithic. This change must preserve existing CPU behavior,
public APIs, parser spellings, instruction ordering, and encoded opcode values.

## Macro changes

The implemented `component!` runtime block contains:

- runtime type aliases, initially including `Type = vihaco::Type`;
- runtime value aliases, initially including `Value = vihaco::Value`; and
- individual runtime instruction product declarations with their payloads.

The runtime block does not generate a grouped instruction enum, opcodes, or
encoding. Those are machine-level concerns owned by `composite!`.

The existing `instruction { ... }` form may remain for ordinary components
whose execution boundary is independent instruction products. Do not silently
reinterpret existing component declarations.

Completed macro tests cover:

- runtime type/value aliases;
- individual runtime instruction product generation;
- representative product construction behavior;
- coexistence of `runtime` and `syntax` blocks; and
- regression coverage for existing component declarations.

## CPU migration status

Move the following into one `component!` invocation in `vihaco-cpu`:

- every `CPU` state field, including stack, frames, heap, span, program-counter
  state, and return values;
- the complete set of individual runtime instruction products;
- runtime `Type` and `Value` aliases;
- `SurfaceType`, `SurfaceValue`, and `SurfaceInstruction`; and
- all existing `cpu::` syntax patterns.

Duplicate authoritative declarations have been removed. Handwritten code now
remains only for:

- per-instruction `Execute<I>` implementations and shared `op_*` semantics;
- display, conversions, and compatibility forwarding; and
- tests.

Preserve these public exports where possible:

- `CPU`;
- the individual generated runtime instruction product types;
- `SurfaceInstruction`, `SurfaceType`, and `SurfaceValue`;
- typed CPU message types; and
- `StepOutcome`.

## Execution-boundary migration

The individual runtime products are the component execution boundary. The
containing composite generates the encoded/runtime instruction sum and routes
each payload to one implementation per product:

```rust
impl Execute<cpu::runtime::instruction::Add> for CPU { /* ... */ }
impl Execute<cpu::runtime::instruction::Branch> for CPU { /* ... */ }
impl Execute<cpu::runtime::instruction::Load> for CPU { /* ... */ }
```

Requirements:

- Preserve the existing CPU state and all operation semantics.
- Reuse the existing `op_*` methods as implementation bodies wherever
  possible.
- Preserve typed messages, `StepOutcome`, effects, and fault behavior.
- Give each instruction the appropriate `Message`, `Effect`, and `Fault`
  associated types. Shared types are acceptable where behavior is identical.
- Do not split CPU state into ALU, stack, heap, or control-flow components.
- Make the containing composite responsible for building and dispatching its
  grouped encoded instruction into the appropriate runtime product execution.

`CPU::execute_instruction(Instruction)` may remain temporarily as a forwarding
compatibility adapter, but it must not be the canonical implementation. If it
would require retaining a second large dispatch match, remove it or document
the API change instead of preserving the old dispatcher by default.

## Remaining acceptance work

The runtime-block implementation already satisfies the structural criteria:

1. CPU state, runtime instruction information, runtime aliases, and syntax
   information each have one authoritative declaration inside `component!`.
2. No separately handwritten CPU runtime instruction enum remains.
3. Component products no longer own runtime instruction ordering or opcodes;
   those belong to the containing composite.
4. Existing focused macro and CPU tests pass.

The runtime-block acceptance items are now resolved:

5. The containing composite can define its own runtime instruction sum from
   the CPU's individual products.
6. The runtime aliases resolve to the existing ABI `Type` and `Value` types.
7. Runtime product namespace assertions are present.

The next integration step is a composite-level test that builds a machine-local
instruction sum and routes its payloads to the CPU's `Execute<I>` implementations.

Run the full repository checks after those decisions are implemented:

```text
   cargo fmt --all -- --check
   cargo test -p vihaco --test component_macro
   cargo test -p vihaco-cpu --all-targets
   cargo test --workspace --all-targets
   cargo test --workspace --doc
   cargo clippy --workspace --all-targets -- -D warnings
   hawkeye check
```

## Ownership boundary

`vihaco-cpu` does not own syntax-to-runtime resolution, module construction, or
program loading. Those responsibilities belong to the composite that mounts
the CPU. The composite selects CPU instructions, parses the CPU syntax,
resolves symbols/types/values, lowers them into the generated CPU runtime
instruction type, and loads the resulting program.

The CPU component only supplies the declarations and execution behavior needed
by that composite. A composite-level consumer test is useful, but this plan
does not add a CPU-specific `Resolve` implementation, `ProgramImage`, loader,
or composite wrapper.

## Scope note

The working tree also contains changes to demos, docs, and unrelated composite
tests. This plan covers the runtime-block and CPU declaration migration; those
other changes should be treated as pre-existing branch work unless explicitly
assigned to the follow-up integration phase.

## Non-goals

- Splitting CPU into multiple components.
- Changing CPU execution semantics or state layout.
- Replacing the global ABI `vihaco::Type` or `vihaco::Value` types with new
  CPU-specific runtime types.
- Keeping the grouped `Instruction` dispatcher as the canonical CPU execution
  boundary.
- Implementing syntax-to-runtime resolution, module/program loading, or a
  composite wrapper for the CPU.
