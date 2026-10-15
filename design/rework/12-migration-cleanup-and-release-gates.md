# Step 12: migration, cleanup, and release gates

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

**Prerequisites:** [Step 11](11-diagnostics-cross-crate-and-editor.md).
**Next:** implementation complete, with separately tracked follow-up decisions.

## Outcome

Finish with one coherent implementation and documentation set, remove obsolete
draft machinery, and validate the supported Cargo configurations. Make any
feature-mode retirement an explicit breaking migration with bounded scope.

Caller and guide migration belongs to each behavior-changing PR. PR 2 migrates
whole-dialect component declarations and PR 4 supplies the compiled end-to-end
example. This step audits their completeness, closes remaining documentation
gaps, and owns any bounded feature-mode retirement. Required repository gates
apply to every PR, with the final feature/docs audit completed here.

## Feature and legacy decision

The current facade defaults to `component-attribute` and `composite-inventory`;
the runtime defaults only to `derive`. `Component`/`Composite` supertraits and
`#[composite]` expansion therefore differ by enabled features. The historical
off-by-default composite description is no longer accurate for the facade.

The cleanest eventual arrangement is unconditional marker traits, one composite
inventory expansion, and macro availability controlled by the ordinary `derive`
feature. Recommend removing semantic mode switches after their consumers have
migrated. This is a public breaking change, not just removal of private files.

The defaults above describe the reference branch. On the stack from `main`,
introduce only the migration boundaries needed as each macro lands; preserve
them until the complete replacement API and affected callers are ready. At this
step, use the migration inventory from Step 1 to choose a concrete outcome:

- **Retire modes in this rework:** migrate feature-off aggregate consumers,
  simplify marker traits and macro exports, and remove legacy composite code.
  Do this only if examples and tests can move without requiring new routing or
  bytecode policy.
- **Retain modes for a bounded follow-up:** finish the replacement inside the
  present feature boundaries, verify both modes independently, and record the
  exact blockers and consumers for retirement. Keep the legacy path isolated;
  do not leave a second inventory implementation.

Choose and record this once; do not leave docs implying a mode has disappeared
while its behavior still exists. Removing the older `component!` macro entirely
is a separate API decision. It currently supports consumers beyond the new
attribute, including aggregate instruction encoding. Its coexistence test can
remain until that decision is made. Avoid pulling generated execution or encoding
design into this cleanup merely to eliminate the name.

## Migration tasks

1. Search all macro calls, `HasInstructionSet` bounds, generated aggregate type
   references, macro re-exports, and feature declarations. Update dependent
   callers in the same migration change.
   Verify that PR 2 replaced component leaf/group/wildcard declarations with dialect lists
   such as `#[instructions { arith, boo, foo }]`, and supplied `Execute` impls for
   every leaf. Remove any residual old forms rather than retaining partial-selection aliases.
   If the domain needs a smaller vocabulary, define a smaller dialect instead
   of reintroducing a component-specific instruction filter.
2. If retiring modes, make `Component` and `Composite` unconditional markers.
   Generic code needing executable types must explicitly add `HasInstructionSet`.
   Remove the feature-off composite aggregate expansion and its generated parser
   association after consumers have authored sets.
3. Audit `vihaco-runtime/src/lib.rs`, facade `lib.rs`, and `macros/mod.rs` together.
   Expose the intended attribute path consistently and keep hidden finalizers
   available wherever emitted code references them. Do not accidentally rename
   the old `component!` re-export while preserving it.
4. Audit Cargo forwarding, fixture features, defaults, and CI package checks.
   A removed feature name must not remain in documentation or test crate manifests.
   If compatibility feature names are retained temporarily, make their behavior
   explicit and avoid continuing to select two composite implementations.
5. Remove replaced helper code, unused finalizer context fields, test-only
   production APIs, obsolete lint allowances, and token-string assertion tests
   superseded by semantic coverage. Keep line-sensitive diagnostics intentional.
6. Keep one inventory wire grammar. Remove historical `RuntimeN`/`SurfaceN`,
   `__vihaco_instructions`, per-entry selection arms, and old selector-attribute
   plumbing wherever it remains in implementation/callers. A compile-fail fixture
   demonstrating rejection of old syntax is still useful.
   Also remove component `Selection`, `select_names`, named/wildcard tags,
   overlap/unknown-leaf logic, and direct explicit coverage. Keep one uniform
   inventory-driven coverage path for all dialect leaves.

## Documentation and examples

Update guides for components, composites, instructions, parsing, and the guide
index. Review messages/observers examples where they assume legacy marker
supertraits or generated aggregates. Update README/API references where relevant.

The main example should show the full workflow:

1. A dialect with a surface-to-runtime operand mapping.
2. A component declaring whole dialects with `#[instructions { arith, boo, foo }]`
   and implementing `Execute` for every instruction in them.
3. Two composite levels, aliases, and sibling fields sharing the same leaf.
4. An authored selector-shaped instruction-set enum.
5. Parsing selected source forms, resolving operands explicitly, and manually
   dispatching author-named variants to the correct sibling field.
6. A loadable boundary and a brief explanation of why its entries are not lifted.

Put executable examples in `docs/examples` or compiled guide blocks and wire
them through `vihaco-doctests`. Use the actual default lowercase mnemonic policy,
or an explicit pattern when showing snake_case source names.

Mark older design documents as historical/superseded with links to this rework
plan and the final implementation contract. Update their status summaries and
contradictory author-facing examples; preserve useful rationale and experimental
evidence instead of deleting the history. Clearly describe whether the mode
switches remain and which macro syntax is current.

Document namespace override use for imported type names and custom inventory
IDs. Hidden tree/backing macro names are compiler plumbing; examples should
focus on leaf types and authored variants except when demonstrating an override.

## Required checks

Run the repository’s commit gates after the final implementation/migration:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --doc
hawkeye check
```

Add the standard two-line SPDX headers to new Rust source files; line-sensitive
trybuild fixtures remain excluded as configured. Doctests are a separate gate:
`--all-targets` does not run them. Build the docs with `pnpm build` after installing
dependencies with pnpm if needed.

Before retiring mode switches, check relevant packages independently:

```sh
cargo check -p vihaco --no-default-features
cargo check -p vihaco-runtime --no-default-features
cargo test -p vihaco-runtime --features composite-inventory
cargo test -p vihaco-runtime-derive --all-features
```

Also exercise feature-off composite behavior in a fixture whose dependency
graph does not re-enable inventory mode. The support producer dev dependency
enables component attributes, so a workspace or facade-only command cannot
establish every “features off” behavior by itself.

If modes are retired, replace the old feature-specific tests with independent
runtime/facade checks for their final `derive` availability and no-default-feature
contract. Keep CI’s independent package checks and Linux/macOS/Windows coverage;
do not require platform-specific code or local editor tools in ordinary builds.

## Final review and completion criterion

Review the end state against the responsibility table in the overview: one
validated model, one protocol, one exporter/collector, shared path/tree support,
and macro-specific semantics at their proper stages. Inspect final public enum
names, visibility, generic bounds, and error spans.

The work is complete when the selected macro APIs and all required checks pass,
examples teach the implemented behavior, and the editor verification is recorded
with its limits. Record any retained legacy mode as a specific migration follow-up,
not an unfinished rewrite of the new path. Use conventional commits and a breaking
change marker/footer where public behavior changes. Release through release-plz;
do not publish crates manually.
