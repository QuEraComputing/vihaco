# VM declarations through ordinary Rust types

> **Status:** Steps 1–7 approved; Steps 2–7 committed; Step 8 implemented, pending review.
> **Updated:** 2026-10-02.
> **Feature:** nondefault `typed-declarations`; no compatibility layer.
> **Scope:** `#[dialect]`, `#[instruction]`, `#[component]`, `#[composite]`,
> `#[instruction_set]`, and the small shared runtime contracts they need.

This plan replaces the namespace-inference, exported-inventory-macro, and
companion-type-tree architecture in `../rework/`. Keep that directory as a
historical record. This directory is the proposed specification for the next
implementation, subject to its contract checkpoint.

The public API describes a VM: dialects, instructions, components, device
fields, loadable sections, and executable instruction sets. Ordinary Rust
imports, aliases, visibility, and generic bounds keep their ordinary meaning.
The library supplies the associations between those concepts.

Implementation should be smaller as well. Each macro parses one declaration,
validates the facts it can see, and emits ordinary Rust items once. The compiler
resolves external types through traits and associated types. Complete capability
descriptions are associated constant data, not proc-macro callback responses.
No inventory transport protocol, exporter ID, inferred companion namespace,
provider tree, or canonical selector tree is needed.

## Public declaration shape

The core spelling to prove and freeze in [Step 1](01-contracts-and-baseline.md)
is:

```rust,ignore
#[dialect]
pub mod arithmetic {
    #[instruction]
    pub struct Add(pub i64);
}

#[dialect]
pub mod control {
    #[instruction]
    pub struct Halt;
}

#[component]
#[instructions { arithmetic, control }]
pub struct Cpu<T> {
    // Component state.
}

#[composite]
pub struct Controller {
    #[device(42)]
    cpu: Cpu<u8>,
}

#[composite]
pub struct Machine {
    #[device(7, alias = "ctl")]
    controller: Controller,
}

#[instruction_set(composite = Machine)]
pub enum MachineInstruction {
    #[target(controller::cpu)]
    Halt(control::Halt),

    #[target(controller::cpu)]
    Add(arithmetic::Add),
}
```

The dialect attribute consumes `#[instruction]` markers on unrestricted `pub`
unit/tuple structs
in an inline Rust module. Those structs are the actual runtime instruction
types; their module items, imports, visibility, and operand paths retain normal
Rust meaning. Paired surface types are defined in that scope and publicly
reexported as `dialect::syntax::InstructionName`. Unannotated bindings use the
authored field types. As amended by the user during Step 8, `$0:SourceType` pattern annotations
select distinct syntax field types while the authored struct declares
executable field types.
Resolution remains explicit program/composite logic, with no implicit conversion.
Complete coverage metadata is generated.

A set's paired surface enum is available as `syntax::MachineInstruction`, using
the authored runtime enum's name. As with dialect leaves, the actual hidden
surface definition remains beside the runtime type and `syntax` reexports it,
so payload and framework-root paths keep their authored scope. Sets are authored
at module scope; use separate authored modules for multiple sets.

Every device requires an explicit u8 code, unique within its parent composite.
Eligible device fields use that code directly as their Field key and hidden
field-name constant value. Reordering fields does not change keys; capability
reports still follow declaration order. Loadable devices require codes too but
remain excluded from parent Field traversal.

The target names actual device fields. The payload names the actual instruction
type in the author's Rust scope. Imported renames and type aliases do not
change the declaration. Users never provide `inventory = ...`,
`inventory_id`, helper-module paths, or generated type routes.

Components declare complete dialects. Every listed instruction requires
execution coverage. Composites expose complete eligible child capabilities,
independently of a child's executable instruction set. An instruction set
selects individual instructions and destinations; a loadable device field stops
parent selection and lifting across its section boundary.

The base system preserves explicit operand resolution and execution through
ordinary Rust and existing runtime contracts. Later execution features can build
on these declarations without changing availability or making additional
machinery mandatory. This plan implements the core declaration system and
migrates the shipped CPU to use it.

## Experimental feature and stabilization

New APIs and supporting runtime contracts are available only with the nondefault
`typed-declarations` feature, forwarded through facade/runtime/derive. Feature-off
use preserves the current API. Feature-on use selects the new declaration
contract, including the new composite emitter once Step 7 is complete.
Cargo feature unification can select that contract indirectly; test isolated
feature-off/on graphs and a graph that enables it through another dependency.
No legacy macro spelling, wrapper, or adapter is introduced. Step 8 migrates
`vihaco-cpu` and its feature-on consumers before this plan is complete. CPU must
work in both feature modes and under indirect feature activation. After
stabilization, migrate remaining consumers and remove the old implementations
and feature gate.

## Required CPU migration

The 2026-10-02 scope amendment requires actual migration of the shipped CPU in
[Step 8](08-integration-and-diagnostics.md), within PR 5. Auditing or
recording a future CPU migration is insufficient: users must be able to depend
on `vihaco-cpu` when `typed-declarations` is enabled. Verify public parsing,
resolution, selected execution, control-flow/effects, encoded instruction and
section semantics, and indirect Cargo feature activation. A feature-on example
must use the real CPU component. Reuse the `impl Execute<...> for CPU` blocks
from `../vihaco-execute-bench` selectively, adapting them to the current contract.
Keep main's `CPU::stack: Vec<Word>` unchanged in both modes and replace all unsafe
benchmark operations with main's safe checked operations. Do not port the
benchmark stack wrapper or separate stack pointer; Step 8 records the source
worktree and main baseline explicitly.

Step 8 adds a nondefault CPU feature forwarding to the facade/runtime and gates
legacy/typed CPU versions with `cfg(not(feature = "typed-declarations"))` and
`cfg(feature = "typed-declarations")`, respectively. Public exports and
mode-specific tests follow the same flag. Coherent feature-on CPU graphs enable
`vihaco-cpu/typed-declarations` directly or through another dependency: enabling
only a dependency's feature does not enable CPU's local cfg flag in reverse.

Step 7's temporary CPU exclusion ends in Step 8. Both Step 8 and final Step 9
feature-on gates must include CPU; legacy documentation callers
that remain unmigrated require their own documented feature-off graph. Keep the
feature nondefault. Completing this migration does not authorize feature-gate
removal, compatibility APIs, or a change to reviewed opcode/section semantics.

## Source references and branch context

The implementation checkout is `rob/types-and-dialect-declarations`, at
`main` revision `cf70ca8e2f27a844329cbe24542873bf648204de` after the user's
2026-10-01 reset. The tracked worktree is clean.

Prior preparation remains on `rob/shared-inventories-and-dialect`: approved
`fbfec16` followed by WIP `7d36633`. That WIP contains the old Step 3
options/root changes, path inference/rebasing, and tests. Both are reference
history, not ancestors or completed steps of the new branch. Preserve that
branch; do not cherry-pick its whole patch. Port only useful behaviors in their
owning steps, following the [Step 1 classification](step-1-results.md#classification-of-the-prior-work).

The preserved behavioral reference is `rob/inventory-test` at
`2b87ad69e0a312c62c2157b6317b48b386215cdd`. Read it through a separate
checkout/archive when comparing behavior. It is not the new branch base and
its hidden protocols are not requirements.

Links to `../../crates/` describe current repository files. Proposed paths
describe files to create in their owning step. Check the actual base at the
start of each step rather than assuming reference-only modules exist.

## Table of contents and implementation order

| Step | Plan | Result | Depends on |
| --- | --- | --- | --- |
| 1 | [Contracts and baseline](01-contracts-and-baseline.md) | [Approved no-commit checkpoint](step-1-results.md). | — |
| 2 | [Runtime associations and validation](02-runtime-associations-and-validation.md) | [Committed as 072166e](step-2-results.md). | 1 |
| 3 | [Dialect declarations](03-dialect.md) | [Committed as c3945f7](step-3-results.md). | 2 |
| 4 | [Whole-dialect components](04-components.md) | [Committed as a849e04](step-4-results.md). | 2–3 |
| 5 | [Composite fields and capabilities](05-composites.md) | [Committed as 7f63618](step-5-results.md). | 2, 4 |
| 6 | [Authored instruction sets](06-instruction-sets.md) | [Committed as 7e33c88](step-6-results.md). | 3–5 |
| 7 | [Selected surface parsing](07-selected-parsers.md) | [Committed as c38b7c8](step-7-results.md). | 6 |
| 8 | [CPU migration, integration, diagnostics, and editor behavior](08-integration-and-diagnostics.md) | Cfg-gated CPU migration plus cross-crate, generic, diagnostic, and editor evidence. | 3–7 |
| 9 | [Migration, cleanup, and release gates](09-migration-cleanup-and-release-gates.md) | Preserve migrated CPU support, finish callers/docs, cleanup, final gates. | 8 |

There are nine step review units. Step 1 is expected to change only local
design/probes and has no commit. Steps 2–9 are eight expected implementation
commits. The already approved `fbfec16` remains separate history on the prior
branch; it is not inherited or a completed step of this replacement plan.

PR 3 may contain tested internal composite preparation. Keep the existing
default public composite behavior working; PR 4 can activate the complete new
feature-gated composite/instruction-set/parser contract. Do not expose an attribute that
depends on a future parser or placeholder generator.

## Per-file review and commit workflow

Never stage or commit any file under `design/` at any point in this plan.
This includes plans, contract records, probes, fixtures, and results. Exclude
them from every implementation commit and PR. Do not move local planning
artifacts elsewhere or create empty commits to satisfy a boundary.

Each numbered file is one work and review unit, not one Rust source file.

1. Complete that file's implementation tasks, affected callers/documentation,
   focused checks, and exit criterion. Keep the workspace buildable.
2. Run the repository gates below before any implementation commit. Inspect
   changed diagnostic fixtures and the complete eligible diff.
3. Report the outcome, changed behavior/files, checks, limits, and proposed
   conventional commit message. For design-only work, report a no-commit
   checkpoint. Stop for user review.
4. Address feedback and rerun affected checks. Commit eligible changes only
   after explicit user approval. Review completion alone is not commit approval.
5. Report the commit hash, or record the approved no-commit checkpoint, before
   starting the next numbered file.

Do not combine step commits or squash them without explicit approval.
A PR does not replace its individual step checkpoints. If a step unexpectedly
has no eligible changes, report that fact; do not manufacture a commit.
Writing this replacement plan does not authorize implementation or a commit.

## Pull request structure

| PR | Main idea | Steps | Expected new step commits | Depends on |
| --- | --- | --- | --- | --- |
| 1 | Typed instruction relationships and dialects | 1–3 | 2 | `main` |
| 2 | Complete component capabilities | 4 | 1 | PR 1 |
| 3 | Typed composite fields and complete capabilities | 5 | 1 | PR 2 |
| 4 | Authored instruction sets and selected parsing | 6–7 | 2 | PR 3 |
| 5 | Integration, migration, and cleanup | 8–9 | 2 | PR 4 |

See [the PR plan](pull-request-plan.md) for branches, scope, activation, and
merge criteria. Create draft PRs when their actual changes are reviewable,
update descriptions to the final scope, and merge in dependency order.
Do not publish empty scaffolding PRs or include design artifacts.

## Required checks

Before every eligible implementation commit:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --doc
mise exec -- hawkeye check
```

Run affected feature configurations independently, including facade and direct
runtime use, `typed-declarations` off/on, and indirect feature unification.
Run on the repository's current stable compiler; record the exact
version. Rust 1.85 is the edition minimum, not an additional promised MSRV.
Do not silently raise the parser crates' explicit Rust 1.75 requirement.

Do not add custom test harnesses that write generated source or build artifacts,
invoke Cargo/rustc subprocesses, or execute generated fixture binaries. This
includes rustc-stdin compilation and nested Cargo feature-graph runners. Use
in-process parsing/emission assertions, ordinary unit/integration tests and
doctests, and established trybuild fixtures for compiler diagnostics. Run
independent feature graphs and checked-in fixture crates as top-level
verification commands or CI jobs, never from inside a test. Do not reintroduce
the removed compilation runners in a later step.
Proc-macro diagnostic changes need reviewed trybuild snapshots.
Check actual values/types/order, not pretty-printer spacing.

## Implementation responsibilities

| Layer | Owns | Does not need |
| --- | --- | --- |
| Runtime support | Instruction/dialect associations, coverage traversal, typed fields, static descriptions and validation | Rust source discovery or a token protocol |
| Dialect attribute | Marked runtime structs, pairing, grammar metadata, complete ordered leaves | Exported inventory macros |
| Component macro | Dialect declarations and coverage under original bounds | Per-leaf reflection or tree mounts |
| Composite macro | Device fields, child types, aliases, section boundaries | Child module-name inference |
| Instruction-set macro | Authored choices, exact target/type checks, paired enums | Replacing payload paths with generated selectors |
| Parser emission | Associated leaf parsers and target source prefixes | Complete child token enumeration |

Metadata and helper traits may need cross-crate visibility. Keep them under one
document-hidden support surface; they are generated implementation contracts,
not user-authored inventory APIs. `Execute<I>` is different: it expresses
component behavior and is a documented VM interface.

## End-of-file checkpoint

This is the replacement plan's overview, not a completed implementation step.
Step 4 is committed as a849e04, with replacement-feature correction 10d5b88.
Step 5 is committed as 7f63618; Step 6 is approved and committed as 7e33c88. Step 7 is approved and committed as c38b7c8; Step 8 is implemented and awaiting review; see [its results](step-8-results.md). No new commit, branch, push, or PR
is authorized merely by creating these plans. Never stage or commit anything
under `design/`; preserve the prior approved/WIP branch and unrelated user work.
