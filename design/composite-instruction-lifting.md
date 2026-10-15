# Composite instruction lifting

> **Status:** proposal; implementation planned. The inventory protocol and
> current tuple-selector syntax are specified in
> [structured-instruction-inventories.md](structured-instruction-inventories.md).
>
> **Related:** [issue #118](https://github.com/QuEraComputing/vihaco/issues/118),
> `../vihaco-static-top-experiment/design/dialect-macro.md`,
> `../vihaco-static-top-experiment/design/instruction-selection.md`, and
> `../vihaco-static-top-experiment/design/route-export.md`

## Goal

Let a composite export the instructions available through its fields. Each
field prefixes its child's logical selectors and accepted source syntax while
retaining the child's canonical runtime and surface payload types. An author
selects entries by writing their full logical paths as tuple payloads in an
instruction-set enum and chooses the names of its variants.

Instruction composition and parsing must work before route generation exists.
The author resolves surface operands and dispatches runtime instructions with
ordinary Rust. Routes may later generate dispatch for the same authored enum.

## Example

Assume a `cpu` dialect declares `ConstI64`, a `CPU` component selects it, a
`Stream` composite contains `core: CPU`, and a `Hydra` composite contains
`stream_a: Stream` and `stream_b: Stream`.

| Level | Available inventory selector | Accepted source text |
| --- | --- | --- |
| Dialect | `cpu::ConstI64` | `cpu.const_i64 1` |
| `Stream` | `core::cpu::ConstI64` | `core::cpu.const_i64 1` |
| `Hydra` | `stream_a::core::cpu::ConstI64` | `stream_a::core::cpu.const_i64 1` |

Authors select an entry using its logical path as the variant's single tuple
payload:

```rust
#[instruction_set(composite = Hydra)]
pub enum HydraInstruction {
    LoadStreamA(stream_a::core::cpu::ConstI64),
    LoadStreamB(stream_b::core::cpu::ConstI64),
}
```

The payload paths are inventory selectors consumed by the attribute macro;
they need not resolve as Rust types. The macro replaces each with the
inventory's canonical runtime type and generates the corresponding surface
variant. `LoadStreamA` and `LoadStreamB` remain distinct even though both
ultimately contain `cpu::ConstI64`.

The logical selector path uses canonical field names followed by a dialect
instruction name. It is not a Rust path to a generated wrapper type. A field
alias may add an accepted spelling in source text, but it does not change the
selector path or the resulting surface value.

## Responsibilities by layer

### `dialect!`: leaf entries

The existing dialect macro generates a canonical runtime struct, a surface
struct with its `Parse` implementation, and an instruction enumerator. The
dialect owns the leaf syntax, including its dialect head. For example,
`cpu::syntax::ConstI64::parser()` accepts `cpu.const_i64 1`.

The current dialect enumerator's descriptor already carries the instruction
identifier, runtime type, surface type, mnemonic, and declaration ordinal.
The planned passive inventory exporter preserves that pair and its syntax
when it passes through later layers.

### `#[component]`: selected capabilities

The component macro already resolves `#[instructions { ... }]` and checks one
`Execute<I>` obligation for each selected canonical instruction. Extend it to
export the complete ordered selection through a component inventory. Its
entries refer to the dialect's original runtime and surface structs: the
component does not wrap them or prepend a component-name prefix.

Explicit selections retain written order; wildcards retain dialect declaration
order. Existing overlap, duplicate-type, and unknown-name checks remain in
force. The exported inventory should reuse the same resolved descriptors as
the coverage checks, rather than independently reconstructing paths.

### `#[composite]`: prefix child inventories

For each eligible field, the composite prepends its canonical field name to
every child selector and records the field name and any source aliases as a
source prefix step. It retains the child's canonical runtime and surface
types and leaf parser. Repeating this through another composite produces a
longer logical selector and an ordered sequence of source prefixes, with no
per-entry wrapper types or parsers.

The composite exports its complete ordered inventory even without an authored
instruction set. A parent consumes this export exactly as it consumes a
component's export. A procedural macro cannot inspect the items produced by
another macro, so the callback protocol carries each producer's full
inventory into a procedural finalizer for recursive and cross-crate use.

Following issue #118, `#[loadable]` fields are separate section boundaries and
are excluded from automatic lifting into the parent inventory. Their current
bytecode and SST section-loading behavior remains available.

### `#[instruction_set]`: authored public choices

The author writes an enum and names its variants. Each variant has exactly
one tuple payload containing the full logical selector path. The
instruction-set macro resolves that selector against the associated
composite's inventory and generates:

- one runtime enum variant containing the selected canonical runtime type;
- one matching flat surface enum variant containing its surface type;
- a parser for the surface enum that combines only selected branches, each
  consuming the recorded field prefixes and then calling the leaf parser;
- `SurfaceInstruction` support for the generated surface enum;
- the composite's `HasInstructionSet` association; and
- clear compile errors for unknown, unavailable, or repeated selectors.

The flat enums preserve route choice through their author-named variants.
The parent parser does not invoke a child's whole instruction-set parser when
it needs only one child entry. Instructions absent from an authored enum must
not parse as instructions of that enum. Unit, named-field, multi-field, and
non-path payload variants are invalid in the initial grammar.

No `InstructionForm` trait or universal surface-to-runtime conversion is
needed. Inventories carry the paired types, and each surface type carries its
own parser through the existing `Parse` trait.

### Author-owned resolution and dispatch

The author converts a parsed surface enum into the runtime enum, resolving
operands with whatever module context is required while preserving the chosen
variant. A blanket `From` conversion would not cover symbol resolution,
interning, or source expansions, so this remains explicit.

The author then matches on the runtime enum to construct messages, execute
components, and handle effects or work. This first implementation does not
require route functions, work mappings, or generated dispatch. A future route
attribute may use the same enum to generate the match.

## Implementation sequence

1. **Specify the inventory protocol.** Define a common descriptor shape for
   component and composite exports, including full logical paths, runtime and
   surface types, and deterministic order. Specify selection, callback, and
   unknown-path diagnostics. Test module-qualified and cross-crate invocation.
2. **Extend `#[component]`.** Export its selected dialect descriptors while
   retaining the existing `Execute<I>` checks. Use a callback chain or
   equivalent expansion so multiple selectors form one complete inventory.
3. **Export composite inventories.** Prefix child selectors and source steps
   and export the result. Preserve generic parameters and where-clauses. Keep
   `#[device]` metadata, aliases, and `#[loadable]` section delegation working.
4. **Implement `#[instruction_set]`.** Resolve each authored tuple payload
   selector against the composite inventory and replace it with the selected
   canonical runtime type. Generate the surface enum, selected parser
   alternatives, and `HasInstructionSet` association. Keep author variant
   names independent of selector paths.
5. **Add an end-to-end manual-dispatch example.** Parse through two composite
   levels, resolve operands, and use a handwritten match to execute the chosen
   field. Use the same leaf instruction through two sibling fields.
6. **Migrate documentation and compatibility tests.** Update the composite,
   component, and parser guides and their doctests. Preserve the existing
   generated composite instruction modules during an opt-in transition, then
   decide when the authored-set behavior becomes the default.

## Validation

Cover explicit and wildcard component selections; duplicate identifiers from
different dialects; repeated child types in sibling fields; recursive source
prefixes and field aliases; a selected subset of available entries; loadable
fields; generic composites; and cross-crate use. Trybuild fixtures should pin
errors for malformed or unknown tuple selectors, invalid variant shapes,
duplicate selections, missing child inventories, and conflicting generated
names or source prefixes. Parser tests should also cover mnemonics with a
prefix relationship, such as `halt` and `halt_all`, so parser ordering does
not change accepted source text.

Before completion, run formatting, Clippy, all-target tests, doctests, and the
SPDX header check described in `AGENTS.md`.

## Deferred work

- **Routes and work:** optional generated dispatch over the authored enum.
- **Bytecode encoding:** assign stable opcodes to authored variants, including
  variants that share a canonical leaf instruction. The current ABI derive
  does not establish this policy for lifted entries.
- **Exact generated path layout:** validate hidden payload alias paths and IDE
  behavior through compile tests. Public enum variant names remain author-owned.
