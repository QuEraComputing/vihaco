# `#[instruction_set]` implementation plan

> **Status:** implemented; the source syntax was revised to keep concrete
> runtime types in the authored enum.
> **Scope:** authored runtime and surface instruction enums, selected parsing,
> diagnostics, and the composite's `HasInstructionSet` association.
> **Depends on:** the implemented dialect, component, and composite inventory
> exporters. See [structured instruction inventories](structured-instruction-inventories.md),
> [composite instruction lifting](composite-instruction-lifting.md), and the
> [composite inventory plan](composite-inventory-implementation-plan.md).
> **Related exploration:** [type-tree inventories](instruction-inventory-type-tree.md).

## Intended result

An author chooses a subset of a composite's available instructions and gives
each choice a public variant name:

```rust
#[instruction_set(composite = Hydra)]
pub enum HydraInstruction {
    #[selector(stream_a::core::cpu::ConstI64)]
    LoadA(cpu::ConstI64),
    #[selector(stream_b::core::cpu::ConstI64)]
    LoadB(cpu::ConstI64),
}
```

The tuple payloads are concrete Rust instruction types. The `#[selector]`
attributes identify logical routes through the composite; those paths do not
have to resolve as Rust types. `#[instruction_set]` consumes the selector
attributes and produces, conceptually:

```rust
pub enum HydraInstruction {
    LoadA(cpu::ConstI64),
    LoadB(cpu::ConstI64),
}

pub enum HydraSurfaceInstruction {
    LoadA(cpu::syntax::ConstI64),
    LoadB(cpu::syntax::ConstI64),
}

impl HasInstructionSet for Hydra {
    type Runtime = HydraInstruction;
    type Syntax = HydraSurfaceInstruction;
}
```

The author-named variants distinguish `stream_a` from `stream_b` even though
both carry `cpu::ConstI64`. The macro checks each concrete payload type
against the selected inventory entry at compile time.

`HydraSurfaceInstruction::parser()` accepts the selected source forms only.
For example, the `LoadA` branch consumes `stream_a::core::` or the declared
aliases for those fields, then calls the dialect's `ConstI64` surface parser.
The parser returns `LoadA` regardless of which source alias was used. The
`#[loadable]` fields of `Hydra` are absent from its inventory and cannot be
selected through this attribute.

## Current foundation

The shared `Inventory`, `Entry`, `SourceStep`, `collect`, and
`CollectedInventory::resolve` code already lives in
`crates/vihaco-runtime-derive/src/inventory/mod.rs`. An entry contains exactly
`selector`, `runtime`, `surface`, and `source_steps`; it has no `version` or
`origin` field. A producer's hidden `__entries!` macro returns its complete,
ordered inventory once to a callback. The collector records the producer
namespace so the finalizer can qualify its `RuntimeN` and `SurfaceN` aliases.

With `composite-inventory` enabled, `#[composite]` exports the concatenated
inventories of its non-loadable device fields. The `vihaco` crate currently
enables that feature by default. In this mode, `Composite` does not imply
`HasInstructionSet`, and `#[composite]` does not create executable aggregate
enums. The authored instruction set will supply that association. The legacy
feature-off composite expansion still owns its generated enums and
`HasInstructionSet` implementation; `#[instruction_set]` must be available
only with the inventory expansion to avoid conflicting implementations.

The available inventory stays complete after an authored set is declared. A
parent composite may lift a child's available entries independently of the
child's selected executable set.

## Author-facing contract

### Attribute and selector grammar

The required argument is `composite = TypePath`. An optional
`inventory = NamespacePath` points directly to the composite's hidden
inventory namespace when the composite type is an imported rename, type
alias, or uses a non-default inventory ID:

```rust
#[instruction_set(
    composite = ImportedHydra,
    inventory = crate::hardware::__vihaco_inventory_hydra,
)]
enum HydraInstruction {
    #[selector(stream_a::core::cpu::ConstI64)]
    Load(cpu::ConstI64),
}
```

For a direct composite type path, derive the sibling namespace by replacing
the terminal type identifier with the shared `__vihaco_inventory_*` name.
Use the same relative-path rebasing rules as the component and composite
collectors. The override changes inventory lookup only; it does not change
selector identity or accepted source text. Continue to honor
`#[vihaco(crate = ...)]` for a re-exported framework root.

The initial variant grammar is one tuple field containing a Rust type path
and one `#[selector(path)]` attribute. The selector's segments are canonical
composite field names followed by the dialect and instruction identifiers.
Reject unit, named-field, multi-field, parenthesized, and reference payloads;
reject qualified or generic selector paths.
Reject an empty authored enum until the intended `Parse` behavior for an empty
set is specified. Preserve the enum's visibility, author variant names,
documentation, and supported derives. Define and test the policy for variant
`#[cfg]` attributes before accepting them: the runtime enum, surface enum,
and parser branches must be enabled together.

The selector attribute is consumed at expansion time. A field alias is accepted in
source text but is not a selector segment. Variant names are independent of
selector names and may select the same canonical leaf type through different
field paths. Selecting the same full logical path twice is an error.

### Generated API

- Keep the runtime enum at its declaration site with the authored visibility,
  variant names, and concrete runtime payloads. This gives editor hovers the
  actual dialect instruction struct and keeps the enum's public path stable.
- Expose a matching flat surface enum with the authored enum's visibility,
  provisionally named by replacing its trailing `Instruction` with
  `SurfaceInstruction`. Pin the final naming rule with a public API compile
  test, including a runtime enum without that suffix.
- Implement `Parse<'src>` and `SurfaceInstruction` for the surface enum.
- Implement `HasInstructionSet` for the specified composite, with `Runtime`
  and `Syntax` associated with those two enums.
- Do not implement `Instruction`/bytecode traits or generate routing,
  conversion, operand resolution, or dispatch in this phase.

The author converts each surface variant to its runtime variant while
resolving operands with the relevant module context, then matches the runtime
variant to choose a device and call `Execute`. There is no blanket `From`
conversion because symbol lookup, interning, and source expansion can require
context and can fail.

## Expansion architecture

### Stage 1: parse and emit the authored declaration

Add an `instruction_set` attribute entry point to
`crates/vihaco-runtime-derive/src/lib.rs`, and re-export it through `vihaco-runtime`
and the `vihaco` facade. Put parsing, validation, and code generation in a
dedicated `instruction_set/` module; reuse the shared inventory code rather
than adding another wire format.

Parse the attribute arguments and an `ItemEnum`. Validate the variant shapes
and store ordered `(variant name, selector path, payload type, attributes)`
records. Keep the original selector and payload tokens in the context passed to the finalizer so its
diagnostics can point at authored paths. Generate one request for the
composite's **complete** inventory through `inventory::collect`. The request
has an empty tag because there is one producer; the finalizer receives the
ordered variant records and resolved producer response.

Emit the runtime enum at the authored site after removing `#[selector]`
attributes. The collector and finalizer run inside a hidden sibling module.
The finalizer checks that each authored payload type equals the selected
inventory runtime type and emits the surface enum. Re-export the surface
enum with the authored visibility and `#[doc(inline)]`, then emit the
`HasInstructionSet` implementation at the authored site. Runtime hovers and
Rust documentation display the real Rust type paths written by the author.

Keep hidden naming distinct from the composite's inventory namespace. Let
ordinary Rust duplicate-name errors diagnose collisions with neighboring
items, and pin those errors in compile tests. If the user requests a renamed crate
root, rebase that path when invoking the finalizer inside the sibling module.

### Stage 2: select and finalize

Add a hidden `__instruction_set_finalize!` procedural macro. Parse the
collector's `CollectedInventories`, resolve the composite's bare payload
aliases with `CollectedInventory::resolve`, and require exactly one producer
record. Build a map keyed by the **full** logical selector; do not compare
only the final instruction identifier and do not deduplicate equal runtime
types. Iterate the authored variants in declaration order:

1. Validate that the selector exists in the composite inventory.
2. Reject a repeated full selector, reporting the later occurrence.
3. Check that the authored tuple payload type matches the selected runtime
   type, and emit a `SurfaceN` alias for the paired surface type.
4. Emit a surface enum variant with the authored name and its `SurfaceN`
   payload.
5. Emit a parser branch using that entry's `source_steps` and leaf surface
   parser.

Index `N` by authored variant position, not by inventory position. This keeps
the surface enum and finalizer output aligned even when selection order
differs from inventory order. Use the existing inventory parser and
alias resolution; neither the macro nor a callback should parse the
producer's entry syntax by hand.

### Selected parser

Each entry's `source_steps` is ordered outermost to innermost. At every step,
accept the canonical name and its aliases followed by `::`; then delegate to
`<SurfaceN as Parse<'src>>::parser()`. Map the result into the corresponding
surface enum variant. Compose alternatives from selected entries only, in
authored order. Compose aliases within each step rather than enumerating all
cross-products of aliases across nested fields.

Use the `vihaco` or `vihaco-runtime` re-export of `chumsky`, following the
existing legacy composite parser's `Parse` signature. Choose a parser
composition that handles arbitrary variant counts without a fixed-size tuple
limit. Verify backtracking and end-of-instruction behavior for prefix-related
mnemonics such as `halt` and `halt_all`; parser order must not silently make a
selected entry unreachable. A branch must consume the whole source spelling
when parsed as a standalone instruction.

The parser calls the leaf parser, not a child's instruction-set parser. This
keeps a parent's selected subset independent of a child's authored set and
prevents unselected child instructions from entering through a broad child
parser.

## Diagnostics

Use `syn::Error` at the authored attribute, enum, variant, or selector span
when available. Add trybuild fixtures for at least:

| Input | Expected diagnosis |
| --- | --- |
| Attribute on a non-enum item | `instruction_set` requires an enum. |
| Missing or repeated `composite`, unknown argument, malformed `inventory` | Invalid attribute argument at that argument. |
| Missing or repeated selector attribute | Expected exactly one `#[selector(path)]`. |
| Unit, named-field, multi-field, or non-path variant | Expected one runtime instruction type path in a tuple variant. |
| Payload type differs from the inventory entry | Compile-time type mismatch at the authored payload. |
| Selector using a source alias | Unknown canonical selector; point to the full authored path. |
| Unknown leaf or unavailable component capability | Selected path is not in the composite inventory. |
| Path behind a `#[loadable]` boundary | Not selectable from this composite. Distinguish it where available metadata permits; otherwise state that the path is absent from the inventory. |
| Repeated full selector | Duplicate selection at the later variant. |
| Missing or inaccessible inventory namespace | Ordinary Rust path error at the generated request, with an attribute-level hint when namespace inference is impossible. |
| Generated surface or hidden-module name collision | A compile error with the colliding name. |

The current inventory entries have no origin metadata. The finalizer can
identify an absent full path but cannot always know whether the cause is an
unknown dialect instruction, a component's unselected capability, or a
loadable boundary. Do not promise distinctions that require data absent from
the protocol. If precise diagnostics become a requirement, add the minimum
metadata to the shared producer contract and test it across all exporters.

## Generic types and attributes to settle in the first spike

The composite inventory is fixed by its declaration, even for a concrete
generic field such as `Cpu<T>`, but an authored enum and a target composite
can have separate generic parameters and bounds. The macro cannot inspect
the composite declaration when it sees `composite = Hydra`. Before claiming
blanket generic support, compile a small example covering a generic
composite with bounds, a concrete instantiation, and the desired
`HasInstructionSet` implementation. Decide the authored syntax for passing
impl generics and where-clauses, or document a first-version restriction to
concrete target types. Avoid adding unused enum parameters or a hidden enum
variant merely to carry generic parameters.

Similarly, verify how Rust forwards authored enum and variant attributes
through an attribute macro. Preserve `doc`, `derive`, and visibility where
their behavior is clear. Mirror any accepted `cfg` onto the surface enum and
parser branches, or reject it with a targeted diagnostic. Do not emit a parser
branch for a variant absent from either enum.

Inspect rust-analyzer completion and go-to-definition on a selector path and
on the concrete payload type. Selectors remain macro metadata rather than
Rust paths, but the payload is ordinary Rust and should retain its normal
editor support. Use compile tests and an editor probe to pin the public enum
spelling and hover behavior.

With rust-analyzer 0.3.3065, `LoadA(leaf::cpu::ConstI64)` appears in the
variant hover, and hovering or navigating from `ConstI64` reaches the dialect
struct. The enum usage hover has its authored module path. The logical
selector inside `#[selector(...)]` still has no semantic tokens, hover,
completion, or go-to-definition; that remains separate editor work.

The proposed [type-tree inventory design](instruction-inventory-type-tree.md)
records experiments with first-expansion `pub use` trees, imported dialect
aliases, nested composites, same-named dialects, and rust-analyzer hover and
navigation. It also identifies the remaining generated variant-hover and
instruction-set staging questions. The current `#[selector]` plus concrete
payload form remains the implemented author-facing API.

## Implementation steps

1. **Spike the staged output.** Prove that a runtime enum at the authored
   site can keep concrete payload types while the hidden finalizer checks
   them against selected inventory types. Prove the public surface re-export, `HasInstructionSet` impl, and
   one selected parser branch in both the facade and direct runtime crates.
   Settle the generic target syntax and generated surface name here.
2. **Add input parsing and selection.** Validate the attribute and enum
   grammar, collect the composite's full inventory, resolve alias paths, and
   select full logical paths in author order. Keep duplicate and unknown-path
   checks in the finalizer.
3. **Generate surface parsing.** Emit the flat surface enum, nested source
   prefix parsers, leaf parser delegation, and `SurfaceInstruction` impl.
   Test selected-only parsing and alias combinations.
4. **Add end-to-end usage.** Parse through two composite levels, resolve an
   operand explicitly, and manually dispatch two author variants that share
   one canonical leaf type through sibling fields.
5. **Migrate documentation and compatibility tests.** Update the component,
   composite, parser, and instruction guides and their compiled examples.
   Keep feature-off legacy tests isolated while that expansion exists.

## Verification and completion criteria

Integration tests should cover selection order independent of inventory
order; two sibling fields with the same leaf payload; field aliases at two
levels; omitted instructions failing to parse; `#[loadable]` exclusion;
module-qualified and cross-crate composites; `inventory = path`; custom
`#[vihaco(crate = ...)]`; a selected leaf with operands; a unit leaf; and a
source mnemonic prefix pair. Include a generic-composite case according to
the syntax chosen in the first spike. Compile-fail tests should pin variant
grammar, duplicate and unknown selectors, missing namespaces, and generated
name collisions.

Completion means the public enums and association compile in another crate,
the parser accepts exactly the selected forms, and the manual-dispatch example
executes the intended sibling device. Run `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace --all-targets`, `cargo test --workspace --doc`, and
`hawkeye check` before merging the implementation.

## Deferred work

- Generated routes, work mappings, and execution dispatch.
- A blanket surface-to-runtime conversion or operand resolver.
- Stable bytecode opcodes for authored variants, particularly two variants
  with the same canonical leaf payload type.
- Arbitrary child-provided parser endpoints beyond source prefix steps and a
  dialect leaf parser.
- Removal of the feature-off legacy composite aggregate expansion after its
  consumers have migrated.
