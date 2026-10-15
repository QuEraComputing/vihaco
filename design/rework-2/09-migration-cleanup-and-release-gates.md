# Step 9: migration, cleanup, documentation, and release gates

**PR:** [PR 5: integration, migration, and cleanup](pull-request-plan.md#pr-5-integration-migration-and-cleanup).

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
**Prerequisites:** Step 8 committed; actual public integration evidence accepted.
**Next:** Final reviewed step; follow the PR merge workflow.

## Test implementation policy

Do not add custom test harnesses that write generated source or build artifacts,
invoke Cargo/rustc subprocesses, or execute generated fixture binaries. This
includes rustc-stdin compilation and nested Cargo feature-graph runners.
Use in-process parsing/emission assertions, ordinary unit/integration tests and
doctests, and established trybuild fixtures for compiler diagnostics. Run
independent feature graphs and checked-in fixture crates as top-level
verification commands or CI jobs, never from inside a test. Do not reintroduce
the removed compilation runners in a later step.

## Outcome

Finish caller/documentation coverage and remove obsolete preparation, preserving
Step 8's shipped CPU support in both cfg-selected feature modes. CPU migration
is a prerequisite delivered by Step 8. Keep the feature boundary until
stabilization; add no compatibility layer.
The public explanation and implementation should tell the same simple story:
actual instructions, complete dialect capabilities, actual fields, selected sets.

## Migration and cleanup tasks

1. Audit every macro caller and public guide/example against the frozen contract.
   Migrate dialect declarations to inline `#[dialect]` modules containing
   unrestricted pub `#[instruction]` unit/tuple structs, with ordinary operand
   types shared by runtime/surface fields and paths resolving in the authored
   module. New examples explicitly enable `typed-declarations`; current examples
   remain in feature-off builds unless migrated in this step. Add compiled
   examples for any newly migrated callers. Keep Step 8's compiled feature-on
   CPU example current; legacy guide examples may remain in a separately verified
   feature-off graph.
   Migrate relevant old partial/wildcard declarations to complete dialects with
   missing execution implementations included. Do not invent a partial mode
   solely to preserve a reference fixture.
2. Audit for obsolete preparation selectively ported from fbfec16 and old Step 3;
   the new branch starts at main and does not inherit that machinery. Remove
   any unused copies that were introduced:
   token protocol/collected-record types, provider/type routes, namespace
   inference, exporter IDs, tree builders, callback finalizers, and temporary
   dead-code/reexport allowances. Retain only actually used structural validation
   or framework-root/attribute support. Preserve unrelated pending user work.
3. Remove implementation-only tests of deleted protocols/trees and replace their
   behavioral coverage with typed identity/order/availability fixtures.
   Each feature mode selects its implementation directly; no protocol adapter
   or alternate compatibility macro is introduced.
4. Recheck the CPU migration delivered by Step 8 against the final public
   documentation and consumer graphs. Retain its mutually exclusive cfg-gated
   legacy/typed versions and coherent CPU-to-facade feature forwarding. Fix any
   regression exposed by cleanup; CPU migration itself must already be complete
   before this step starts. Preserve main's `CPU::stack: Vec<Word>` and safe
   operations/checks in both modes; cleanup must not introduce the benchmark
   worktree's unsafe stack implementation.
5. Record remaining consumer migration work before stabilization, excluding CPU:
   its feature-on implementation and consumer coverage were completed in Step 8.
   The experimental release keeps current defaults and the nondefault gate.
   Once stable, migrate remaining consumers, remove the old implementations, and
   remove the gate in a deliberate breaking release. Do not retain compatibility
   exports as a fallback or claim that retirement has already happened.
6. Update README crate/feature tables, release notes, guides, rustdoc, compiled
   examples, and migration examples. Show type/import aliases, complete coverage,
   nested target fields, ordinary operands, explicit Resolve/execution, and loadable
   boundaries. Require explicit u8 device codes and explain their use as
   parent-local Field keys. Loadable devices retain codes without parent
   traversal. Explain capabilities versus executable selection. Use
   syntax::AuthoredEnumName for set surface types, matching dialect::syntax::Leaf;
   put separate sets in separate authored modules when needed.
7. User guides never instruct authors to use inventory namespace overrides,
   generated IDs, companion trees, or hidden traits. Internal module docs explain
   association/validation ownership and timing for maintainers.
8. Verify the core example requires only VM declarations and existing runtime
   contracts. Future behavior can be additive; do not include speculative API
   scaffolding in this release.
9. Audit every stack commit and PR diff for the permanent design exclusion.
   Never move design records/probes into another directory merely to commit them.

## Required checks

Run the repository gates once the final changes are ready:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --doc
mise exec -- hawkeye check
```

Run independent affected runtime/derive/facade feature configurations, including
`typed-declarations` off/on and indirect feature unification, and the
static producer/consumer fixtures through top-level commands or CI jobs.
The final feature-on checks must include `vihaco-cpu`; excluding it is no longer
an acceptable final verification graph. Run CPU all-target tests and doctests
with the feature both off and on, plus actual codegen/execution through a
feature-on facade consumer and an indirect feature-unification graph. Run
feature-on workspace clippy/tests/doctests without excluding CPU; only remaining
unmigrated legacy documentation callers may use an explicitly documented separate
graph. Verify those legacy callers with the default workspace gates above.
Do not launch those commands from test bodies. Record the exact stable compiler; preserve
the parser crates' explicit 1.75 compatibility.
Build the docs site with pnpm when site behavior/content requires it.
Review final trybuild snapshots and reuse accepted actual editor evidence unless
new changes affect its conclusions.

Search active code/docs for stale override/transport/tree dependencies and
temporary allowances. Historical design documents remain local; their presence
does not imply an active implementation.

## Exit criterion

The gated new API is coherent and the shipped CPU is usable with the feature
both off and on, including when enabled by another dependency. CPU parsing,
resolution, selected dispatch/execution, encoding, effects/observation, and
multisection loading pass their regression and public consumer checks. CPU is
included in the final feature-on test/doc/lint graph. Obsolete preparation is
removed. Remaining consumer migration and removal of the old implementations/gate
are precisely recorded; no compatibility layer exists.
The stack is ready for normal PR review/merge and release-plz.
No manual publishing, design commit, or unrelated cleanup is part of this step.
Report the final diff, gates, limits, and conventional commit for approval.

## End-of-file checkpoint

**Status:** planned; this step has not been implemented under rework-2.
**Proposed dedicated commit:** `refactor: finish typed VM declaration migration`.
Report completed implementation, eligible files, checks, and material limits;
wait for explicit approval before committing. If the final public change is
breaking, adjust the conventional marker/footer to the reviewed diff.
Never stage or commit anything under `design/`. A completed report is not
commit approval, and no next step starts while this checkpoint is pending.
