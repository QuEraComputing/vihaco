# Dialect and component instruction inventory implementation plan

> **Scope:** `dialect!` and the new `#[component]` attribute only. Composite
> inventory propagation and `#[instruction_set]` consume this work later.
> **Protocol reference:** [structured-instruction-inventories.md](structured-instruction-inventories.md).
> This plan supersedes that proposal's `version` and `origin` inventory fields.

## Compatibility policy

Backward compatibility is not a requirement for this implementation. Change
existing macro output, hidden protocols, public behavior, callers, tests, and
documentation wherever the inventory design requires it. Replace the current
enumerator directly; do not retain a compatibility layer, dual exporter,
deprecated alias, or transition feature. Make dependent changes in the same
implementation so the workspace builds against one protocol.

## Intended result

Each `dialect!` declaration exports its complete, declaration-ordered inventory
through one hidden callback macro. `#[component]` reads the full inventories
of its selected dialects, resolves its `#[instructions { ... }]` selectors in
written order, checks `Execute<I>` coverage and duplicate canonical types,
and exports the resulting complete component inventory. Neither macro creates
a runtime instruction collection or changes the leaf instruction types.

For `arith::{Sub, Add}, boolean::*, arith::Halt`, the component export contains
`arith::Sub`, `arith::Add`, every `boolean` entry in declaration order, then
`arith::Halt`. Every entry retains its logical selector and runtime and
surface types. Its `source_steps` is empty.

## 1. Fixed inventory protocol

Every producer accepts the same request and invokes the named callback
**once** with its complete inventory:

```text
arith::__private::__entries! {
    callback: collect,
    state: { /* caller-owned tokens */ },
}

collect! {
    state: { /* unchanged caller-owned tokens */ },
    inventory: {
        entry {
            selector: { arith::Add },
            runtime: { Runtime0 },
            surface: { Surface0 },
            source_steps: {},
        },
        entry {
            selector: { arith::Halt },
            runtime: { Runtime1 },
            surface: { Surface1 },
            source_steps: {},
        },
    },
}
```

| Part | Contract and reason |
|---|---|
| `__entries!` | One generated request arm per producer, regardless of entry count. It forwards data instead of selecting names. |
| `callback` | The consumer's in-scope macro identifier. Declarative expansion invokes it to deliver inventory tokens. |
| Producer namespace | The consumer records a caller-visible path to the producer's hidden aliases once per collected inventory. An exported macro cannot infer the path used to invoke it, including imported names or nested modules. |
| `state` | Opaque tokens passed through unchanged by the producer. Generated collectors use them to carry earlier inventories to the finalizer. |
| `inventory` | The producer's full, ordered available list. Consumers own selection; there is no `select` request field or per-entry callback. |
| `entry` | One available instruction, with fields and trailing commas in the shown order. |
| `selector` | The canonical logical identity. Components preserve it; composites prepend field names. It is not assumed to resolve as a Rust type path. |
| `runtime` | The executable leaf type's bare alias name. The consumer qualifies it with the collected producer namespace. |
| `surface` | The matching leaf parser type's bare alias name, resolved the same way. |
| `source_steps` | Ordered composite field prefixes and accepted source aliases. It is empty for dialects and components; a composite prepends its own field step. |

There is no `version` field: this hidden protocol is an internal vihaco
implementation detail changed together with its producers and consumers.
There is no `origin` field: the final two segments of `selector` identify the
dialect leaf, entry position gives inventory order, and the surface parser
owns its mnemonic. The dialect declaration still validates duplicate names
and mnemonics; the inventory need not repeat that metadata.

**One shared helper owns this entire wire format.** Each producer supplies
typed entries and calls the same exporter builder. Dialect, component, and
future composite code must not each write their own `entry` token template,
alias declarations, or `__entries!` arm. The helper emits the aliases, the
complete `inventory: { ... }` response, the root-backed macro, and its
module-qualified re-export together. This keeps field order, punctuation,
path anchoring, and callback behavior identical at every level.

At later composite levels, `source_steps` records accepted source spellings
from outermost field to innermost field. For example:

```text
selector: { hydra::stream_a::core::cpu::Halt },
source_steps: {
    step { name: "hydra", aliases: { "h", }, },
    step { name: "stream_a", aliases: { "s", }, },
    step { name: "core", aliases: { "c", }, },
},
```

This entry accepts `hydra::stream_a::core::cpu.halt`, `h::s::c::cpu.halt`,
and mixtures of those prefixes. The dialect's surface parser handles
`cpu.halt`; it contributes no `source_steps` entry. The logical selector
always uses canonical field names, never source aliases.

## 2. Shared inventory construction and collector state

Add shared `Inventory`, `Entry`, and `SourceStep` types in
`crates/vihaco-runtime-derive/src/inventory/`. That module parses a complete
callback inventory into the typed model, validates missing or extra fields
and malformed paths, and renders the model back to protocol tokens. Its
exporter builder accepts the typed inventory and producer names and emits
all hidden aliases and the one-arm `__entries!` macro. The aliases point to
runtime and surface types resolved at the producer's definition site; the
macro response contains their bare names. The consumer pairs the response
with its known namespace and resolves the aliases before selection.

Shared operations select entries in requested order, concatenate inventories,
check logical selector collisions, and later prepend a composite field
selector and source step. Producer code supplies only its own inputs:
`dialect!` maps declarations into leaf entries, the component finalizer
chooses entries from collected dialect inventories, and a future composite
will supply field metadata. Selection syntax and `Execute<I>` coverage remain
component-specific; inventory construction and export do not.

The same module also owns a **collector-chain generator**. It accepts an
ordered list of producer namespaces and opaque
per-request tags, plus the finalizer to invoke. It emits the local collector
macros, unique callback names, the initial producer call, state forwarding,
and the finalizer call. Components use it for dialect selectors; future
composites use it for eligible child fields. Each macro supplies its own tag
data and interprets the collected records in its own finalizer. Neither macro
handwrites the callback chain.

The producer returns `state` without interpreting it. Each generated
collector captures the received `inventory` body as opaque token trees,
appends a record with its request tag to `state`, then invokes the next
producer:

```text
state: {
    record {
        namespace: { arith::__private },
        tag: { dialect: { arith }, selection: { Halt, Add } },
        inventory: { /* the complete arith inventory */ },
    },
}
```

The first producer receives `state: {}`. Each intermediate collector passes
all accumulated `collected` records to the next producer. The last collector
adds its own record and invokes the caller-specified procedural finalizer
with the original item, crate root, and collected inventories. Only that
finalizer parses entries and applies its macro-specific rules. This staging
supplies concrete expanded tokens to the procedural macro; a procedural
macro cannot inspect an earlier macro's output directly.

Keep parsing, selection, validation, and rendering as separate functions in
that module. Expose the hidden function-like component finalizer through the
runtime and facade path, so generated code does not require a direct
dependency on the derive crate.

Before conversion, prove the callback chain in a small cross-crate spike:
one dialect, an aliased import, `crate::` and `super::` qualified paths, and
an exported component inventory. Check that the shared exporter builder
anchors paths correctly from the generated sibling module and that selector
spans remain useful after forwarding.

## 3. Export passive dialect inventories

In `crates/vihaco-runtime-derive/src/dialect/`, replace the selection logic
in `enumerator.rs` with a full-inventory exporter. Keep runtime and surface
struct generation and declaration validation in `mod.rs` and `validation.rs`.

- Build an `Inventory` from the dialect declarations in declaration order and
  pass it to the shared exporter builder. The builder generates the one
  `macro_rules!` request arm and all entry tokens. Remove the old
  `select: { * }`, explicit-name, `@select`, and unknown-name arms.
- Keep the declared dialect and instruction names in each `selector`, even
  when a consumer reaches the dialect through an imported Rust alias.
- Have the shared builder emit hidden public runtime and surface aliases at
  the dialect site and return their bare names. The collector records the
  dialect namespace so its finalizer can qualify them. Component aliases are
  necessary because selected dialect
  paths may rely on imports visible only where the component was declared;
  the same builder gives dialects uniform exporter generation.
- Have the shared builder emit the root-backed macro and module-qualified
  `__entries!` re-export. Remove the current `__vihaco_instructions!`
  protocol and update all its in-repository callers and tests in the same
  change.

Test entry order, logical selectors, runtime/surface pairing, mapped payload
types, nested modules, imported aliases, and a downstream crate. Preserve
existing dialect declaration errors.

## 4. Collect and finalize component selections

Keep selector parsing and immediate syntax checks in
`component_attribute/selector.rs`. In `component_attribute/mod.rs`, pass the
authored selectors as ordered producer requests to the shared collector-chain
generator. Its tags contain each authored selection. The
generated callbacks accumulate the **full** dialect inventories and then
invoke the component finalizer.

The finalizer should:

1. Resolve `::*`, `::{A, B}`, and `::A` against the corresponding full
   inventory. Preserve written order for named selections and dialect order
   for wildcards. Report an unknown name at the selector's original span.
2. Concatenate selected entries in selector order without sorting or silent
   deduplication. Reject duplicate logical selectors, including different
   dialect declarations with the same declared name. Preserve each entry's
   canonical dialect selector and empty source steps; do not add a component
   prefix.
3. Emit `Execute<Runtime>` obligations for wildcard entries using the
   component's original generics and where clause. The first attribute
   expansion checks explicit selections through their authored Rust paths;
   this preserves rust-analyzer highlighting and navigation for resolved
   names, plus completion after an instruction name has been started.
4. Emit conflicting marker implementations keyed by canonical runtime type
   to reject semantic duplicates, including selections reached through
   different Rust aliases. Keep the existing early repeated-selector checks.
5. Pass the complete selected `Inventory` to the same exporter builder used
   by `dialect!`. It emits the hidden sibling namespace's definition-site
   aliases and passive `__entries!` exporter. The component remains a
   `Component`; it does not acquire `HasInstructionSet` or an aggregate enum.

The aliases in item 5 are required for cross-crate export. For example,
`#[instructions { a::Add }]` may refer to a local `use some_crate::arith as a`
visible where the component is declared but absent where another crate
invokes its exporter. `pub type Runtime0 = super::a::Add` resolves at the
component definition; the exported inventory sends `Runtime0` and the
consumer qualifies it with the component's namespace. An exported macro
cannot safely emit the local path `a::Add` at every downstream call site.

Move wildcard coverage and duplicate-type checks from
`component_attribute/coverage.rs` into the finalizer. Keep direct explicit
coverage in the first expansion, and remove the old descriptor-specific
callback grammar and independent per-selector enumerations.

## 5. Make export names usable across crates

Choose and test one hidden sibling namespace convention shared with the
future composite exporter, for example `__vihaco_inventory_cpu` beside `CPU`,
containing `__entries!` and the type aliases. The root backing macro name must
include the producer kind and identifier. Add
`#[vihaco(inventory_id = unique_ident)]` for same-named producers in different
modules; update the existing `#[vihaco(crate = ...)]` parsing so both arguments
can coexist. Reject duplicate or malformed overrides.

Document that importing or type-aliasing a producer does not make its hidden
namespace discoverable by reflection. Later consumers can supply an explicit
inventory namespace path. Test default names, an ID override, collisions,
crate-path overrides, and cross-crate expansion before calling this stage
complete.

## 6. Concrete change set

| Current code | Remove or replace | Add |
|---|---|---|
| [`dialect/enumerator.rs`](../crates/vihaco-runtime-derive/src/dialect/enumerator.rs) | Its selected-descriptor format, wildcard and explicit selection arms, per-instruction `@select` arms, and unknown-name fallback. | Build typed leaf entries and call the shared exporter builder; the builder emits the single full-inventory arm. |
| [`dialect/mod.rs`](../crates/vihaco-runtime-derive/src/dialect/mod.rs) | `__vihaco_instructions!` wiring and re-export. | Insert the shared builder's aliases, root-backed macro, and `__entries!` re-export into the generated dialect module. Keep its runtime/surface struct generation and declaration validation. |
| [`component_attribute/coverage.rs`](../crates/vihaco-runtime-derive/src/component_attribute/coverage.rs) | Independent per-selector enumerator calls and the callback grammar that matches every old descriptor field. | Keep direct explicit-path `Execute<I>` checks in the first expansion; move wildcard obligations and type-level duplicate checks into the finalizer. |
| [`component_attribute/mod.rs`](../crates/vihaco-runtime-derive/src/component_attribute/mod.rs) | The `expand_coverage_check` expansion path. | Emit direct checks for authored paths, feed selector requests to the shared collector-chain generator, emit the hidden component inventory namespace, and invoke the component finalizer. |
| [`component_attribute/selector.rs`](../crates/vihaco-runtime-derive/src/component_attribute/selector.rs) | No wire-format code belongs here. | Preserve authored-selector parsing and early syntax/overlap errors; provide spans and normalized requests to the collector generator. |
| New `inventory/` module | — | Own the typed inventory model, parser, transformations, path anchoring, alias generation, exporter builder, naming, and collector-chain generator. It alone writes protocol tokens and callback plumbing. |
| [`common.rs`](../crates/vihaco-runtime-derive/src/common.rs), [`lib.rs`](../crates/vihaco-runtime-derive/src/lib.rs), and [`vihaco-runtime/src/lib.rs`](../crates/vihaco-runtime/src/lib.rs) | `#[vihaco(...)]` parsing limited to `crate = ...`. | Parse `inventory_id`, expose the hidden component finalizer, and re-export it through the runtime root. |
| [`vihaco/tests`](../crates/vihaco/tests) | Tests and fixtures that invoke the old descriptor protocol. | Full-inventory, collector-chain, ordering, cross-crate alias, coverage, and diagnostic tests. |

## 7. Migrate tests and validate

- Test the shared inventory parser and exporter builder with an ordered entry
  list, empty and nonempty `source_steps`, aliases, and malformed fields. The
  dialect and component integration tests must exercise that same builder;
  there should be no producer-specific copy of the wire-format renderer.
- Test the shared collector-chain generator with two producers, repeated
  requests to one producer, opaque tags, and exact state forwarding. Check
  that its final callback receives both complete inventories in request order.
- Update `crates/vihaco/tests/dialect_macro.rs` to inspect the new complete
  exporter. Add a cross-crate consumer fixture for the hidden macro and type
  aliases.
- Extend `crates/vihaco/tests/component_attribute.rs` for multiple selectors,
  explicit order, wildcard order, alias and qualified paths, and generic
  components. Have a callback consume the component's exported inventory so
  the test verifies the export itself, not just `Execute<I>` coverage.
- Extend the `compile_fail` fixtures for unknown names, repeated selectors,
  semantic duplicate types, duplicate logical selectors, missing
  `Execute<I>`, malformed inventory data, name collisions, and malformed
  `inventory_id`. Update `.stderr` only for intentional diagnostic changes.
- Run `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace --all-targets`, `cargo test --workspace --doc`, and
  `hawkeye check`. Inspect a small rust-analyzer expansion and navigation case
  for an explicit selector and an exported type alias.

## Completion boundary

This phase is complete when dialect and component exporters use the same
full-inventory callback protocol; the component finalizer alone performs
selection; existing component coverage guarantees still hold; the old
enumerator is gone; and a consumer in another crate can read either inventory.
The shared inventory helper is the only code that emits aliases, `entry`
records, and the `__entries!` request arm; its collector-chain generator is
the only code that emits multi-producer callback plumbing. Every inventory
producer publishes
its own new `inventory: { ... }` through that protocol: the dialect publishes
declarations, the component publishes its selection, and a future composite
will publish eligible child entries with field selectors and source steps
prefixed. `#[instruction_set]` consumes the composite inventory without
replacing it. Composite lifting, authored instruction-set enums, and parser
generation are separate follow-on phases.
