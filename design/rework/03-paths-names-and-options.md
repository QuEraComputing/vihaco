# Step 3: paths, names, and options

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

**Prerequisites:** [Step 1](01-contracts-and-baseline.md),
[Step 2](02-inventory-model-and-protocol.md).
**Next:** [transport and trees](04-transport-and-type-trees.md).

## Outcome

Give every generated path an explicit source scope and destination scope.
Centralize root discovery, namespace inference, generated identifiers, and
runtime/surface tree resolution. Remove handwritten chains of `super` tokens
and repeated one-level rebasing calls from the macro implementations.

## Current implementation and lessons

[common.rs](../../crates/vihaco-runtime-derive/src/common.rs) resolves facade
and runtime roots and parses `crate`/`inventory_id` overrides. Shared inventory
code provides `sibling_namespace` and one-level rebasing, but component,
composite, and instruction-set code apply them differently. Composite namespace
inference also accepts ancestor generic path arguments that should not form a
module namespace. Surface paths are built separately from runtime paths.

Preserve the cross-crate behavior already tested. Avoid solving rebasing by
turning every caller path into a guessed `crate::...` absolute path; a macro
does not know its enclosing module or the target of a Rust import.

## Naming contract

| Item | Rule |
| --- | --- |
| Dialect namespace | `<authored dialect path>::__private`. |
| Component/composite namespace | Sibling `__vihaco_inventory_<snake_case inventory_id>`. |
| Default inventory ID | Declared dialect/component/composite identifier. |
| Backing exporter | Crate-root name incorporating producer kind and inventory ID. |
| Public surface enum | Replace terminal `Instruction` with `SurfaceInstruction`; otherwise append `SurfaceInstruction`. |
| Instruction-set helper module | Distinct `__vihaco_instruction_set_<snake_case enum_name>`. |
| Provider route segment | Deterministic local `provider_N` tied to request order. |

Provider indices are hidden implementation identifiers, not stable ABI values.
Preserve ordering rather than promising those names never change. Backing macro
names share the crate’s macro namespace: same-named declarations in different
modules still need distinct inventory IDs. Case-conversion collisions should
have explicit fixtures.

## Implementation tasks

1. Keep root resolution in shared common support. Parse `#[vihaco(...)]` once
   into typed options, with macro-specific supported keys and duplicate-key
   errors. Return a `syn::Path`, not tokens immediately reparsed as a path.
   Other macros using `resolve_root` must retain their behavior.
2. Add a single namespace-inference helper for concrete type paths. Accept
   optional terminal generic arguments; drop them only for the namespace.
   Reject qualified self, parenthesized arguments, and generic ancestors.
   Recognize bare generic type parameters when declaration generics are known.
3. Honor `inventory = path` overrides for imported renames, aliases, custom IDs,
   and child types whose namespace cannot be inferred. An override is a module
   path; validate it before inserting it into generated Rust.
4. Introduce a scope-rebasing operation accepting a module depth or explicit
   generated-scope descriptor. Differentiate ordinary module paths from
   type paths that contain generic arguments.
5. Use it for authored dialect paths, child inventory paths, framework roots,
   and qualified paths inside forwarded generics/where clauses. Visit nested
   type arguments and qualified self types without rebasing generic parameters
   or prelude names as though they were local modules.
6. Add one builder for `<namespace>::__types::{runtime,surface}::<route>` and
   one for `<namespace>::__selectors::<selector>`. Both tree kinds must be
   available in production, replacing the current test-only surface helper.
7. Centralize name generation and helper-attribute stripping. Strip only the
   attributes the macro consumed, preserving remaining authored metadata.

## Rebase semantics

For a path known to name a local module beside a generated child module,
rebasing by depth `d` follows these rules:

| Authored path | At depth 1 | At depth 3 |
| --- | --- | --- |
| `arith` | `super::arith` | `super::super::super::arith` |
| `self::arith` | `super::arith` | `super::super::super::arith` |
| `super::arith` | `super::super::arith` | `super::super::super::super::arith` |
| `crate::arith` | `crate::arith` | `crate::arith` |
| `::dependency::arith` | unchanged | unchanged |

An unqualified first segment can also name an external dependency. Do not
blindly turn `dependency::Device` into `super::dependency::Device`: that can
lose extern-prelude resolution. Prefer a definition-scope bridge in the first
generated namespace: import parent names there, resolve each bare producer
path once, and bind it under a hidden `pub use` alias. Rebase explicit
`self`/`super` roots into that bridge; build deeper mounts from the known alias
instead of guessing whether the authored root is local or external. Keep this
bridge in the first expansion and verify navigation through it.

Do not blindly apply module-path rebasing to `T`, `Option<T>`, `Self`, or trait
names inside generics.
Generic contexts can use a documented parent import plus rebasing of explicit
`self`/`super` roots, or a definition-scope bridge. Choose one policy and use
it consistently. Broad `use super::*` remains acceptable where it is needed
for that bridge, but it should not be an unexplained substitute for path rules.

Expose typed destinations such as producer namespace, runtime mount, surface
mount, and selector mount. Call sites should describe the destination, rather
than repeating a rebasing call three times and relying on module nesting.

## Verification

- Unit-test the path table at several depths, raw identifiers, custom roots,
  terminal lifetime/type/const arguments, and invalid ancestor generics.
- Compile nested modules using bare imports, `self`, repeated `super`, `crate`,
  external absolute paths, and bare external dependency roots without an
  explicit preceding `use`. Include renamed Cargo dependencies in the last case.
- Compile generic components with lifetime/type/const parameters, default type
  parameters, bounds, associated types, and explicit relative paths in bounds.
- Test facade use, direct `vihaco-runtime` use, renamed Cargo dependencies, and
  a re-exported custom root. The root must expose all hidden finalizers.
- Pin default/custom IDs, duplicate overrides, generated-name collisions,
  concrete generic target inference, and explicit alias namespace overrides.

## Exit criterion

There is one tested policy for naming and each path transformation. Producer
and consumer implementations no longer infer namespaces independently or
manually calculate tree-path nesting. Type identity remains anchored at the
producer definition across imports and crate boundaries.

## Implementation checkpoint

**Architecture review reopened:** the implementation below is uncommitted.
The user's encapsulation requirement rules out requiring namespace overrides
and exporter IDs. See the [parallel research](encapsulation-research.md).
Reconcile this step with the chosen architecture before committing or proceeding.

The earlier Step 3 implementation is complete for review. `common.rs` now parses typed `VihacoOptions`
with root-only and producer key policies, rejects duplicates at the later key,
validates module-path overrides, and returns a `syn::Path` for framework roots.
Existing macro callers retain facade/runtime discovery and custom-root behavior.
Consumed helper attributes share one stripping function; remaining metadata is
preserved. Composite field helper stripping now uses that function.

The private `inventory/paths.rs` centralizes generated names, concrete-type
namespace inference, explicit namespace overrides, runtime/surface type paths,
and selector paths. Inference accepts terminal lifetime/type/const arguments,
drops only that terminal segment's arguments, and rejects generic ancestors,
qualified types, parenthesized arguments, and known generic-parameter roots.
Aliases, imported renames, and custom IDs have an explicit override path.
Collected records reuse the same module-path validation.

Generated scopes distinguish the producer namespace, runtime/surface mounts,
selector mounts, and instruction-set helper. Known local paths support a depth
operation. Bare authored paths instead resolve through parent imports in the
first generated namespace and bind to a hidden `pub use` alias; deeper mounts
use that alias. Forwarded generics follow the same parent-import policy and move
only explicit `self`/`super` roots, including nested type/const arguments and
qualified self paths. Qualified `as` positions move with inserted parent segments.

Validation passed:

- 38 derive unit tests (24 existing inventory tests plus 14 new options/path
  tests), including actual three-crate rustc fixtures for type identity across
  nine authored path spellings, and depth-one/depth-three generic declarations.
- Renamed Cargo dependencies on the facade and direct runtime, with default
  discovery and custom re-exported relative roots, in offline Cargo fixtures.
  Both manifests and their shared Rust source are static files under
  `crates/vihaco-runtime-derive/tests/fixtures/roots/`.
- Three reviewed trybuild snapshots for duplicate framework roots, generic
  roots, and unsupported producer options on `Message`.
- Namespace case-conversion and crate-root backing-macro collisions, with
  successful distinct-ID compiler fixtures.
- Workspace format, warnings-denied Clippy, all-target tests, doctests, and
  SPDX checks (100 files, zero conflicts).

Review feedback removed hardcoded source-file writes from the test harnesses.
Root discovery compiles its static fixture manifests directly. Compiler tests
read fixed source from `tests/fixtures/paths/`, apply generated paths/names/
generics through `syn`, and send the resulting tokens to rustc through stdin.
Temporary directories contain compiler artifacts only. Static fixture Rust
files also passed an explicit rustfmt check.

The local [bridge output-shape probe](probes/step3-bridge.rs) preserved hover,
completion, and definition to the original runtime/surface declarations with
rust-analyzer 0.3.3065 (2026-09-27). Results are in
`/tmp/vihaco-step3-editor-results.json`; the isolated workspace is
`/tmp/vihaco-step3-editor`. The editor runner now accepts an explicit source and
probe list while preserving its Step 1 defaults. This tests concrete output
shape, not staged macro expansion. Owning macros and their hidden finalizer
re-exports remain in their planned steps; no placeholder entry points were added.

Eligible changes comprise 25 files: five modified derive files, the new path
module and its tests, the renamed-root Cargo fixture runner, and the root-options
trybuild runner plus three `.rs`/`.stderr` pairs under `crates/vihaco/`.
The fixture directory adds seven Rust files, two Cargo manifests, and a narrow
ignore rule for the fixture Cargo lockfiles.
Every `design/` file remains local and excluded from staging, commits, and PRs.

Proposed dedicated commit:
`refactor: centralize inventory paths names and options`.
Await explicit approval before committing Step 3 or starting Step 4. Nothing
from Step 3 is staged or committed, and no PR has been created.
