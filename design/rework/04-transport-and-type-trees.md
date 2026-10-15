# Step 4: transport and type trees

**PR:** [PR 1: shared inventories and the dialect producer](pull-request-plan.md#pr-1-shared-inventories-and-the-dialect-producer).

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

**Prerequisites:** [model/protocol](02-inventory-model-and-protocol.md) and
[path support](03-paths-names-and-options.md).
**Next:** [`dialect!`](05-dialect.md).

## Outcome

Make the shared inventory module the only owner of exporter and callback
plumbing. Give producers a small tree-emission API so the same model drives
their first-expansion mounts and later exported routes.

## Keep the expansion architecture

The [collector review](../instruction-inventory-simplification.md) recommends
the present direct chain. Keep one one-arm local callback per ordered request:

```text
producer 0 → callback 0 → producer 1 → callback 1 → procedural finalizer
```

Callbacks treat response bodies as opaque tokens. Each appends a record with
its namespace and consumer tag, then directly invokes the next producer or
finalizer. Zero requests directly invoke the finalizer with an empty
collection. Do not replace this with a request queue, a procedural collector
at every hop, per-entry callbacks, or global proc-macro state.

## Producer API

Use a typed description with a producer kind, inventory ID, generated namespace
layout, and tree mounts. The exporter receives a validated `Inventory` and
emits one complete response plus the module-qualified `__entries` re-export.

The exporter must not interpret selectors, filter entries, or construct
consumer-specific coverage checks. It validates that each exported type route
is usable through both its runtime and surface trees, retaining the current
compile-time type-reference checks where they catch broken producer wiring.

The first expansion emits:

- concrete leaf re-exports for a dialect;
- complete dialect provider mounts for a component;
- field mounts for a composite;
- the selector view needed for authored instruction-set payload navigation.

The finalizer emits the passive exporter after it knows the actual ordered
entries. Keep those phases explicit in the API; a helper should not accidentally
delay trees until finalization.

## Tree descriptions

Introduce small records such as `LeafExport`, `ProviderMount`, and `FieldMount`.
They contain spanned identifiers and typed namespace paths. Both runtime and
surface mount emission use the same source record and destination-scope rules.

Use `pub use` at leaves and mounts. Re-export the original concrete structs;
do not introduce type aliases, wrappers, or marker payloads. The historical
editor experiments specifically found that alias chains obscured navigation.

Keep provenance trees and the canonical selector view separate:

```text
__types::runtime::provider_0::cpu::Add → vendor_a::cpu::Add
__types::surface::provider_0::cpu::Add → vendor_a::cpu::syntax::Add
__selectors::cpu::Add                → the same runtime leaf
```

The selector view is for authored Rust references. The inventory is still
the authority for complete available capabilities, and the provider tree is the
authority for paired type resolution. Generated component mounts and inventories
must expose the same leaves; there is no filtered component tree or selection
tag. Canonical merges follow the Step 1 probe decision;
do not flatten provenance to make a merge easier.

## Implementation tasks

1. Extract exporter and direct collection generation to `export.rs` and
   `collect.rs`. Reuse the protocol renderer instead of reproducing entry fields.
2. Factor the collected-record token fragment once per request. Its next-hop
   and finalizer branches should differ only in the continuation.
3. Build a typed `ProducerRequest` from a producer namespace and opaque tag.
   Keep requests in authored order. Shared transport still supports repeated
   namespaces for sibling composite fields; component validation rejects repeated
   dialect membership rather than silently deduplicating it.
4. Give finalizer calls one shared envelope renderer. Context remains opaque
   during collection and is parsed once by the destination macro.
5. Implement leaf/provider/field tree helpers using Step 3 paths. Preserve
   `#[doc(hidden)]` placement and narrowly scoped generated lint allowances.
6. Keep collectors in the producer or instruction-set helper module. Ensure
   callback names cannot interfere with surrounding authored macros.
7. Re-export hidden procedural finalizers through runtime and facade; generated
   consumers must not require a derive-crate dependency. Keep existing feature
   gates consistent at every export boundary during migration.
   Add each finalizer entry point and its re-exports with its owning macro PR;
   PR 1 does not need placeholder component/composite/instruction-set finalizers.
8. Remove obsolete direct-response types or dead-code allowances if no
   production consumer actually uses them. Retain useful protocol-test helpers
   in test code rather than adding speculative public internal APIs.

## Verification

Use actual macro expansion in a small fixture to test zero, one, multiple, and
repeated producer requests; empty inventories; opaque state and tags; callback
count; and response order. A token substring count alone cannot establish
state forwarding or that the chain expands across crates.

Compile a producer → intermediate exporter → downstream consumer chain.
Include an aliased producer import and custom IDs. Resolve both runtime and
surface leaves and prove they are the original types with type assignments.

Inspect generated syntax through `syn` for the one-arm exporter contract and
first-expansion tree placement. Use a bounded many-producer fixture to detect
unexpected recursion growth without claiming an unlimited expansion depth.
Rerun editor probes if the tree placement changes.

## Exit criterion

All producers use one exporter, all consumers use one collector, and all tree
mounts are driven by typed records. Request order and cross-crate type identity
are established by expanding fixtures. Shared transport carries data without
knowing component membership, composite lifting, or instruction-set selection rules.
