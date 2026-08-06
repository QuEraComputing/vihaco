# Concepts to Review

This is a holding list for vihaco concepts that are unused by the current
runtime/demo path, remain only for compatibility, or are described by stale
documentation. Nothing in this file is approved for deletion. Each item needs
a separate decision after the relevant deferred phase or downstream usage has
been audited.

## Review criteria

- `phase-one unused` means the concept is not needed by the current runtime
  execution pipeline, but may be required by a planned phase.
- `transitional` means a newer API has replaced the concept in the intended
  design, but references or compatibility code remain.
- `likely unused` means there is no current framework/demo consumer visible in
  this workspace; external users must still be checked before deletion.
- Stale documentation is listed separately from code so it can be corrected
  without prematurely removing an API.

## Runtime and macro APIs

### `GeneratedMachine` and `CompositeMetadata`

Status: phase-one unused; retain pending module/source-symbol resolution.

Evidence:

- `composite!` still generates `GeneratedMachine` in
  `crates/vihaco-runtime-derive/src/composite.rs`.
- The current consumer is the crate-override test in
  `crates/vihaco/tests/runtime_macro_crate_override.rs`.
- `CompositeMetadata::validate_source_symbols` and alias lookup support the
  future module-loading/source-resolution path, but are not part of runtime
  instruction execution today.

Decision needed: whether metadata generation belongs in the first public
`composite!` API or should be deferred until module/source-symbol resolution is
implemented. Do not delete the trait or metadata types yet.

### `CompositeMetadata` helper methods

Status: phase-one unused/low-use.

The `devices`, `device_by_name`, `source_symbol_aliases`,
`source_symbol_device_code`, and `validate_source_symbols` helpers are defined
in `crates/vihaco-runtime/src/generated.rs`. Only some are used internally or
by tests. Revisit once `vihaco-syntax::Resolve` and module loading consume
machine metadata.

### `EffectSink`

Status: likely legacy compatibility; audit before removal.

`EffectSink` lives in `crates/vihaco-abi/src/traits/event_sink.rs`. The new
composite runtime routes effects through `Absorb`, `Observe`, and route-aware
`Handle`. Its visible current use is primarily facade/API compile coverage.
Audit ABI consumers and external compatibility requirements before removing or
de-emphasizing it.

### `Observe` follow-up effect stream

Status: transitional contract requiring a design decision.

`crates/vihaco-runtime/src/observe.rs` retains an associated `Effect` type and
returns `Effects<Self::Effect>` for compatibility with the existing
`#[observe]` macro. Generated composite routing currently discards those
follow-up effects. Decide whether observers should remain effect-producing or
whether the public contract should be simplified to observation returning only
`Result<(), Error>`.

### Demo-local `Route` trait

Status: likely unused; mark for deletion review after demo cleanup.

`demos/examples/demo/vihaco/route.rs` defines a route abstraction, but the
current generated composite uses private route marker types directly as the
`Observe`/`Handle` route parameter. No framework code depends on the demo-local
trait.

### `machine!` placeholder

Status: deferred design material, not an implemented framework API.

`demos/examples/demo/vihaco/machine_macro.rs` contains a placeholder/comment for
the former `machine!` direction. The public first-iteration macro is
`composite!`; decide whether the placeholder should be removed or retained as
historical design material once structural composite migration is complete.

### Historical `demos/src/main.rs` scaffold

Status: likely whole-file deletion candidate.

The file contains an empty `main`, a no-op local `machine!`, and obsolete local
concepts such as `NoFault`, `NoEffect`, `Step`, and local `Type`/`Value` models.
It has no apparent role in the current examples or vision. Confirm that no
workspace target or documentation links to it, then remove it if it is only
historical scaffolding.

### `SchedulerMetadata` and `SharedDeviceMetadata`

Status: likely unused; audit before removal.

`crates/vihaco-abi/src/metadata/scheduler.rs` defines and re-exports these
metadata types, but there are no repository consumers, demo uses, or clear
vision references. Check downstream API compatibility before deleting them.

### `Message` marker trait

Status: low-use API requiring review.

The runtime derives and exports the `Message` marker, but the current demo and
execution contracts use concrete message types and `NoMessage` without needing
the marker. Determine whether it is still a meaningful bound for the derive or
whether it is legacy API surface.

### `expect_exactly_one_effect`

Status: likely legacy helper; review after documentation migration.

`crates/vihaco-runtime/src/generated.rs` exports this helper. It is used by
legacy examples/tests but not by the current demo execution pipeline or the
vision execution model, which handles `Effects` as a stream. It may still be a
useful general helper, so deletion should follow a usage and API audit.

## Stale documentation and package metadata

### `GeneratedComponent` references

Status: transitional remnants; clean up documentation and metadata.

`GeneratedComponent` was removed from the compiled runtime API, but references
remain in places such as:

- `crates/vihaco-runtime/Cargo.toml` package description;
- `docs/src/pages/guide/composites.md`;
- `docs/src/pages/guide/components.md`;
- `docs/src/pages/guide/messages.md`;
- `docs/src/pages/guide/observers.md`;
- `docs/src/pages/quickstart.astro`;
- `demos/examples/demo-vihaco-concepts.md`.

These should be migrated to `Execute<I> -> StepResult<E>` and the
`composite!` route model, or explicitly labeled historical/deferred material.

### Old attribute-macro documentation

Status: stale transitional documentation.

Several guide sections still describe `#[component]` and `#[composite]`, while
the current runtime derive exports function-like `component!` and `composite!`.
The affected examples should be updated or clearly marked as historical before
the public API is considered documented.

### Stale concept/design references

Status: documentation reconciliation required.

- `demos/examples/demo-vihaco-concepts.md` contains stale paths and old
  `to <component>`/`effects to` syntax; the agreed syntax is `absorb with` and
  `handle with`.
- `vision/macro-generation.md` should be reconciled with the selected route
  syntax and the actual `Handle<E, R>` contract.
- `vision/execution-pipeline.md` presents `HandleEffects<R>` as an alternative
  design. Decide whether it remains an explicitly rejected alternative or
  should be removed to avoid competing public models.
- Remaining `GeneratedComponent` references in README, guide pages, and demo
  concept notes should be migrated or labeled historical.

## Explicitly not review candidates at this time

The vision and execution-pipeline documents describe the following as active
runtime concepts, and the current implementation/tests use them: `Execute`,
`Execution`, `StepResult`, `NoMessage`, `Supply`, `Absorb`, `Handle`, route-aware
observation, and the component execution/effect pipeline. Their absence from a
particular demo path is not sufficient evidence for deletion.

Similarly, `Resume`, clock/scheduling, surface/module resolution, bytecode
loading, `Resolve`, `ProgramImage`, and generated scheduling are deferred or
demo-independent capabilities described by the vision. They are not deletion
candidates merely because the first runtime iteration does not exercise them.
