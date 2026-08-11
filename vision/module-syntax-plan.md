# Module Syntax Rewrite: Parallel Implementation Plan

## Purpose and boundary

This is the implementation plan for the module-syntax portion of the larger
[composite syntax/runtime plan](composite-syntax-runtime-plan.md). It turns the
current design into small, parallelizable work packages and identifies the
integration points that must be landed in order.

The target ownership boundary is:

```text
component
    local instruction, value, and source-type syntax
    local parser implementations
    runtime instruction products

composite
    complete module dialect
    public namespaces and aliases
    source-syntax sums
    SST section headers
    semantic analysis and lowering policy
    runtime route selection
```

The source dialect consumed by one SST module is represented by one
`ModuleSyntax` type. Its instruction, value, and type syntax are composed from
the mounted components; its header syntax is owned by the composite that owns
the SST section.

This plan does not redesign runtime effect dispatch. That work is already
largely present in the repository and remains covered by the companion plan.

## Current repository state

The following pieces are already present or substantially implemented. Agents
should preserve them and build on their current APIs rather than recreate them:

- `vihaco-runtime-derive` generates composite `syntax` and `runtime` modules,
  surface parsers, route dispatch, named message resolvers, and program-module
  loading scaffolding.
- `InstallProgramModule` and `BuildProgramModule` exist in
  `vihaco-module`; `ProgramImage` provides the standard implementation.
- Generated loading validates and forwards direct child sections. The recent
  generated loader change makes child forwarding separate from loading the
  composite's own section.
- Parser patterns no longer require an instruction `head` and namespaced
  instruction tokens are supported.
- Runtime route tests, basic installation tests, message resolution, string
  interning, and child forwarding exist.

The main unfinished seams are:

- `vihaco-syntax` still defines `ParsedModule<I, Ty, H>`,
  `ParsedFunction<I, Ty>`, and `Resolve<I, Ty, H>`.
- There is no shared `ModuleSyntax` trait or component `InstructionSet` syntax
  contract in the public API.
- Generated component source sums and composite-owned header syntax are not
  yet wired through the parser and resolver.
- Generated loading still names `LoadOwnSstSection` and `LoadSstSection` and
  still carries independent surface-type/header parameters.
- The existing demo and parser documentation still describe the old generic
  syntax API.

## Agent execution rules

Each agent works on one work package and must:

1. inspect the current code and tests before editing;
2. keep changes inside the listed ownership boundary where possible;
3. add or update focused tests with the implementation;
4. run the narrowest relevant tests, formatting, and compilation checks;
5. report changed files, public API decisions, and unresolved integration
   assumptions.

Do not have parallel agents edit the same implementation file. If an API
   decision affects multiple work packages, the contract agent lands the
   contract first and dependent agents rebase or apply the contract before
   implementation. Generated files under `.agents/` and `.claude/` are not
   hand-edited.

## Dependency graph

```text
                         ┌─ B parser-model migration ─┐
A contracts ─────────────┼─ C component syntax API ────┼─ E composite codegen
                         └─ D loader trait rename ─────┘          │
                                                                  ├─ F lowering/loading integration
                                                                  └─ G nested loading

F ── H metadata/header semantics ── I integration tests ── J demo/docs
C ────────────────────────────────┘
```

The independent tracks can start together after the repository audit, but the
integration agents must consume the contracts from A. Work packages that touch
the same macro call sites are intentionally sequenced to keep merge conflicts
small.

## Step 0 — Coordinator audit and contract freeze

**Owner:** coordinator, before parallel implementation begins.

Record the current behavior and establish the exact public names to use. Read
the current `vihaco-syntax`, `vihaco-module`, `vihaco-runtime-derive` composite
codegen/loadable modules, generated SST tests, demo, and parser guide.

Freeze these decisions in the implementation PR description or a short design
note before agents proceed:

- `ModuleSyntax` has associated `Instruction`, `Value`, `Type`, and `Header`
  types; `Header` implements `SstHeader`.
- `ParsedModule<S>` contains `S::Header` and `Vec<ParsedFunction<S>>`.
- `ParsedFunction<S>` obtains parameter/return types and body instructions from
  `S`.
- `Resolve<S>` consumes the complete parsed module, including its header.
- component syntax is optional; runtime-only components remain valid;
- headers are composite-owned and are resolved before program installation;
- the new loading names are `LoadSstProgram` and `LoadSstSubtree`;
- resolved header metadata is assigned through an explicit builder operation
  if it belongs in the installed module `Info`.

**Gate:** `cargo check --workspace` on the baseline and a written list of
files each parallel agent owns.

## Step 1 — Shared syntax contracts and data model

**Agent A — `vihaco-syntax` contract owner**

Implement only the foundational public model and its unit tests:

- add and export `ModuleSyntax`;
- change `ParsedModule<I, Ty, H>` to `ParsedModule<S>`;
- change `ParsedFunction<I, Ty>` and `Param<Ty>` to use the module dialect;
- update the `Parse` implementations and `parse_section` to derive all
  syntax types from `S`;
- update `Resolve<S>` to accept `ParsedModule<S>`;
- preserve parsed headers as source syntax, distinct from runtime metadata;
- migrate the existing `vihaco-syntax` tests to a small test dialect marker.

Do not implement composite macro generation in this step. Keep the migration
mechanically useful for standalone consumers.

**Deliverable:** `vihaco-syntax` compiles with no old generic model in its
public API; focused syntax tests pass.

## Step 2 — Component instruction-set syntax contract

**Agent B — component/parser API owner**

Define the optional component-side syntax product and test it independently of
composites:

- add `InstructionSet` with surface `Instruction`, `Value`, and `Type`
  associated types;
- establish the required bounds for instruction parsing and the
  `SurfaceInstruction` marker;
- expose the contract through the appropriate facade crates;
- add a representative component syntax declaration or test fixture using the
  existing pattern parser;
- verify a component can provide local instruction/value/type parsers without
  knowing its mounted alias, device code, composite, or runtime route;
- verify a runtime-only component needs no syntax implementation.

If the declarative component `syntax {}` block is not yet implemented, do not
expand the macro grammar in this work package. Document the exact input shape
that Agent E will consume and leave macro parsing/codegen to that agent.

**Deliverable:** a stable component syntax contract and parser-level tests.

## Step 3 — Loader capability rename and compatibility migration

**Agent C — `vihaco-module` loader owner**

Rename the loading capabilities to match the program/subtree model:

- `LoadOwnSstSection` → `LoadSstProgram`;
- `LoadSstSection` → `LoadSstSubtree`;
- update docs, facade re-exports, trait bounds, and existing tests;
- keep the semantic distinction explicit: own-program loading first, then
  recursive child forwarding;
- decide whether a temporary deprecated alias is needed for a staged migration;
  if not, update all in-repository consumers in this step.

Do not change generated composite behavior beyond the trait names. Do not add
the new module-dialect bounds here; that belongs to the loading integration
step.

**Deliverable:** loader traits have the final names and the existing child
forwarding/install tests still pass.

## Step 4 — Composite source-sum generation

**Agent D — `vihaco-runtime-derive/src/composite` codegen owner**

Using the contracts from Steps 1–2, generate the complete composite syntax
product:

- a namespaced `syntax::Module` implementing `ModuleSyntax`;
- `syntax::Instruction`, `syntax::Value`, and `syntax::Type` sum enums for
  participating `#[syntax]`/device contributions;
- parser implementations that delegate after resolving the composite-owned
  public namespace or alias;
- support for mounting one component syntax more than once under different
  aliases;
- preserve explicit enum wrapping so duplicate local spellings remain
  diagnosable rather than silently merged;
- retain composite-only syntax hooks where the current macro already supports
  them.

The generated runtime instruction enum remains separate from the surface
instruction enum. Do not make runtime products implement source parsing by
default.

Add compile-pass coverage for one component, two components, aliases, and a
runtime-only device. Add compile-fail coverage for missing syntax metadata and
ambiguous/invalid namespace declarations if those diagnostics are part of the
chosen API.

**Deliverable:** a composite can expose a complete generated source dialect,
but loading need not use it yet.

## Step 5 — Composite-owned header syntax and resolution contract

**Agent E — header/metadata owner**

Implement the composite-side header boundary, keeping it separate from
component instruction sets:

- add the syntax declaration/input needed for a composite-owned header block;
- generate the header type/parse hook and the public syntax-resolver method;
- ensure a header can configure multiple devices or machine-wide metadata;
- resolve the header before lowering/installing instructions;
- make failures use the composite error at the resolver boundary and gain
  section/function/instruction context at the outer `eyre::Result` boundary;
- add `BuildProgramModule::set_info` (or the selected equivalent) only if the
  chosen metadata flow requires it, with a standard `ProgramImage` behavior;
- test that resolved header metadata survives installation and that invalid
  headers do not partially install a program.

Header parsing must produce source syntax. It must not directly expose or
mutate arbitrary live child-device state during module parsing.

**Deliverable:** a composite-owned header can be parsed, resolved, and carried
  into installed module metadata when required.

## Step 6 — Generated resolver and SST loading integration

**Agent F — integration owner; starts after Steps 1, 3, and 4**

Update generated loading to use the complete dialect and resolver pipeline:

- parse `ParsedModule<Composite::syntax::Module>` with no caller-supplied
  instruction/type/header generic parameters;
- make `load_parsed` resolve the header rather than discarding it;
- lower each surface instruction into `Vec<RuntimeInstruction>` so one-to-one
  and one-to-many expansion are both supported;
- assign final runtime instruction addresses before resolving labels/source
  symbols that refer to expanded code;
- build a temporary module and install it only after parsing, header
  resolution, lowering, and validation succeed;
- use the renamed `LoadSstProgram`/`LoadSstSubtree` traits;
- retain source function and instruction context when a named lowerer fails;
- avoid requiring capabilities (strings, constants, bytecode, or metadata)
  that a particular program container does not use.

The generated resolver trait should contain the header resolver and named
lowerers. Direct instruction mappings remain generated dispatch and do not
create unnecessary user methods.

**Deliverable:** `load_source` and `load_parsed` use one composite module
dialect end-to-end and preserve one-shot installation semantics.

## Step 7 — Nested composite subtree loading

**Agent G — recursive loading owner; starts after Steps 3 and 6**

Complete recursive loading without leaking child syntax types into the parent:

- generated composites load their own `#[program]` section through
  `LoadSstProgram`;
- they then forward each direct child through `LoadSstSubtree`;
- leaf devices implement `LoadSstSubtree` directly;
- each nested composite parses with its own generated
  `Child::syntax::Module`;
- parents require only the child subtree capability and do not name child
  parser enums or headers;
- validate expected child names and reject duplicates/missing sections with
  useful section context.

Add a nested fixture where parent and child deliberately use different
instruction, type, and header syntax. Confirm child loading happens only after
the parent has accepted its own program section and that a failure leaves the
parent program uninstalled.

**Deliverable:** recursive SST loading works with independent nested dialects.

## Step 8 — Semantic analysis and program-backed resolution

**Agent H — resolver semantics owner; starts after Step 6**

Add the semantic cases needed by the new source sums:

- resolve labels against final runtime addresses;
- resolve source types and report type mismatches clearly;
- resolve constants, strings, and source symbols through the program/context
  capabilities actually required by the composite;
- support composite policy such as source sugar and one-to-many expansion;
- keep live machine-state decisions in runtime message resolution, not source
  resolution;
- add diagnostics containing function and instruction location where the
  lowerer reports an error.

Use broad value operands where semantic errors are preferable; use narrower
value/type syntax only where parser rejection is intentionally part of the
language contract.

**Deliverable:** representative labels, types, constants, source symbols, and
one-to-many lowering are tested through `load_parsed`.

## Step 9 — Integration and regression coverage

**Agent I — test owner; starts after Steps 6–8**

Add coverage across crate boundaries:

- component instruction-set parsing;
- generated instruction/value/type source sums and aliases;
- `ModuleSyntax` parsing and `Resolve<S>`;
- header resolution, invalid-header diagnostics, and metadata installation;
- custom program containers implementing the minimum builder/install traits;
- one-to-many lowering and final label addresses;
- source-location/function/instruction error context;
- independent nested composite dialects and subtree loading;
- runtime-only components mounted beside syntax-bearing components.

Update trybuild `.stderr` fixtures only when diagnostics intentionally change.
Run workspace tests, doctests, clippy, and format as applicable. License checks
remain part of the final coordinator gate.

## Step 10 — Demo, guide, and cross-plan cleanup

**Agent J — migration/docs owner; starts after Step 9**

Migrate the demo and documentation to the final API:

- give demo components optional local instruction/value/type syntax;
- load the composite program through its generated module dialect;
- resolve a composite-owned header before installation;
- forward a nested debug/observer section through `LoadSstSubtree`;
- update parser and composite guides from `ParsedModule<I, Ty, H>` and
  `Resolve<I, Ty, H>` to `ParsedModule<S>` and `Resolve<S>`;
- update examples and any companion vision docs that still describe the old
  loader names or independent syntax parameters.

**Deliverable:** demo, docs, and doctests describe the same API that the tests
exercise.

## Coordinator integration gates

After each convergence point, the coordinator owns conflict resolution and
API consistency:

### Gate 1 — after Steps 1–3

Run `cargo fmt --all -- --check`, `cargo check --workspace`, and focused syntax,
loader, and existing macro tests. Confirm all public re-exports use one set of
names and no old generic API remains accidentally exposed.

### Gate 2 — after Steps 4–6

Run parser derive tests, runtime macro compile tests, generated SST loading
tests, and `cargo test --workspace --all-targets`. Inspect generated code for
the no-partial-install guarantee and for correct error conversion.

### Gate 3 — after Steps 7–8

Run nested loading, metadata, label, and semantic diagnostics tests. Confirm
that parent composites do not depend on child source types and that final
addresses are based on lowered runtime instructions.

### Final gate

Run the repository checklist:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --doc
hawkeye check
```

## Compatibility and migration policy

This is a deliberate public API migration. The preferred end state removes the
old `ParsedModule<I, Ty, H>`, `Resolve<I, Ty, H>`, `LoadOwnSstSection`, and
`LoadSstSection` names. If an intermediate commit needs compatibility aliases,
they must be clearly deprecated and removed before the final integration gate;
the generated macro API and documentation must use only the new names.

## Non-goals

This plan does not add:

- bytecode loading or encoding;
- mandatory parsers for runtime-only components;
- a universal lookup API for strings or constants;
- generated effect-handler traits;
- runtime route selection from arbitrary live device state during source
  resolution;
- compatibility with old `head`-based parser declarations;
- generated scheduling, resume, or continuation policy.
