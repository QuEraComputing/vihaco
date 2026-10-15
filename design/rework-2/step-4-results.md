# Step 4 results: whole-dialect components

**Date:** 2026-10-01.
**Status:** approved and committed as `a849e04`.
**Proposed commit:** `feat: declare complete component dialect capabilities`.

## Behavior

The nondefault `typed-declarations` feature exports `#[component]` from derive,
runtime, and facade. One expansion preserves the authored struct, consumes exactly
one nonempty braced whole-dialect helper, implements Component and Supports for
each actual dialect identity, checks complete Coverage under original generic
bounds, and emits ordered static Capabilities. Syn impl generics remove helper
and impl parameter defaults while retaining them on the authored struct.

A declaration-local anonymous constant forces complete overlap validation
independently of component state parameters. Its checked function adds no
Execute/Coverage predicates. Same-head disjoint descriptions pass; exact repeated
paths reject structurally and repeated module aliases reject through coherence.
Extra Execute implementations grant no undeclared dialect availability.
No component instruction enum, sibling namespace inference, or token inventory
is emitted. Conditional item existence propagates to generated items; ordinary
struct attributes remain on the struct.

The facade's existing `component!` name and the new attribute share Rust's macro
namespace. Feature-off exports retain the current function-like macro. Feature-on
exports select the complete attribute. Existing CPU, tests, and guide/example
callers explicitly use the already existing runtime::component_macro export.
Their aggregate instruction/encoding/dispatch behavior remains covered in both
configurations. This introduces no adapter or newly named legacy macro.

## Eligible changes

- New derive emitter: crates/vihaco-runtime-derive/src/attr_typed_component.rs.
- Derive registration and shared module-path validator visibility.
- Runtime public export/documented executable example and facade feature-selected export.
- New runtime component integration tests and 17 static trybuild cases with snapshots.
- Existing renamed runtime/facade/indirect feature fixture callers now declare whole dialects.
- Existing CPU and facade test callers plus counter/quickstart examples and component guide imports.
- Component guide documentation of the experimental opt-in declaration model.

All committed implementation files are outside design/. The user approved the dedicated commit.
All design artifacts remain local and must be excluded from the dedicated commit.

## Verification

Compiler: rustc 1.96.0 (ac68faa20 2026-05-25), local macOS ARM64.

Passed:

- cargo fmt --all -- --check.
- Default and typed workspace clippy, all targets, warnings denied.
- Default and typed workspace all-target tests.
- Default and typed workspace doctests, including the executable runtime component example.
- Final focused component test rerun without TRYBUILD=overwrite.
- Isolated renamed runtime/facade dependency graphs, cross-crate imported/absolute paths,
  indirect feature activation, and feature-off fixtures.
- Runtime no-default-features and derive-only checks.
- mise exec -- hawkeye check: 100 files, zero changes/conflicts/unsupported.
- git diff --check.

Positive component tests cover declaration/leaf order, same-named disjoint dialects,
module/type import aliases, concrete and generic type aliases, lifetimes/type/const
parameters/defaults, relative bounds, named/tuple/unit structs, blanket execution,
independent message/effect types, relative roots, item cfg/cfg_attr, and function-local scope.

The 17 negative cases cover structural helper/option/path grammar, absent dialect
contracts, leaf paths, repeated aliases, missing execution for a newly added leaf,
coverage only for Cpu<u8>, complete overlap on an unused generic declaration, and
undeclared availability despite a custom Execute implementation. Generated
snapshots were inspected and rerun normally.

Logs: /tmp/vihaco-step4-*.log.

## Limits and checkpoint

Only macOS/Rust 1.96 was run locally; remote cross-platform CI has not run.
Parser crates and their Rust 1.75 requirement were unchanged. The guide's opt-in
example is marked ignore in the default guide doctest suite; its component shape
is compiled by the feature-gated runtime rustdoc example and facade fixtures.
Metadata remains internal generated support. Composite/set/parser work belongs
to later numbered steps and has not started.

Review completion is not commit approval. Wait for explicit user approval before
committing eligible implementation changes, and do not start Step 5 while this
checkpoint is pending.


## Approved commit

The user authorized committing Step 4 and requested a title-only message.
Commit `a849e04` is `feat: declare complete component dialect capabilities`.
It includes the reviewed follow-up formatting improvement, removal of the
emitted syntax module docstring, and instruction-helper diagnostic spans.
The qualified missing-contract case brings the component negative matrix to
18 cases. Formatting has a prettyplease regression test; the hidden parser hook
now operates on ordinary module items instead of an opaque macro body.

Final workspace lint, tests, doctests, and explicit feature-off runtime tests
passed, together with format/license checks and all commit hooks. Logs are in
/tmp/vihaco-step4-commit-*.log. The user's separate runtime default-feature
change was excluded from the commit and restored by the hooks. No design file
was staged or committed. Step 5 has not started.


## Replacement-feature correction

The user clarified that typed declarations replace the legacy component macro;
there is no feature-on compatibility emitter. Commit `10d5b88`
(`fix: replace legacy component macro with typed feature`) gates the legacy
emitter and exports out of typed builds, retains the same crate-root name in
both modes, and restores CPU/guide callers to their original component! usage.
No attributes namespace is used.

Legacy CPU and guide suites run in the default workspace graph. Typed CI/hooks
exclude vihaco-cpu and vihaco-doctests until their migration; new typed facade
coverage runs separately. A compiler fixture proves component_macro is absent
with typed declarations enabled. Default full-workspace tests/lint/doctests and
typed tests/lint/doctests in the supported graph all pass, as do commit hooks.
Logs are in /tmp/vihaco-component-replacement-*.log. No design file was committed.
Step 5 has not started.
