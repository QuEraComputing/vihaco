# Step 7: component finalization

**PR:** [PR 2: whole-dialect component contracts](pull-request-plan.md#pr-2-whole-dialect-component-contracts).

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

**Prerequisites:** [model](02-inventory-model-and-protocol.md),
[transport](04-transport-and-type-trees.md), and
[component first expansion](06-component-input-and-first-expansion.md).
**Next:** [composite](08-composite.md).

## Outcome

Validate complete dialect inventories, establish coverage and uniqueness for
every leaf, then concatenate and export one ordered component inventory. The
component's entire tree and inventory describe exactly what it must execute.

## Starting point

[component_attribute/finalize.rs](../../crates/vihaco-runtime-derive/src/component_attribute/finalize.rs)
currently selects named/wildcard entries and splits explicit and wildcard
coverage between expansion stages. Remove that selection and coverage split.
Keep provider-route prefixing, generic coverage, semantic duplicate checks, and
export, using Steps 2–3's shared support and a compact context.

## Complete-dialect algorithm

For every collected record, in request order:

1. Parse its provider/request tag and validate it against the context’s
   expected request identity. Require the expected response count and ordering;
   diagnose malformed internal tags rather than treating them as indices blindly.
2. Validate a dialect-shaped inventory: logical paths identify declared dialect
   leaves and have no composite source prefixes. Do not assume any imported
   alias is the declared dialect name. Require one canonical dialect root,
   `type_route == selector`, empty source steps, and a nonempty inventory. The
   dialect exporter guarantees completeness; a consumer cannot enumerate Rust
   module members to independently detect an incomplete hand-written exporter.
3. Keep every received entry in its declaration order. There is no named lookup,
   wildcard expansion, filtering, or per-instruction availability decision.
4. Resolve every runtime type from the response namespace and original route.
   Build the same `Execute<I>` obligation for every leaf before rewriting routes.
5. Prefix the provider only on exported type routes. Preserve each logical
   selector and its empty source-step list.
6. Concatenate complete inventories with full-selector uniqueness. Preserve
   dialect-list order and declaration order inside each dialect. Reject repeated
   dialect membership, including aliases of the same nonempty dialect, rather
   than deduplicating or accepting two requests for different leaf subsets.

Produce a small resolved model holding the exported inventory, canonical
runtime paths and one uniform set of coverage obligations. The emitter receives
that validated result without any selection mode or explicit-leaf special case.

## Coverage and duplicate semantics

Generate an `Execute<I>` obligation for every instruction in every declared
dialect, inside a function checked under the component’s original impl generics and
where clause. The obligation must hold for every supported instantiation;
do not add a new `where Component<T>: Execute<I>` predicate that merely makes
the check conditional and accepts incomplete generic coverage.

Keep a local marker trait keyed by runtime type and generate one impl per
exported entry. Conflicting impls detect equal canonical Rust types reached
through different paths or provider aliases. Comparing selectors or type-route
tokens cannot replace this proof. Give the generated checks spans from the
authored dialect declaration where possible, including the uncovered leaf's
canonical name in useful diagnostics.

Reject logical duplicates before emission, including distinct dialects both
claiming `cpu::Halt`. Distinct `cpu` dialects with disjoint complete leaf sets
are valid, and their type routes retain different provider branches. The
same leaf offered in two different composite fields will later be valid;
do not move the component’s type-duplicate restriction into shared concatenate.

Adding a new leaf to a dialect creates a new required `Execute` impl in every
component declaring that dialect. Verify this with a dialect whose added leaf
lacks coverage: the whole component must fail, even if no composite's authored
instruction set selects that leaf. This requirement must not be weakened by
downstream instruction-set choices or by conditionally generated bounds.

## Implementation tasks

Activate the new public component attribute only after its finalizer is complete.
Include the Step 6 compilation and editor cases in this step's integration checks.

1. Parse the shared finalizer envelope and compact component context once.
2. Implement the ordered algorithm as a `Result<ResolvedComponent>` function.
   Consume each received inventory when prefixing routes and concatenating it;
   retain only the runtime paths needed for coverage and duplicate checks.
3. Validate tags and dialect response shape at this boundary. A hand-written
   provider should not smuggle lifted composite entries into a dialect declaration.
4. Generate coverage and semantic duplicate checks using the shared root,
   generics-scope policy, and tree resolver.
5. Emit the exporter through the single shared builder. Keep its routes aligned
   with the provider mounts produced in Step 6.
6. Complete any canonical selector merge deferred by Step 1’s chosen approach.
   Verify that it exposes exactly all runtime leaves exported by the inventory and
   does not merge provider provenance. First-expansion editor behavior remains
   an acceptance condition rather than a presumed consequence of compilation.
7. Remove `Selection`, `select_names`, selection tags, unknown-leaf/overlap
   component machinery, direct explicit coverage, whole-item serialization,
   and duplicate context/tag parsing mechanics replaced by shared support.

## Tests

- Observe exported selector order for several complete dialects: authored
  dialect order followed by each dialect's full declaration order.
- Resolve both type trees across imported aliases and cross-crate components;
  assert runtime/surface pairing for mapped operands.
- Require coverage for every leaf, including generic components whose
  `Execute` implementation is valid only for one concrete parameter.
- Reject repeated dialects through aliases, canonical selector duplicates,
  and semantic runtime-type duplicates with different logical names.
- Exercise true same-named dialect collisions and valid disjoint leaves,
  including Step 1’s imported-alias and complete-inventory collision cases.
- Reject malformed tags, unexpected responses, duplicate provider entries,
  and invalid type routes without panicking.
- Assert exact equality between generated component tree leaves and inventory
  entries, including surface pairing. There are no tree-only unused dialect leaves.
- Show that adding an uncovered instruction to any listed dialect fails coverage,
  even when the component's downstream authored set omits that instruction.
- Show that a dialect omitted from the component declaration contributes no
  tree branch or inventory entry and cannot be selected through that component.

Run the component integration and trybuild harness plus cross-crate inventory
tests. Review changed E0119/trait-bound spans deliberately; do not overwrite
unrelated stderr fixtures.

## Exit criterion

The component exports every leaf of its listed dialects and proves both generic
execution coverage and canonical type uniqueness. Its inventory, provenance
trees, and selector view agree for the complete leaf set. It creates no aggregate
instruction types or automatic execution implementation.
