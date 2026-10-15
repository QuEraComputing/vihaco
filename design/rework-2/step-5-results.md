# Step 5 results: typed composite fields and capabilities

**Date:** 2026-10-01.
**Status:** approved and committed as `7f63618`.
**Proposed commit:** `refactor: prepare typed composite capability traversal`.

## Behavior

The typed composite emitter is internal and compiled only with
`typed-declarations`. No public placeholder attribute, temporary namespace
option, or compatibility alias was added. Public activation awaits Step 7.
The current public composite expansion continues to run in this step.

Named fields are parsed once into records containing their actual identifiers,
types, device arguments, source names, optional sections, and stored eligibility.
Ordinary fields and item attributes are preserved. Device/loadable/root helpers
are consumed. Repeated helpers, device codes, canonical names/aliases, and section
names fail locally, including collisions involving section boundaries. Source
segments use standard string/character validation. Codes are optional and no
numeric codes are allocated implicitly.

Eligible fields get parent-local literal indices in declaration order, actual
`Field<INDEX>::Child` associations, canonical-first `SOURCE` arrays, and hidden
public `__vihaco_field_<unraw name>` constants. Their spelling retains case and
Unicode. No child types are relocated, no getters or companion namespaces are
generated, and original generic bounds remain in place. Declaration metadata
under the hidden `CompositeSpec` contract retains all device codes, source
spellings, and section names, including loadable devices. Consumers requiring
numbered metadata must require explicit codes.

Descriptions project child types through field relations and reference complete
child capabilities in field order. Reports use canonical field names; aliases
never duplicate report rows. A child's executable instruction set does not
filter its available instructions. Ordinary and loadable fields get neither
field relations nor parent capability entries. Loadable children need only their
loader contracts, without parent capability or instruction-set obligations.

Nongeneric declarations force complete description validation in an anonymous
constant. Generic declarations force fixed child descriptions immediately and
retain original bounds for dependent children. Concrete capability reporting
forces generic-dependent descriptions at code generation; the instruction-set
boundary will be implemented in Step 6. Opaque type macros are conservatively
considered dependent for generic declarations. No unused associated constant is
used as proof.

Section forwarding is shared with the current emitter. Its own-section hooks,
child order, optional child handling, and unexpected-section rejection remain
covered for bytecode and SST. Generated loader type/lifetime parameters avoid
shadowing authored generics. Authored program-counter implementations and child
instruction-set associations remain intact. Conditional item existence is
propagated to all generated items; conditional device membership is rejected.
Ordinary `cfg_attr` field attributes are retained.

## Eligible files

- `crates/vihaco-runtime-derive/src/attr_typed_composite.rs`: internal emitter,
  structural/validation tests, and static-fixture rustc-stdin runner.
- `crates/vihaco-runtime-derive/src/composite_loader.rs`: shared section emitter.
- `crates/vihaco-runtime-derive/src/attr_composite.rs`: existing emitter uses shared loaders.
- `crates/vihaco-runtime-derive/src/attr_typed_component.rs` and `src/common.rs`:
  share conditional-item gate extraction.
- `crates/vihaco-runtime-derive/src/lib.rs` and `Cargo.toml`: internal module and
  AST test traversal support; no public macro registration.
- Seven static compiler fixtures under
  `crates/vihaco-runtime-derive/tests/fixtures/composites/`.
- `crates/vihaco-runtime/src/declarations.rs`: hidden device metadata and docs
  relating actual field types, full capabilities, and executable selection.

All eligible implementation files are outside `design/`. The implementation was committed with explicit user approval. Design artifacts
were excluded and must remain excluded from every commit.

## Verification

Compiler: `rustc 1.96.0 (ac68faa20 2026-05-25)`, local macOS ARM64.

Passed:

- Default workspace clippy, all targets, warnings denied.
- Default workspace all-target tests and doctests.
- Typed clippy, all-target tests, and doctests in the supported graph:
  `--workspace --exclude vihaco-cpu --exclude vihaco-doctests --features vihaco/typed-declarations`.
- Final focused internal-emitter tests against the real runtime in an isolated
  feature graph; source comes from static fixtures and generated ASTs go to
  rustc stdin. Temporary output contains build artifacts, not generated source files.
- Existing renamed-runtime/facade/indirect feature graph fixtures and public
  composite/loadable fixtures through the workspace suites.
- Formatting, license headers, and `git diff --check`.

Positive compiler fixtures assert empty/state/loadable-only composites; repeated
child types; complete nested order; exact child/runtime type identities; imported,
qualified, aliased, generic, and associated types; lifetimes/consts/defaults;
raw identifiers, case and Unicode; canonical names and literal aliases;
optional/explicit device metadata; custom roots; item cfg/cfg_attr; ordinary
field attrs; function-local declarations; and loader generic-name collisions.
Execution verifies bytecode/SST parent-before-child nested loading, missing
children, unexpected children, and authored program-counter behavior.

Five negative compiler fixtures check unavailable ordinary/loadable targets,
missing generic capability bounds, fixed overlap even on an unused generic
parent, and complete nested generic overlap forced by concrete reporting.
Structural tests cover twenty invalid field declarations plus unsupported
shapes/options and duplicate roots. A separate type-dependence test checks
qualified fixed names, associated types, lifetimes, const expressions, and
opaque macros. No public macro diagnostic snapshots changed.

Logs: `/tmp/vihaco-step5-*.log`.

## Limits and checkpoint

Public typed composite activation awaits Step 7. Authored instruction-set and
selected parser work has not started. CPU and guide/example migration remain
outside the current typed graph, as established by the Step 4 correction.
Only local macOS/Rust 1.96 checks ran; remote cross-platform CI has not run.
Parser crates and their Rust 1.75 requirement were unchanged.

Review completion is not commit approval. Wait for explicit approval before the
dedicated implementation commit, exclude every design file, and do not begin
Step 6 while this checkpoint is pending.


## Approved commit

The user explicitly approved the dedicated commit. Commit `7f63618` is
`refactor: prepare typed composite capability traversal` on
`rob/composite-capabilities`. All configured hooks passed: format, default and
typed workspace clippy, and license headers. It contains the fifteen reviewed
implementation files and no design artifacts. Step 6 has not started.

## Later test-policy correction

On 2026-10-01 the user requested removal of custom compilation test harnesses.
The internal composite Cargo/rustc-stdin runner and its seven unused fixtures
are removed in the pending Step 6 changes. The structural validation and type
parameter dependency tests remain in process. The earlier compiler results
above are historical evidence, not retained automated coverage. Public-macro
semantic coverage belongs to ordinary integration tests and established
trybuild fixtures after Step 7 activation. Future tests must not launch custom
Cargo/rustc compilation or write generated source/build artifacts.

## Later device-code amendment

During Step 6 review the user required explicit u8 codes on every device,
including loadable sections, and chose those codes as the parent-local Field
keys. Hidden field-name constants contain the same codes; declaration order
only controls metadata/capability ordering. The active contract and Steps 5–9
supersede earlier optional-code and declaration-order-index decisions.
