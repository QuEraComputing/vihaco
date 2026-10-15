# Macro and inventory reimplementation

> **Status:** Step 1 accepted without a commit; Step 2 committed; Step 3 remains uncommitted while its architecture is reconsidered.
> **Reviewed:** 2026-09-30, including the uncommitted type-tree experiment.
> **Reference implementation:** `rob/inventory-test`.
> **Implementation base:** `main`, followed by the five stacked PRs below.
> **Scope:** `dialect!`, `#[component]`, `#[composite]`,
> `#[instruction_set]`, and their shared compile-time inventory infrastructure.

The user's subsequent review requires users to work only with VM concepts,
without inventory namespace overrides, exporter IDs, or knowledge of generated
type trees. The [encapsulation research](encapsulation-research.md) recommends
actual Rust instruction payloads, semantic field targets, and trait-associated
capabilities. The user also requires static-topology work to extend this base
system additively, preserving complete eligible child availability and ordinary
field-target selection. Routes and advertised route entries must not become
mandatory or replace that availability contract.
This is an explored replacement, not an accepted plan revision or
implemented API. Steps 3 onward still describe the earlier architecture and
must be reconciled before implementation continues. Step 3 has no commit
approval. All research and plan files remain excluded from commits and PRs.

This plan keeps the design reached by the draft and rebuilds its internals
around explicit models, shared path and tree operations, and small expansion
stages. Here, “component” means the attribute macro. The older `component!`
macro is a migration dependency, not another macro to redesign in this work.

Components declare complete dialects with `#[instructions { arith, boo, foo }]`.
Every instruction in every listed dialect requires an `Execute<I>` implementation
and is exported by the component. This contract supersedes partial component
selection in the draft and earlier design documents. Instruction-set enums still
choose individual routes from a composite's complete available inventory.

The supported spelling is the brace-list form above. Step 1's Rust 1.96 probe
found that an outer consuming macro can receive both that form and the
illustrative `#[instructions = { arith, boo, foo }]` form; the latter remains
unsupported by the rework contract. See the [contract record](contract.md).

## Source references and branch context

All source/test paths, links, and descriptions of the “current implementation”
in this directory refer to the draft on **`rob/inventory-test`**. Read that branch
when inspecting the code being discussed. Proposed file layouts refer to the
new implementation that the PR stack will create.

The planning review observed HEAD
`3a18b0f78f15b34b0293da53f605be29e1ddb125` (`WIP`) plus pending type-tree
changes. Those source changes and fixtures are now preserved in reference
commit `2b87ad69e0a312c62c2157b6317b48b386215cdd` on `rob/inventory-test`.
Step 1 reads that revision through isolated archives; the untracked historical
design documents remain in the implementation checkout. The pinned revision
and baseline evidence are recorded in [contract.md](contract.md).

The reimplementation starts from **`main`**, not the draft branch. At this
review, local `main` is `215704b995ae9d2000ad0d2c566542d8927585e5` and lacks the
draft's `inventory/`, `dialect/`, `component_attribute/`, `composite_inventory/`,
and `instruction_set/` modules. These source links therefore describe code to
learn from; they do not establish that those files already exist on the stack's
base. Implement each macro in its owning PR and migrate the callers actually
present on that PR's base. Use the then-current `main` when starting the stack;
the hash above records the inspected snapshot rather than pinning the new work.
Implementation began on `rob/shared-inventories-and-dialect` at then-current
`main`, `cf70ca8e2f27a844329cbe24542873bf648204de`. Step 1's
[contract and probes](contract.md) were accepted at a no-commit checkpoint.
Step 2's [implementation checkpoint](02-inventory-model-and-protocol.md#implementation-checkpoint)
was approved and committed as `fbfec16` (implementation files only).
Step 3's [implementation checkpoint](03-paths-names-and-options.md#implementation-checkpoint)
is ready for review; its commit has not been approved.

## Table of contents and implementation order

Implement the numbered steps in order. Each file defines a work step with
prerequisites, implementation tasks, tests, and an exit criterion. Complete,
and review each numbered file's work separately using the workflow below.
Commit only eligible changes outside `design/`. Step 1 is a no-commit review
checkpoint; the remaining eleven implementation commits form the five PRs below.
Steps 6–7 share PR 2 and
Steps 9–10 share PR 4, so each merged PR provides a complete macro implementation.

| Step | Plan | Result | Depends on |
| --- | --- | --- | --- |
| 1 | [Contracts and baseline](01-contracts-and-baseline.md) | One current contract, behavior matrix, and small probes for unresolved cases. | — |
| 2 | [Inventory model and protocol](02-inventory-model-and-protocol.md) | Validated inventory data, structural identities, and one token grammar. | 1 |
| 3 | [Paths, names, and options](03-paths-names-and-options.md) | Shared namespace discovery, scope rebasing, and generated naming. | 1–2 |
| 4 | [Transport and type trees](04-transport-and-type-trees.md) | Shared passive exporters, callback collection, and tree emission. | 2–3 |
| 5 | [`dialect!`](05-dialect.md) | Leaf types, leaf parsers, and an ordered dialect inventory. | 1–4 |
| 6 | [Component input and first expansion](06-component-input-and-first-expansion.md) | Ordered dialect paths and immediate complete tree mounts. | 3–5 |
| 7 | [Component finalization](07-component-finalization.md) | Whole-dialect concatenation, uniform coverage, duplicate checks, and export. | 2, 4, 6 |
| 8 | [`#[composite]`](08-composite.md) | Recursive field lifting with intact section boundaries and loaders. | 3–4, 7 |
| 9 | [Instruction-set declarations and selection](09-instruction-set-declarations-and-selection.md) | Authored runtime enum, inventory validation, and paired surface variants. | 2–4, 8 |
| 10 | [Selected surface parsers](10-selected-surface-parsers.md) | Parsing exactly the selected entries and accepted field spellings. | 5, 9 |
| 11 | [Diagnostics, cross-crate use, and editor behavior](11-diagnostics-cross-crate-and-editor.md) | Evidence that the staged system works as a public API and in an editor. | 5–10 |
| 12 | [Migration, cleanup, and release gates](12-migration-cleanup-and-release-gates.md) | Coherent documentation, retired draft machinery, and final checks. | 11 |

The serial order follows the dependency graph: infrastructure → dialect →
component → composite → instruction set. Prepare fixtures and guides alongside
implementation within the active step. Wait at its review checkpoint before
starting the next step. Shared protocol and path changes should have one owner;
avoid rebuilding all four consumers against different temporary protocols.

## Per-file review and commit workflow

Never stage or commit any file under `design/` at any point in this plan.
This exclusion includes plans, contract records, probes, fixtures, and results;
none belongs in an implementation commit or PR. Do not move local planning
artifacts elsewhere or create an empty commit merely to satisfy a step boundary.

Each numbered plan file is one review unit. If it produces eligible changes
outside `design/`, those changes form its own dedicated commit: source, tests,
fixtures, and public documentation needed to complete the step. Individual Rust
source files are not separate commit units. A step that changes only `design/`
has a no-commit review checkpoint. Step 1 is such a step. The overview and PR
plan coordinate these units rather than adding implementation steps.

For every numbered file:

1. Complete its implementation tasks and exit criterion, including affected
   callers and documentation. Keep the workspace buildable at the commit boundary.
2. Run the step's checks and the repository's required pre-commit gates. Review
   the diff and any changed diagnostic fixtures.
3. Report back with the completed step, changed files and behavior, check results,
   any remaining limitations, and a proposed conventional commit message for
   eligible changes, or state that it is a no-commit checkpoint.
   Stop here so the user can review the completed work.
4. Address review feedback and rerun affected checks. Commit only after the user
   explicitly approves the reviewed work for committing. A completion report
   alone does not authorize a commit.
5. For eligible changes, create the dedicated step commit and report its hash.
   For design-only work, obtain review approval and record the no-commit
   checkpoint. Then proceed to the next numbered file unless directed otherwise.

Do not start the next step while review or commit approval is pending, or combine
multiple step files into one commit. Keep the step commits separate within their
PRs; combining or squashing them requires explicit user approval. If a step must
prepare a later stage, keep that preparation internal and testable until the
owning step activates the complete public API. Do not introduce placeholder
finalizers or temporary public syntax to manufacture a commit boundary.

## Pull request structure

Use five PRs organized around the major implementation ideas. The
[detailed PR plan](pull-request-plan.md) defines scope, dependencies, migration
ownership, and merge criteria for each.

| PR | Main idea | Primary steps | Separate step commits | Depends on |
| --- | --- | --- | --- | --- |
| 1 | Shared inventory infrastructure and the dialect producer | 1–5 | 4 (Steps 2–5; Step 1 review only) | `main` |
| 2 | Whole-dialect component contracts | 6–7 | 2 | PR 1 |
| 3 | Recursive composite inventory lifting and section boundaries | 8 | 1 | PR 2 |
| 4 | Authored instruction sets and selected surface parsing | 9–10 | 2 | PR 3 |
| 5 | Final migration, obsolete machinery, and integration closure | 11–12 | 2 | PR 4 |

PR 1 targets `main`. Each later PR is based on and targets its predecessor's
branch. The draft on `rob/inventory-test` remains the source reference throughout
the stack. See the [branch/target table](pull-request-plan.md#branch-bases-and-pr-targets).

Each PR includes its relevant tests, diagnostic fixtures, caller migration, and
documentation. Apply Step 11's integration/editor checks and Step 12's required
repository gates as each idea lands; PR 5 completes the remaining audit and any
bounded mode retirement. In particular, PR 2 owns migration from partial
component declarations to complete dialect lists throughout the workspace.

## Design baseline and precedence

The documents describe several successive implementations. Use their settled
semantics together with the current source; do not combine incompatible token
schemas or authored APIs.

| Existing document | What this plan takes from it | What has been superseded |
| --- | --- | --- |
| [Type-tree inventories](../instruction-inventory-type-tree.md) | First-expansion `pub use` trees, provider routes, selector-shaped tuple payloads, and editor evidence. | Optional partial component selection, historical “proposed” wording, and examples that say those trees are unavailable. |
| [Collector simplification](../instruction-inventory-simplification.md) | Keep the direct callback chain until measurements justify another transport. | Its `runtime`/`surface` alias schema predates `type_route`. |
| [Dialect/component plan](../dialect-component-instruction-inventory-plan.md) | Passive complete inventories, coverage, semantic duplicates, shared export, and no compatibility copy of the protocol. | Partial component selection, split explicit/wildcard coverage, `RuntimeN`/`SurfaceN` transport, and references to removed enumerator/coverage modules. |
| [Composite inventory plan](../composite-inventory-implementation-plan.md) | Recursive lifting, field aliases, namespace overrides, and loadable exclusion. | The off-by-default description: the facade currently enables inventory mode by default. |
| [Instruction-set plan](../instruction-set-implementation-plan.md) | Flat paired enums, selected leaf parsing, manual resolution/dispatch, and concrete generic targets. | `#[selector(...)]` plus a separately authored concrete payload. The working draft now takes the selector directly as the tuple payload. |
| [Structured inventories](../structured-instruction-inventories.md) | Availability versus executable selection, expansion stages, order, and no lift wrappers. | `version`, `origin`, per-entry aliases, and some stronger diagnostic promises. |
| [Composite lifting](../composite-instruction-lifting.md) | Route choice retained by authored variants; no dependency on generated dispatch. | Historical implementation descriptions and older selector/editor limitations. |
| [Crate split](../crate-split.md) | Macros remain in `vihaco-runtime-derive`; every generated root works through runtime and facade re-exports. | Its pre-split file inventory is historical. This rework does not reopen the crate split. |

These files are the proposed rework specification. Step 1 records decisions
where the historical documents leave alternatives. Implementation should
update that record if a probe changes a recommendation.

## Responsibilities and invariants

| Layer | Owns | Output |
| --- | --- | --- |
| Shared inventory | Data validation, protocol, ordered transformations, tree paths, export, collection. | Compile-time tokens and hidden Rust type trees. |
| Dialect | Leaf declaration, paired operand types, leaf grammar. | Runtime/surface structs and declaration-ordered entries. |
| Component | Whole-dialect membership and `Execute<I>` coverage for every leaf. | Every entry of each listed dialect in list/declaration order, without a component source prefix. |
| Composite | Field traversal and section boundaries. | Complete eligible child entries with field selectors, type routes, and source steps prepended. |
| Instruction set | Authored executable choices and their parser. | Runtime enum, matching surface enum, and `HasInstructionSet` association. |

Keep these identities separate:

- **Logical selector:** `stream_a::core::cpu::Add`; identifies an available
  route and always uses canonical names.
- **Type route:** `stream_a::core::provider_0::cpu::Add`; relative to a
  producer's runtime or surface tree, preserving provider provenance.
- **Source steps:** `stream_a`/`a`, then `core`/`c`; accepted parser spellings.
- **Authored variant:** `LoadA`; retains the execution choice even when another
  variant has the same leaf payload type.
- **Producer namespace:** the caller-visible module path used to collect an
  inventory and resolve its type routes.

Inventory order is semantic. Components concatenate dialects in authored list
order and preserve declaration order within each dialect; composites preserve
field order and child order. Do not sort or silently deduplicate entries.
For generated dialects and components, the runtime/surface trees and inventories
expose exactly the same leaves. A child’s authored executable set does not
restrict what its parent can lift from its complete available inventory.

Adding an instruction to a dialect adds a coverage obligation to every component
using that dialect. The component must implement it before compiling again.
This is the intended stronger contract; there is no per-instruction opt-out.

## Lessons from the current implementation

The staged handoff is useful and should survive the rewrite. A producer returns
its complete inventory once; the shared collector forwards it opaquely; a
procedural finalizer interprets it. A Rust type tree cannot enumerate available
members or encode source aliases by itself.

The current implementation also identifies concrete cleanup targets:

| Current location | Finding | Rework response |
| --- | --- | --- |
| [inventory/mod.rs](../../crates/vihaco-runtime-derive/src/inventory/mod.rs) | Model, grammar, transforms, path rebasing, exporter, and callback generation share one 513-line module. Selector keys use rendered token strings. | Separate responsibilities; use validated relative paths and structural keys. |
| [component_attribute/mod.rs](../../crates/vihaco-runtime-derive/src/component_attribute/mod.rs) | The draft parses leaf selections and emits explicit coverage checks; mounts use repeated rebasing; selector roots are grouped by the last authored dialect path segment. | Replace selection grammar with a dialect list and typed mounts; remove direct leaf coverage; probe canonical roots through import aliases. |
| [component_attribute/finalize.rs](../../crates/vihaco-runtime-derive/src/component_attribute/finalize.rs) | It forwards the whole struct context, selects named/wildcard entries, and rebases generic paths. | Concatenate complete dialect inventories; check every leaf through one coverage path; retain semantic duplicate checks and reduce context. |
| [composite_inventory/mod.rs](../../crates/vihaco-runtime-derive/src/composite_inventory/mod.rs) | Typed field information becomes token tags, then is reparsed to build first-expansion trees. Finalizer context includes unused identity/root fields. | Build mounts from the field model; serialize tags only at the handoff; forward only used context. |
| [lib.rs](../../crates/vihaco-runtime-derive/src/lib.rs) and [attr_composite.rs](../../crates/vihaco-runtime-derive/src/attr_composite.rs) | Composite input is parsed once to generate wiring and again to sanitize the struct. | One parse, one validated declaration, and one sanitized item. |
| [instruction_set/mod.rs](../../crates/vihaco-runtime-derive/src/instruction_set/mod.rs) | Parsing, declaration rewriting, selection, and parser generation are combined. A `HashMap` can overwrite duplicate exported selectors; surface path construction bypasses the shared runtime helper. | Validated lookup and one runtime/surface resolver; explicit selected-entry model. |
| [generated.rs](../../crates/vihaco-runtime/src/generated.rs) and Cargo manifests | Feature switches alter marker supertraits and composite output; facade and runtime defaults differ. | Preserve configurations during the rewrite; evaluate their retirement explicitly in Step 12. |

The alias grouping and duplicate-map observations are code-review findings.
The baseline suites below do not establish behavior for every combination of
same-named dialects and imported aliases. Add small reproductions in Step 1
before choosing a fix.

## Proposed source organization

Keep shared infrastructure private to the existing derive crate:

```text
vihaco-runtime-derive/src/
  common.rs                       root/options support shared with other macros
  inventory/
    mod.rs                        narrow internal exports
    model.rs                      entries, identities, ordered operations
    protocol.rs                   wire parsing and rendering
    paths.rs                      naming, inference, scope transformations
    trees.rs                      leaf re-exports and provider/field mounts
    export.rs                     one passive producer exporter
    collect.rs                    one direct callback-chain generator
    tests.rs                      shared behavioral tests
  dialect/{mod,parse,validation,emit}.rs
  component_attribute/{mod,parse,emit,finalize}.rs
  attr_composite/{fields,loaders}.rs
  composite_inventory/{mod,emit,finalize}.rs
  instruction_set/{mod,parse,validation,select,emit,parser,finalize}.rs
```

This is a responsibility map, not a requirement to create trivial one-function
files. Merge adjacent small modules when their boundaries add no value. Keep
proc-macro entry points thin and make the implementation operate on
`proc_macro2` tokens and `syn::Result`. Generated runtime fallible operations
continue to use the repository’s `eyre` conventions. A generic compiler
framework or new support crate is unnecessary unless a real second consumer
requires one.

## Evidence and validation policy

During this review, the following commands passed:

```sh
cargo test -p vihaco-runtime-derive --all-features
cargo test -p vihaco --test dialect_macro --test component_attribute \
  --test composite_inventory --test instruction_set --test instruction_set_many \
  --test inventory_cross_crate --test inventory_type_tree \
  --test dialect_macro_compile_fail --test component_attribute_compile_fail \
  --test composite_machine_compile_fail --test instruction_set_compile_fail
```

The first command ran 17 unit tests. The second covered eleven integration
test targets, including the four trybuild harnesses. This is a focused baseline,
not evidence of a complete workspace, feature, or editor validation run. The
rust-analyzer results in the type-tree design document are historical evidence;
this planning review did not rerun those probes.

Those passing suites characterize the existing draft's partial component API.
Migrate their declarations and coverage expectations to whole dialects; they
are not evidence that the new contract has already been implemented.

Run focused semantic tests as each step lands. Prefer compiler/type assertions,
actual callback expansion, and parser behavior over token substring counts.
Keep diagnostic fixtures line-sensitive and review changed `.stderr` files.
Useful integration milestones are shared infrastructure (Step 4), completed
dialect/component (Step 7), and the full selected parser chain (Step 10). Run
the full repository gates before implementation commits as required by
`AGENTS.md`, and finish with Step 12’s feature/docs checks. Measure build or
editor performance only if a change is being justified by performance.

Every PR must merge with the supported workspace configurations and doctests
passing. Preparation for a later step can remain internal, but no merged PR
should require a later PR to repair its public API, callers, or documentation.

## Scope boundaries and decisions

Require whole dialects for components and preserve selector-shaped instruction-set
payloads, provider-separated type routes, concrete generic composite targets, and
the direct collector chain. Keep bytecode opcode policy, generated routes/work,
blanket operand conversion, arbitrary parser endpoints, and new instruction
declaration shapes as separate work.

Three details need evidence before their implementation is fixed: canonical
selector-tree merging through import aliases; attribute propagation on
component/composite declarations; and the migration cost of removing feature
modes. Their probes and recommended defaults are in Steps 1, 6, 11, and 12.
No hidden protocol compatibility layer is proposed. Keep only one active
inventory grammar as shared code is replaced.
