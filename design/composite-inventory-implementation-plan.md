# Composite inventory implementation plan

> **Status:** implementation plan
> **Scope:** the feature-gated, inventory-exporting `#[composite]` expansion and
> the small shared changes it requires. `#[instruction_set]`, generated routes,
> and removal of the legacy expansion are later work.
> **Design references:**
> [composite instruction lifting](composite-instruction-lifting.md),
> [structured instruction inventories](structured-instruction-inventories.md),
> and the [implemented dialect/component protocol](dialect-component-instruction-inventory-plan.md).
> The latter supersedes the older proposal's `version` and `origin` fields.

## Intended result

With `composite-inventory` enabled, `#[composite]` exports every instruction
available through its non-loadable `#[device]` fields. It preserves each
child's canonical runtime and surface types, prepends the field name to the
logical selector, and records
the field name and aliases as one source-prefix step. A parent composite reads
that export through the same hidden `__entries!` protocol used by dialects and
components. Export does not depend on an authored instruction set.

For a component that exports `cpu::ConstI64`, this pair of composites yields:

```rust
#[composite]
struct Stream {
    #[device(1, alias = "c")]
    core: CPU,
}

#[composite]
struct Hydra {
    #[device(1)]
    stream_a: Stream,
    #[device(2)]
    stream_b: Stream,
    #[device(3)]
    #[loadable]
    auxiliary: Stream,
}
```

`Stream` exports `core::cpu::ConstI64`; `Hydra` exports
`stream_a::core::cpu::ConstI64` and `stream_b::core::cpu::ConstI64` in that
order. The two Hydra entries may have identical payload types. Their selectors
remain distinct. `auxiliary` contributes no Hydra entry, and the `core` source
step accepts both `core::` and `c::`.

## Feature and compatibility boundary

Add an opt-in `composite-inventory` Cargo feature. It is **off by default**.
Forward it from `vihaco` to `vihaco-runtime`, and from `vihaco-runtime` to a
matching `vihaco-runtime-derive` feature. It implies `component-attribute`,
because the new component inventory is the immediate leaf provider. Keep the
existing default feature lists unchanged.

The feature flag alone selects the expansion of the existing `#[composite]`
attribute. With the feature off, it keeps its current runtime/syntax enums,
parser, `HasInstructionSet` impl, and section-loading behavior. With the
feature on, it exports the inventory described here and stops generating the
aggregate enums and `HasInstructionSet` impl. Both expansions reject attribute
arguments; there is no per-declaration mode. Cargo feature unification can
enable the new expansion for all `#[composite]` uses in a dependency graph, so
feature-on tests must migrate or explicitly account for existing consumers of
the aggregate types.

This is a temporary feature boundary. After consumers move to authored
instruction sets, enable the new expansion by default and remove the old
aggregate expansion and its feature switch in a later change. Gate legacy
tests to the feature-off configuration or migrate them before the feature-on
workspace suite runs.

With the feature enabled, make `Composite` a marker trait without a
`HasInstructionSet` supertrait, as `Component` already is under
`component-attribute`. Keep the original supertrait when the feature is off.
The feature-off expansion still implements both traits. Feature-on composites
implement `Composite` but do not implement `HasInstructionSet`; a later
`#[instruction_set]` will own that association. Update generic call sites
that use the instruction types to name `HasInstructionSet` explicitly. Test
both feature configurations, including Cargo feature unification.

## Inventory contract

Use the current shared `Inventory`, `Entry`, `SourceStep`, `collect`, and
`export_contents` code in `vihaco-runtime-derive/src/inventory/`. Do not add a
second wire format or per-entry declarative-macro arms. Entries contain only
`selector`, `runtime`, `surface`, and `source_steps`; they have no `version` or
`origin` field.

For each eligible field, in declaration order:

1. Request the child's **complete** inventory once.
2. Resolve its bare `RuntimeN` and `SurfaceN` aliases through the collected
   child namespace.
3. Prepend the canonical Rust field identifier to each child selector.
4. Prepend one `SourceStep` with the field name and its declared aliases.
5. Preserve the child's entry order, type paths, and remaining source steps.

Concatenate these transformed inventories without sorting or deduplicating.
Reject a resulting logical-selector collision. Repeated child types in
different fields are valid. An inventory may be empty: a composite containing
only loadable devices or non-device fields still exports an empty inventory.
Only `#[device]` fields without `#[loadable]` are eligible. Continue to require
`#[loadable]` fields to be devices, and retain their existing section-loading
behavior.

The exported inventory is the set of **available** entries. It is independent
of any later authored instruction-set selection, so a parent can consume a
child composite that has no executable enum.

## Finding child inventories

For a direct concrete field type path such as `crate::devices::CPU<B>`, derive
the sibling namespace by replacing the terminal type name with the shared
`__vihaco_inventory_*` name. Drop type arguments only from the namespace path;
leave the field's declared Rust type unchanged. Reanchor relative paths from
the generated sibling module, following the component finalizer's existing
path rules. A concrete generic application has a fixed inventory because the
provider is the `CPU` declaration. A bare type parameter does not identify a
fixed provider automatically.

Extend `#[device]` with an optional `inventory = path` argument. This names
the child's hidden inventory namespace explicitly and is required for an
imported rename, type alias, non-default inventory name, or field type from
which a namespace cannot be derived. It does not change the canonical field
selector or accepted source spelling. Reject duplicate overrides, malformed
paths, and an unsupported automatically derived field type with a diagnostic
at the field attribute. An unresolved namespace should produce an ordinary
Rust path error at the request site.

The current component exporter names its sibling namespace from the declared
type while `inventory_id` changes only its crate-root backing macro. The older
design says the ID changes both. Resolve this before cross-crate composite
tests: make the shared naming rule use `inventory_id` for both component and
composite namespaces, update existing component tests and documentation, and
require `inventory = path` at a use site when the producer has a non-default
ID. Keep the existing root-backed macro naming and its collision checks.

## Expansion stages

1. **Initial attribute expansion:** Parse the struct, strip consumed
   attributes, and retain current validation of unique device codes, field
   names, aliases, and loadable section names. Record field type, canonical
   name, aliases, loadable status, and optional inventory path in a typed
   field model. The feature-on expansion should not build the legacy aggregate
   enums or require child `HasInstructionSet` implementations.
2. **Collection:** Emit the struct and `Composite` impl, then a hidden sibling
   inventory module. Inside that module, use `inventory::collect` with one
   `ProducerRequest` per eligible field. Its opaque tag carries the field name
   and aliases. Carry the composite's identity, configured crate root, and
   inventory ID as finalizer context. An empty request list goes directly to
   the finalizer. Keep the existing direct callback chain; the collector
   simplification review proposes no change to it.
3. **Finalization:** Add a hidden `__composite_inventory_finalize!` proc macro.
   Parse the collected records and field tags, resolve aliases, call the shared
   prepend and concatenate operations, and emit the combined inventory with
   `export_contents("composite", ...)`. Re-export the finalizer through
   `vihaco-runtime` and the `vihaco` facade so downstream expansion needs no
   direct derive-crate dependency.
4. **Feature-off path:** Keep the existing aggregate generation and loaders
   when the feature is disabled. Share field parsing and validation where
   practical. Feature-on output must keep bytecode and SST section delegation
   for `#[loadable]` fields, without making them inventory entries.

The hidden namespace contains definition-site aliases and one `__entries!`
request arm. Its macro responds once with the complete ordered inventory and
passes caller state through unchanged. It generates no per-entry wrapper
types, parser, aggregate enum, or `HasInstructionSet` impl.

## Concrete code changes

| Location | Change |
| --- | --- |
| `crates/vihaco/Cargo.toml`, `crates/vihaco-runtime/Cargo.toml`, `crates/vihaco-runtime-derive/Cargo.toml` | Add and forward the opt-in feature. |
| `crates/vihaco-runtime/src/generated.rs` | Conditionally remove the `Composite: HasInstructionSet` supertrait when the feature is enabled. |
| `crates/vihaco-runtime-derive/src/lib.rs` and runtime/facade re-exports | Select the composite expansion at compile time and expose the hidden finalizer. Reject attribute arguments. |
| `crates/vihaco-runtime-derive/src/attr_composite.rs` | Reuse validation, parse `inventory = path`, form eligible-field requests, and retain section loading in the feature-on expansion. |
| New `crates/vihaco-runtime-derive/src/composite_inventory/` | Own finalizer context/tag parsing and composite-specific validation. Use shared inventory parsing and export. |
| `crates/vihaco-runtime-derive/src/inventory/` and component namespace call site | Reuse collection/export; settle the shared `inventory_id` namespace rule. |
| Integration and trybuild tests | Pin order, path resolution, aliases, boundaries, feature behavior, and diagnostics. |

## Tests and completion boundary

Add integration tests that inspect the exported callback data, not just whether
the annotated struct compiles. Cover one component child, two sibling fields
of the same type, two composite levels, nested alias source steps, explicit
inventory-path overrides, module-qualified and cross-crate children, a
concrete generic child, and a generic composite. Verify that a loadable child
is omitted from the parent inventory while its section loaders still work.
Check an empty inventory. Verify the feature-off `#[composite]` still parses
and exports its legacy aggregate types, while the feature-on `#[composite]`
exports an inventory and does not emit those types.

Use trybuild fixtures for duplicate codes or aliases, duplicate field
inventory overrides, malformed or missing inventory paths, unsupported field
type inference, generated namespace collisions, and a logical-selector
collision. Pin useful spans where callback forwarding permits them. Reject
unexpected `#[composite(...)]` arguments under both feature configurations.

This phase is complete when a parent composite in another crate can collect a
child composite's complete inventory, preserve order and paired types, and
export it again; loadable boundaries and the feature-off legacy behavior still
pass their tests. Update the composite guide to describe the feature-gated
replacement and the `Composite` trait bound change. Run formatting, Clippy,
workspace all-target tests, doctests, and `hawkeye check` as specified in
`AGENTS.md`.

`#[instruction_set]`, selected parsers, operand resolution, dispatch, routes,
and bytecode opcode assignment remain outside this phase.
