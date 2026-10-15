# Rework contract and Step 1 evidence

**Status:** Step 1 accepted; no commit needed (design-only work).
**Recorded:** 2026-09-30. Step 2's model/protocol is now complete for review;
see its [implementation checkpoint](02-inventory-model-and-protocol.md#implementation-checkpoint).

## Revisions and implementation baseline

- Implementation branch: `rob/shared-inventories-and-dialect`, based on local
  `main` and `origin/main` at `cf70ca8e2f27a844329cbe24542873bf648204de`.
- Preserved reference: `rob/inventory-test` at
  `2b87ad69e0a312c62c2157b6317b48b386215cdd`. Its preceding commit is the
  previously reviewed `3a18b0f`. The newer commit contains the pending
  type-tree source, diagnostic fixtures, and `inventory_type_tree.rs`.
  No additional reference commit or branch switch is needed.
- Rust/Cargo: 1.96.0 (`ac68faa20` / `30a34c682`), aarch64 macOS.
- Editor: rust-analyzer 0.3.3065-standalone (`03fcb77246`, 2026-09-27), using
  the installed VS Code server binary. The rustup shim has no installed server.

The actual base has `component!`, `#[dispatch]`, and the legacy `#[composite]`.
It has no `dialect!`, `Execute<I>`, component attribute, instruction-set
attribute, or inventory modules. The derive sources are `common.rs`,
`component.rs`, `attr_component.rs` (dispatch), `attr_composite.rs`,
`attr_observe.rs`, `derive_message.rs`, and `lib.rs`.
`Component` and `Composite` both require `HasInstructionSet` on this base.
Runtime defaults contain only `derive`; there are no inventory-mode switches
in either base manifest. The draft has different marker traits and features;
do not copy those defaults into PR 1.

## Supported contract

1. One passive producer response contains the complete ordered inventory.
   Each entry has exactly `selector`, `type_route`, and `source_steps`.
   There is no per-entry declared-name metadata, origin, version, or paired
   runtime/surface transport alias. Type routes resolve through identically
   shaped runtime and surface trees in the collected producer namespace.
2. Dialects produce public unit/tuple runtime structs and paired surface
   structs, in declaration order. `u32 => i64` maps distinct operand types;
   conversion remains authored runtime code. A leaf accepts at most one
   pattern attribute. Duplicate patterns are an error, rather than last-wins.
3. Components take a nonempty comma-separated list of plain module paths:
   `#[instructions { arith, boo, foo }]`, with an optional trailing comma.
   Each path must name a dialect producer. Named-leaf, grouped, and wildcard
   selection semantics are unsupported; a path naming a leaf rather than a
   dialect is invalid even though it has the same lexical path shape.
   Components concatenate complete dialects in list/declaration order and
   check `Execute<I>` for every leaf under the component's full generic bounds.
4. Component duplicate logical selectors and duplicate canonical Rust runtime
   types are separate errors. Imported renames cannot evade either check.
   Distinct same-named dialects with disjoint complete leaves are valid;
   overlap anywhere in their complete inventories is invalid. Composite
   field paths can distinguish repeated concrete payload types.
5. Composites lift all non-loadable device capabilities in field/child order,
   independent of any child's executable set. They prepend canonical field
   selectors, tree routes, and source steps. Aliases affect source spelling
   only. Ordinary fields and loadable fields add none of those things, and
   loadable fields trigger no parent inventory collection.
6. An instruction-set declaration is a nonempty nongeneric enum with one
   plain selector path as each tuple payload. The target may have terminal
   concrete generic arguments, such as `Machine<u8>`. Conditional enums and
   variants remain rejected. Authored variant identity preserves route choice.
7. The runtime enum stays at authored scope in the first expansion. Surface
   visibility matches the authored enum; `XInstruction` gives
   `XSurfaceInstruction`, otherwise append `SurfaceInstruction`. The authored
   set owns the inventory-mode composite's `HasInstructionSet` implementation.
   Parsing accepts only selected leaves and the corresponding field spellings;
   prefixes use `::`, followed by the dialect's `head.mnemonic` syntax.
8. Inventories determine availability; trees determine Rust type resolution.
   Component trees and inventories expose the same complete leaves. Runtime
   and surface selection must agree with the selected inventory type route;
   a contradictory selector tree is invalid.
9. Declaration names determine default IDs. A component/composite override
   changes both its sibling inventory namespace and backing macro; a dialect
   override changes only its backing macro and keeps `dialect::__private`.
   Same-named declarations sharing the exported macro namespace need distinct
   IDs. Preserve existing root overrides, visibility, generics, and feature
   configurations while introducing each owning macro.

Raw module/path identifiers remain valid Rust paths. Structural identities and
generated IDs must normalize the raw prefix. The source dialect head should
use the unraw name; the draft's `r#type.halt` spelling is characterization only.
Initially reject raw **leaf declarations** with a spanned dialect diagnostic:
the parser derive cannot safely generate their pattern helpers today, even
with an explicit pattern. Supporting raw leaves requires a separate parser
helper correction; never forward the draft's proc-macro panic.

## Conditional items and scope

Preserve ordinary struct attributes, derives, documentation, visibility, and
unrelated field attributes; consume each macro helper exactly once. Duplicate
device/loadable helpers are errors. Item-level `cfg`/`cfg_attr` must gate the
entire expansion, whichever order is used around the attribute macro. The
reference probes establish disabled `cfg` before/after `component` and an
enabled `cfg_attr` component with a preserved derive. Conditional device or
loadable membership is initially rejected, including field `cfg_attr` that
could alter membership. This avoids emitting trees and inventories for a
field removed by Rust. Dialect conditions must likewise gate all of its output.

Inventory-producing declarations are supported at module scope. Function-local
components are unsupported: their generated sibling modules cannot access the
local struct. Do not advertise local declarations as scoped inventories. A
proc macro cannot generally detect its surrounding module versus block from
its input alone; document the boundary and retain a rejection fixture without
promising a dedicated scope diagnostic.

The Rust 1.96 compiler accepts **both** brace spellings when an outer attribute
macro consumes the item. The historical claim that `#[instructions = { ... }]`
is rejected before any macro runs does not hold on this toolchain. The reference
component rejects it in its helper parser. The rework still accepts only the
brace-list spelling in contract 3; no expression parser is needed.

## Selector-tree decision

Keep immediate provenance mounts under `provider_N` for both type trees.
Build canonical selector roots separately, using a narrow dialect mount helper
that can supply its declared root and concrete leaf re-exports independently
of entry callbacks. Group by canonical declared roots, never terminal imported
names. This is a type-tree operation, not another availability protocol or a
new field on every inventory entry. Preserve authored-scope runtime enums.

The reproduction establishes:

| Probe | Compilation | Editor leaf operations |
| --- | --- | --- |
| Two direct same-named dialect paths, disjoint leaves | Pass | Reference authored `field::cpu::Add`: hover, completion, definition to dialect token. |
| One imported rename, one dialect | Pass | Type identity established; separate navigation not measured. |
| One renamed root among two same-named dialects | E0659 | Compiler failure; not an editor acceptance case. |
| Two renamed same-named dialects | Provenance and inventory pass; canonical selector reference gives E0659 | Compiler failure on the canonical path. |
| Same dialect repeated through different aliases | Duplicate selector error | Not applicable. |
| Overlap in complete dialect inventories | Duplicate selector error | Not applicable. |
| Canonical merge emitted from inventory callback | Pass, including selected parser | Authored path and direct canonical reference: no hover/definition/completion. |
| Separate mount helper emitting concrete re-exports | Pass, including selected parser | Authored `field::cpu::Halt`: hover, completion, definition to `vendor_b` dialect token. |
| First-expansion surface provenance route | Pass | Hover, completion, definition to the surface leaf's dialect token. |

The helper probe models the output shape for two known `cpu` providers. It
does **not** implement or verify generalized helper collection/root grouping,
multiple composite levels, or arbitrary cross-crate helper transport. Steps
5–7 must implement and rerun those cases against the real macro expansions.
This bounded evidence favors the helper over moving the canonical tree into
the inventory finalizer. It does not establish universal editor behavior or
variant-hover spelling. Semantic-token classification was not measured.

## Characterization fixtures and remaining ownership

The [isolated probe suite](probes/README.md) adds eight semantic integration
tests and sixteen reviewed trybuild cases against the preserved reference.
Successful cases check concrete types, exact parsed variants/operands,
availability versus selection, order, aliases, ordinary attributes, and both
candidate selector views. Compile failures characterize missing coverage,
complete-inventory overlap, duplicates within one response, malformed finalizer
input without panic, helper duplication, conditional fields, local declarations,
generic ancestor inference, bare external dependency roots, old/unsupported
spellings, alias ambiguity, and raw-leaf failures.

These snapshots pin **reference behavior**, including its raw-leaf panic and
incidental unresolved-path errors. They are not the replacement's desired
diagnostics. Keep them isolated from the buildable implementation workspace;
port the relevant passing/failing cases into ordinary workspace tests as each
owning macro lands. Old wildcard declarations emulate complete membership only
inside the reference probes.

| Remaining case | Owner and acceptance requirement |
| --- | --- |
| Plain external dependency roots | Step 3: preserve extern-prelude lookup; avoid blindly qualifying bare dependency roots with `super`. Keep absolute roots and authored imports working. |
| Duplicate structural selectors, malformed protocol, raw-path identity | Step 2: validated construction and structural keys, including duplicates in a single producer response. |
| Duplicate patterns, raw leaves, conditional dialect items | Step 5: explicit validation and spanned diagnostics; no parser-helper panic. |
| Actual dialect-list grammar and uniform generic coverage | Steps 6–7: migrate wildcards/partial fixtures, check all leaves under generic bounds, verify generalized canonical mounts and new-leaf obligations. |
| Recursive exclusion, duplicate helpers, ancestor inference | Step 8: test actual lifted inventories and namespace validation; preserve loaders. |
| Contradictory runtime tree versus inventory route | Step 9: compiler type-identity check alongside checked selection; do not trust a received selector map. |
| Exact selected variants beyond tuple-choice limits | Step 10: port `instruction_set_many.rs` with variant/value assertions. |
| Public helper export through a crate boundary and editor nesting | Steps 5–11: repeat compile/editor checks against the implemented helpers; existing cross-crate draft tests are only baseline evidence. |

The reference's eleven focused targets and seventeen derive unit tests were
rerun successfully. They cover the preexisting leaf, component, composite,
instruction-set, many-branch, cross-crate, type-tree, and four compile-fail
suites. The new probes augment that evidence without claiming the future
contract is implemented on `main`.

## Migration inventory and mode-retirement boundary

There are no partial component declarations on the actual implementation base.
When porting reference callers, PR 2 owns its component/direct-runtime fixtures
and producer fixture; PRs 3–4 own callers requiring their new macros.

| Reference partial callers | Migration |
| --- | --- |
| `component_attribute.rs`: Calculator and Generic | Full `arith`/`boolean` membership; add missing boolean `And`/`Halt` or `Not` coverage. Preserve dialect/declaration order. |
| `component_attribute.rs`: Qualified, Overridden, first/second Device | Full arith coverage, or declare an actually narrower dialect when that device intentionally exposes a smaller vocabulary. |
| `composite_inventory.rs` and `instruction_set.rs` leaf devices | Complete `cpu` coverage even where executable sets omit leaves; port with owning composite/set macros. |
| `inventory_cross_crate.rs`, `support/inventory-producer`, direct-runtime tests, `support/instruction_set_fixture.rs` | Complete dialect lists; add missing coverage. Preserve canonical selectors through renamed imports. |
| `inventory_type_tree.rs` and `instruction_set_many.rs` wildcards | Plain dialect module lists; the existing complete coverage remains applicable. |
| Compiled components/composites guides | Replace selected declarations in the PR exposing the corresponding macro and rerun doctests. |

Legacy consumers on `main` include CPU's `component!` in `data.rs`,
`component_macro_new.rs`, both compiled examples (`counter.rs`, `quickstart.rs`),
component/instruction/parser/observer guides, and multisection loader fixtures.
Legacy composites consume `HasInstructionSet` and generate aggregate runtime
and syntax instruction enums. Loader fixtures reference those generated enums,
including `nested_machine::runtime::Instruction`, and legacy children such as
`LoadedDevice` do not export inventories.

Introduce inventory composite mode behind an explicit opt-in boundary in PR 3;
retain the existing legacy API until PR 4 can migrate authored sets. Step 12
must not remove the mode just because the new macros compile: CPU has encoded
aggregate instructions and the legacy composite fixtures require paired
aggregate types/parsers. Retiring these consumers requires a concrete adapter
and migration audit. If that needs new routing/bytecode semantics, retain the
legacy mode with those consumers recorded as blockers and a follow-up task.
Removing `component!` itself remains a separate API decision. No caller should
be migrated to the draft's partial API as an intermediate step.

## Checks and checkpoint

Passed on the implementation base: workspace format, Clippy with warnings
denied, all-target tests, doctests, and SPDX check (85 files, zero conflicts).
Independent runtime checks passed with no default features and with `derive`;
the derive crate's all-features tests passed. The preserved draft's focused
baseline and the isolated Step 1 fixtures passed. Probe integration sources
were separately formatted with rustfmt; diagnostic fixture lines were preserved.

No product macro changes, commit, push, or PR were made in Step 1. All Step 1
changes are under `design/`, so this step has a no-commit
review checkpoint. Never stage or commit anything under `design/` at any point
in the plan, including these contract records, probes, fixtures, and results.
The previously proposed Step 1 commit is withdrawn. PR 1 remains based on
`main`; Steps 2–5 have separate reviewed and approved implementation commits.
Existing untracked historical design documents are preserved in place.
