# Step 11: diagnostics, cross-crate use, and editor behavior

**PR:** [PR 5: final migration and integration closure](pull-request-plan.md#pr-5-final-migration-and-integration-closure).

**Review/commit:** Complete this file's work and report it for user review. If
there are eligible changes outside `design/`, wait for explicit approval before
its dedicated commit. If the step changes only `design/`, record a no-commit
checkpoint and wait for review approval before starting the next step. See
[workflow](README.md#per-file-review-and-commit-workflow).

**Commit exclusion:** Never stage or commit any file under `design/` at any
point in this plan, including plans, contract records, probes, fixtures, and
results. Exclude them from every implementation commit and PR. Do not move
local planning artifacts elsewhere or create an empty commit merely to satisfy
a step's commit boundary.

**Source references:** `rob/inventory-test`; see [branch context](README.md#source-references-and-branch-context).

**Prerequisites:** [Steps 5–10](README.md#table-of-contents-and-implementation-order).
**Next:** [migration and release gates](12-migration-cleanup-and-release-gates.md).

## Outcome

Establish that the reimplementation preserves the authored API through nested
macro expansion, crate boundaries, generic bounds, and editor operations.
Consolidate tests around semantic outcomes and review diagnostic changes.

This step closes integration gaps; it does not defer basic macro tests from
earlier steps. Every producer and consumer should already have focused coverage.
Apply the relevant cross-crate, diagnostic, and editor checks in PRs 1–4 as their
macros change. PR 5 owns the final combined audit and remaining evidence, not the
first validation of those earlier changes.

## Cross-crate fixture

Extend [inventory-producer](../../crates/vihaco/tests/support/inventory-producer/src/lib.rs)
and the existing consumer tests rather than inventing a disconnected protocol
mock as the only cross-crate proof.

Use a chain with a dialect and component in one crate, an intermediate composite
in a second, and a consuming composite/instruction set in a third when needed
to establish recursive re-export. The intermediate must consume and publish
the real protocol; direct downstream access to the original dialect is not a
substitute for that handoff.

Cover:

- facade and direct `vihaco-runtime` dependency roots;
- renamed Cargo dependency and a custom re-exported root;
- local/imported dialect aliases, same-named disjoint dialects, custom IDs;
- parent fields using imported type names and explicit inventory overrides;
- nested canonical selectors with mixed source aliases;
- selected runtime and surface payload identity through several `pub use` levels;
- generic components/composites with qualified bounds and concrete targets;
- complete dialect coverage, including a newly added leaf omitted by every
  downstream authored instruction set;
- a child authored set that omits a capability the parent selects;
- a loadable child that is delegated but not lifted.

Run the direct-runtime fixture independently with explicit feature flags.
Workspace feature unification can otherwise mask a missing runtime feature or
hidden finalizer export.

## Diagnostic responsibilities

| Stage | Errors it should own |
| --- | --- |
| Authored parsing | Wrong item shape, malformed selectors/attributes/options, duplicate helper attributes, unsupported conditional membership. |
| Dialect validation | Duplicate leaf names and well-formed mnemonic collisions. |
| Shared protocol/model | Malformed entry fields/routes/source steps, duplicate exported selectors, trailing data. |
| Component dialect validation | Duplicate dialect membership, invalid dialect provider shape, and logical selector collisions across complete inventories. |
| Rust coverage/coherence | Missing or overly narrow `Execute` impls and semantic duplicate runtime types. |
| Composite validation | Duplicate codes/source symbols/sections, invalid field/provider inference. |
| Instruction-set selection | Duplicate choices and full paths absent from inventory. |
| Rust name resolution/type identity | Missing/inaccessible namespaces, generated name collisions, selector/tree mismatch. |
| Parser derive | Invalid leaf patterns and operand bindings. |

Attach errors to authored spans wherever tokens retain them. For a duplicate,
prefer the later occurrence and add the first location where useful. Combine
independent syntax errors when it makes the result clearer; do not accumulate
downstream errors from data already known to be invalid.

Do not promise precise “loadable versus omitted dialect versus unknown” diagnoses
without the necessary protocol metadata. Missing namespace errors can remain
ordinary Rust resolution errors; inference failures should explain the
`inventory = path` remedy at the authored attribute.

## Compile-fail review

Use the existing four harnesses and their `compile_fail` directories. Add the
missing cases identified in Step 1 and review the entire affected stderr diff.
Where first-expansion Rust references and finalizer validation both diagnose
one bad path, decide whether the extra message adds useful evidence or is
avoidable duplication; preserve editor references that have a demonstrated use.

Use `TRYBUILD=overwrite` only for the chosen harness and only after deciding
which diagnostic change is intended. Do not bulk-refresh unrelated fixtures.
Check hidden-finalizer malformed input explicitly so proc-macro panics do not
become the accidental error contract.

## Editor acceptance probe

The [type-tree document](../instruction-inventory-type-tree.md) reports useful
results for rust-analyzer 0.3.3065. Reproduce the operations on the reimplementation
with the actual tool version recorded; those historical results do not prove
new tree placement preserves them.

Use one small checked-in fixture for:

1. Completion/navigation on component dialect module paths and on an instruction-set
   selector’s field/dialect/leaf segments.
2. Hover on the leaf of `stream_a::core::cpu::ConstI64` showing the concrete
   dialect struct, with prefixes recognized as modules.
3. Go-to-definition from the authored leaf to its dialect declaration token.
4. The same operations through an imported dialect alias, two same-named
   dialects, a cross-crate producer, and two composite levels.
5. Variant/enum hover and missing-match-arm suggestions using the authored
   runtime enum name and module path.
6. Partially typed dialect paths and instruction-set selectors. Every generated
   component tree leaf must be inventory-available; instruction-set completion
   can offer available leaves beyond those already declared as variants.

Record the fixture position, editor operation, result, and tool version. If
automation is available, keep a small reproducible probe; otherwise record a
manual procedure. If an editor cannot be run, explicitly mark that check
unverified rather than equating compiler success with editor acceptance.

Hidden paths may appear in variant hover. Preserve the concrete leaf identity
and authored enum path; do not promise exact authored payload spelling if the
editor shows the selector-tree route. Revisit Step 1’s canonical alias merge
decision if that probe regresses.

## Expansion inspection and optional measurements

Inspect one representative nested expansion. Verify that there are no per-entry
lift wrappers, no per-name producer selection arms, no duplicate protocol
renderers, and no macro-global registry or nondeterministic ID generation.
Keep a small illustrative expansion rather than enormous whitespace snapshots.

If fewer generated definitions or faster editor/build behavior is used as a
reason for a change, measure the same fixtures before and after under matching
toolchains and features. Callback traffic carries accumulated inventories and
is not automatically linear in total output size. Do not claim a speedup from
source line count or module count alone.

## Exit criterion

The complete chain works through public roots and across crates, the affected
diagnostics are intentional, and the editor probe results or explicit verification
limits are recorded. The evidence distinguishes compiler behavior, parser
behavior, and editor behavior rather than letting one stand in for the others.
