# Step 4: complete component dialect capabilities

**PR:** [PR 2: complete component capabilities](pull-request-plan.md#pr-2-complete-component-capabilities).

**Review/commit:** Complete this file's work and report it for user review.
If there are eligible changes outside `design/`, wait for explicit approval
before its dedicated commit. If only design changes exist, record a no-commit
checkpoint and wait for review approval before the next step. See
[workflow](README.md#per-file-review-and-commit-workflow).

**Commit exclusion:** Never stage or commit any file under `design/` at any
point in this plan, including plans, contracts, probes, fixtures, and results.
Exclude them from every implementation commit and PR. Do not move local design
artifacts elsewhere or create an empty commit to satisfy a step boundary.

**Source references:** current checkout and pinned behavioral reference;
see [branch context](README.md#source-references-and-branch-context).
**Prerequisites:** Steps 2–3 committed; marker/migration policy settled.
**Next:** [Step 5](05-composites.md).

## Outcome

Implement feature-gated #[component] as one complete expansion: preserve the
struct, declare
whole-dialect support, prove coverage, and expose an ordered static description.
No staged callback or generated component instruction enum is involved.

## Input and ownership

Consume exactly one nonempty #[instructions { arithmetic, control }] helper
with optional trailing comma. Each entry is a plain dialect module path.
Reject leaf/wildcard/group grammar rather than supporting partial membership.
A syntactically valid path that names a leaf instead of a dialect fails its
dialect-contract reference; do not infer intent from capitalization.

Preserve named/tuple/unit structs, docs, visibility, ordinary attributes,
lifetimes/type/const parameters, defaults, and where clauses.
Use direct crate-private fields on internal parsed records; avoid getters that
force unrelated borrows through an entire declaration object.

## Implementation tasks

1. Parse the struct and typed options once. Strip consumed helpers once and
   preserve unrelated attributes. Early-check exact duplicate dialect paths.
2. Emit declared Supports<D> impls through each actual dialect module's fixed
   internal identity. Compiler coherence detects repeated dialect aliases.
   No helper path is supplied by the user.
3. Emit one declaration-local checked function with original generics/bounds
   that requires each dialect's complete Coverage<Self> obligation. Never
   add missing Execute requirements to its where clause. Strip parameter
   defaults from helper/impl generics while preserving them on the authored
   struct; use syn's impl-generics handling rather than copying declaration
   tokens verbatim into every generated item.
4. Build the complete ordered component description from listed dialect
   descriptions. Force canonical overlap validation independently of component
   state parameters whenever the listed dialect identities are fixed.
5. Accept same-head disjoint dialect descriptions; reject overlapping leaves
   anywhere in the complete list, even when no future set selects them.
6. Use the feature-on Component marker established in Step 2. A new component
   needs no aggregate ISA to declare capabilities. Default marker semantics
   stay current; shared consumers that need associations use explicit bounds.
   Coverage does not impose common Execute message/effect/error types beyond
   the component's own authored impls.
7. Keep all generated references in authored scope wherever possible. Emit
   local helper checks without globally exported names, sibling namespace
   discovery, or module-depth arithmetic.
8. Gate the complete expansion coherently under item-level cfg/cfg_attr.
   Verify the supported local declaration scopes from Step 1.
9. Export the attribute only with `typed-declarations` once all behavior works.
   Port relevant reference
   callers directly to whole-dialect syntax; add missing Execute impls rather
   than weaken coverage. Include public docs in this PR.

## Verification

Positive cases: order, qualified/absolute/module aliases, same-named disjoint
dialects, component type import aliases, concrete/generic type aliases, blanket
Execute impls, and explicit relative bounds/defaults.

Negative cases: empty/multiple helpers, old grammar, missing dialect contract,
repeated aliases, complete overlap, missing Execute on an unselected/new leaf,
and coverage that holds only for one generic instantiation.
Preserve structural checks and semantic compiler checks as distinct fixtures.

A custom Execute impl for an undeclared dialect remains unavailable. There is
no component-specific runtime/surface tree to navigate or maintain.
Run actual facade/runtime/renamed-dependency cases in isolated feature graphs
and all relevant repository gates. New examples explicitly opt in.

## Exit criterion

The complete public attribute works in one expansion with no inventories
returned as tokens. Whole-dialect support, generic coverage, order, and semantic
duplicates are enforced, and affected callers/docs compile.
Report the step and proposed dedicated commit; wait for explicit approval.

## End-of-file checkpoint

**Status:** approved and committed as `a849e04`. See [Step 4 results](step-4-results.md).
**Proposed dedicated commit:** `feat: declare complete component dialect capabilities`.
Report completed implementation, eligible files, checks, and material limits;
wait for explicit approval before committing. If the final public change is
breaking, adjust the conventional marker/footer to the reviewed diff.
Never stage or commit anything under `design/`. A completed report is not
commit approval, and no next step starts while this checkpoint is pending.
