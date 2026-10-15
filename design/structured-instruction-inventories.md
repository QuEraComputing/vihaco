# Structured instruction inventories

> **Status:** proposal; not implemented
> **Scope:** `dialect!`, the new `#[component]`, `#[composite]`, and a future
> `#[instruction_set]`
> **Related:** [composite-instruction-lifting.md](composite-instruction-lifting.md),
> `../vihaco-static-top-experiment/design/instruction-selection.md`, and
> `../vihaco-static-top-experiment/design/route-export.md`

## 1. Decision

Each declaration exports an ordered **instruction inventory** through a small,
hidden callback macro. An inventory is compile-time token data, not a runtime
collection. Producers always return their complete inventory. A procedural
finalizer selects entries, validates them, and emits the next inventory or the
authored instruction set. Callbacks carry opaque tokens; they do not parse each
entry or generate one Rust item per entry.

Until route generation needs otherwise, composites identify a lifted entry by
its logical path in the inventory. They do **not** generate a runtime or surface
newtype for each lift. An authored instruction-set enum provides the distinct
runtime and surface variants. Consequently, generated field or entry marker
types do not appear in the user-facing instruction payloads.

The spelling of hidden modules and macro entry points below is proposed. The
inventory semantics and expansion boundaries are the important contract to
test before settling their exact names.

## 2. Why change the current plan?

The current `dialect!` enumerator carries useful structured descriptors, but
also generates a matching arm for every instruction to implement explicit
selection. The new `#[component]` callback matches every descriptor field and
immediately emits coverage checks. Extending that approach through composites
would add more generated `macro_rules!` arms, callback grammars, and per-entry
types at every level. It would also spread selection rules across producer
macros.

An earlier iteration of composite lifting called for one runtime newtype and
one surface newtype per field and child entry. Those types preserve a route
choice, but the first implementation already has an authored flat enum whose
variants preserve that choice. Two variants can carry the same canonical
instruction type and still remain distinct through parsing, resolution, and
manual dispatch. The newtypes therefore do not earn their public type
complexity in this phase.

Moving selection into finalizers gives the implementation one place to check
paths, ordering, duplicates, and diagnostics. Passive inventories reduce the
generated macro body to data forwarding. Avoiding per-entry lift types reduces
the number of generated Rust items visible in IDE expansions.

## 3. Terms and invariants

- A **leaf** is an instruction declared by a dialect. Its runtime and surface
  types and its leaf parser belong to that dialect.
- A **capability** is a leaf selected by a component. Selection does not create
  another instruction type or source prefix.
- A **lifted entry** is one capability available through a particular sequence
  of non-loadable composite fields. Its logical selector includes those field
  names. Its payload types remain the leaf types.
- An **instruction-set choice** is one authored enum variant selecting one
  lifted entry by writing its logical selector as the variant's single tuple
  payload. This variant is the runtime route identity in this phase.
- A **source step** is one composite field's accepted source prefixes: its
  canonical name and any aliases. It is separate from the logical selector.

Aliases are optional and are not required for inventory identity or route
selection. They appear in this proposal to preserve the existing
`#[device(..., alias = "...")]` source syntax: the current composite parser
accepts aliases as alternatives to a field-name prefix. A field without an
alias contributes only its canonical name. Rust import or type aliases are a
different issue, handled by inventory path resolution in section 8.

The inventory is ordered. Dialect entries follow declaration order. A
component's explicit selections follow written order; a wildcard contributes
dialect declaration order; selector groups concatenate in written order. A
composite walks eligible fields in declaration order and preserves each
child's inventory order. No layer silently sorts or deduplicates entries.

Logical selectors use canonical field names and the declared dialect name,
not a field alias, imported Rust alias, generated module name, or public enum
variant name. Two entries may have equal runtime and surface types but must
have different logical selectors. A component still rejects selecting the
same canonical runtime type twice; a composite may expose the same leaf
through different field paths.

### Inductive composition

Every composite publishes an inventory in exactly the same protocol that a
parent composite consumes. Let `I(X)` be the ordered inventory of `X`. Then:

```text
I(dialect)   = its declared leaf entries
I(component) = its ordered selection from dialect inventories
I(composite) = concatenate, in field order, for each non-loadable field f:
               prefix(f, I(type_of(f)))
```

`prefix` adds `f` to each logical selector and adds the field's accepted
source spellings to each entry's source steps. It leaves the canonical
runtime and surface payload types and leaf parser unchanged. The result is a
complete inventory that the composite exports even if it has no authored
`#[instruction_set]`. This is the induction step: a parent at level `n + 1`
uses a child composite's exported `I(child)` at level `n` in the same way
that the child used its own children.

For example, `Stream` exports `core::cpu::ConstI64`; `Hydra` exports
`stream_a::core::cpu::ConstI64`; and a `Fleet` containing `hydra: Hydra`
exports `hydra::stream_a::core::cpu::ConstI64`. Each level contributes one
source prefix and one logical selector segment. A `#[loadable]` field stops
this propagation into its parent but retains its own inventory for its own
section.

This proposal distinguishes the **available capability inventory** from an
**authored instruction-set selection**. `#[instruction_set]` selects from
`I(composite)` to make that composite's runtime and surface enums; it does
not replace `I(composite)` or limit what a parent may lift. This keeps
composition independent of whether a child has authored an executable enum.
If the intended rule is instead that a parent may lift only entries advertised
by the child's authored enum, the protocol needs a second, post-selection
export and a rule for children without an authored set. That is a different
visibility policy and should be decided before implementing nested selection.

## 4. User-facing example

```rust
dialect! {
    cpu {
        ConstI64(i64),
        Halt,
    }
}

#[component]
#[instructions { cpu::{ConstI64, Halt} }]
struct CPU;

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

#[instruction_set(composite = Hydra)]
pub enum HydraInstruction {
    LoadStreamA(stream_a::core::cpu::ConstI64),
    LoadStreamB(stream_b::core::cpu::ConstI64),
}
```

The tuple payloads above are inventory selectors, not Rust type paths that
must resolve at the call site. `#[instruction_set]` consumes each selector and
replaces it with the inventory's canonical runtime type. The expanded enums
are conceptually:

```rust
pub enum HydraInstruction {
    LoadStreamA(cpu::ConstI64),
    LoadStreamB(cpu::ConstI64),
}

pub enum HydraSurfaceInstruction {
    LoadStreamA(cpu::syntax::ConstI64),
    LoadStreamB(cpu::syntax::ConstI64),
}
```

The two variants retain the chosen field path. The generated parser for
`LoadStreamA` accepts `stream_a::core::cpu.const_i64 1` and
`stream_a::c::cpu.const_i64 1`; it produces the same surface variant for both
spellings. The `stream_b` branch produces `LoadStreamB`. The loadable
`auxiliary` field contributes no entry to Hydra's inventory.

The actual expansion may spell a payload through a hidden public type alias
that resolves to `cpu::ConstI64` or `cpu::syntax::ConstI64`. These are aliases,
not distinct wrapper types, but an IDE may show the alias path in a type hint.
The first spike should inspect that output and whether completion and
go-to-definition remain useful for selector paths that are consumed before
Rust name resolution.

An author resolves operands and dispatches by matching these variants. For
example, `LoadStreamA(value)` targets `hydra.stream_a.core`; no type-level
field marker is needed to recover the choice. The authored tuple selector is
the compile-time declaration of that mapping; it does not survive in the
expanded enum's payload type.

## 5. Inventory data

The wire format uses delimited token fields so a callback can carry an entry
without interpreting it. A dialect inventory response is conceptually:

```text
inventory: { version: 1,
    entry {
        selector: { cpu::ConstI64 },
        runtime: { Runtime0 },
        surface: { Surface0 },
        source_steps: {},
        origin: { dialect: cpu, instruction: ConstI64,
                  mnemonic: "const_i64", ordinal: 0 },
    },
}
```

`runtime` and `surface` are bare type alias names. The consumer records the
producer's namespace beside the inventory and qualifies those names before
using them. That module exports hidden type aliases to
anchor the types at the producer's definition site. This avoids sending a
local `use` alias or a relative type path into another crate, where it might
resolve differently. A dialect may use its own public instruction module as
the namespace; component and composite exporters need a generated sibling
module.

`source_steps` is an ordered sequence from outermost field to innermost field.
Each step contains one canonical field name and zero or more source aliases.
The dialect's `surface` type owns the remainder of the syntax through its
existing `Parse` implementation, including the dialect head and mnemonic.
Thus `#[instruction_set]` can construct a branch by consuming each step's
`name::` or `alias::`, then invoking `<Surface as Parse>::parser()`.

The component finalizer selects descriptors but does not change their
selector, types, source steps, or origin. The composite finalizer prepends
one field segment to `selector` and one field prefix group to `source_steps`.
For example:

```text
CPU:    cpu::ConstI64
Stream: core::cpu::ConstI64
        source_steps: [{ name: "core", aliases: ["c"] }]
Hydra:  stream_a::core::cpu::ConstI64
        source_steps: [{ name: "stream_a", aliases: [] },
                       { name: "core", aliases: ["c"] }]
```

The finalizer parses this token grammar into an internal Rust representation
using `syn` and typed structs. It validates the schema version and required
fields. Callbacks treat the entire `inventory` body as `tt` tokens. They do
not embed copies of the descriptor grammar.

The v1 grammar supports prefixes plus the dialect leaf parser. If a later
feature lets a child define arbitrary per-entry syntax, add a parser endpoint
to a new protocol version rather than assuming its syntax can always be
flattened into prefix steps.

## 6. Callback protocol and expansion order

A producer accepts a callback name and opaque state. It invokes the callback
exactly once with all of its entries:

```rust
producer::__entries! {
    callback: __vihaco_collect_0,
    state: { /* opaque caller tokens */ },
}

// What the producer emits:
__vihaco_collect_0! {
    state: { /* the same caller tokens */ },
    inventory: { version: 1, /* all entries */ },
}
```

There is no `select` argument. An unknown named selection is diagnosed by the
consumer finalizer, which has both the authored selector and the full source
inventory. A producer does not call the callback once per entry: finalizers
need the whole ordered list to generate complete items.

For several dialects or fields, `#[component]` or `#[composite]` generates a
short callback chain. Each callback appends the received inventory, its
producer namespace, and the selector or field that requested it to opaque
state and invokes the next producer. The last callback invokes a hidden function-like procedural
macro such as `__finish_component!` or `__finish_composite!`:

```text
first producer -> collector 0 -> second producer -> collector 1
               -> procedural finalizer -> generated inventory exporter
```

These are one callback per **producer**, not per instruction. The finalizers
share the same inventory parser and internal entry model, while keeping their
component, composite, and instruction-set validation separate. This avoids a
large procedural macro with a `mode` switch.

This staged expansion is necessary. A procedural macro cannot inspect Rust
items produced by an earlier macro, nor can it receive a nested producer
invocation and require that invocation to expand before it reads its input.
The declarative callback chain gives the finalizer concrete descriptor tokens.

Component and composite collectors can live inside each declaration's hidden
sibling module so their names do not pollute the author's module. The exported
inventory macro is technically public for cross-crate expansion, but is
`#[doc(hidden)]` implementation plumbing, not a supported user-facing API.
The author-facing public types are the dialect leaf types and the authored
instruction-set enums.

## 7. Responsibilities by stage

### 7.1 `dialect!`

`dialect!` continues to generate one canonical runtime struct and one
independently parsable surface struct per instruction. It generates a passive
full-inventory exporter instead of selection arms. Each entry includes the
declared dialect and instruction identifiers, original ordinal and mnemonic,
and paired bare type aliases that the finalizer resolves through the
collected namespace.

Dialect validation for duplicate instruction names and mnemonics remains at
the declaration. The dialect exporter neither selects names nor performs
`Execute` checks.

### 7.2 `#[component]`

The initial attribute expansion parses and removes `#[instructions { ... }]`,
validates syntax and obvious repeated selectors, and emits calls to the
referenced dialect inventories. It checks explicit selections through direct
Rust type paths so rust-analyzer can highlight and navigate resolved names
and complete a partially typed instruction name. An empty slot in an
incomplete attribute still needs error-tolerant selector parsing.

After collection, `__finish_component!` resolves every selector against its
source dialect's full inventory. It concatenates selected entries in authored
order, checks unknown names, and generates `Execute<I>` obligations for
wildcard entries under the component's original generic bounds. Explicit
obligations come from the first attribute expansion.
Semantic duplicate types, including those reached through Rust aliases,
remain guarded by conflicting marker implementations as in the current
component implementation; procedural token comparison alone cannot prove
type equality. The finalizer exports the normalized selected inventory.

The component does not add a source prefix, wrap payloads, or generate an
aggregate instruction enum.

### 7.3 `#[composite]`

The initial attribute expansion retains the composite's device-code and
alias validation and its section-loading behavior. It requests inventories for
eligible fields. A `#[loadable]` field is excluded from the parent's
inventory, even if its type has an inventory of its own. Field order controls
inventory order; child order is preserved within each field.

After collection, `__finish_composite!` prepends the canonical field name to
each child selector and the field's name/aliases to its source steps. It
validates resulting logical selector collisions and exports the combined
inventory. It generates no per-entry runtime type, surface type, parser, or
aggregate instruction enum for this path. The existing aggregate enums can
remain behind a transition path until their consumers migrate.

The composite may contain a generic concrete field type such as `CPU<B>`:
its inventory is fixed by the `CPU` declaration while the `Execute` check
uses the relevant generic bounds. An unconstrained type parameter `T` cannot
provide a fixed compile-time inventory without an additional declaration;
v1 requires a concrete inventory provider or an explicit provider path.

### 7.4 `#[instruction_set]`

The instruction-set attribute collects the associated composite's complete
inventory, then its finalizer reads each authored variant's single tuple
payload as a full logical selector. In v1 that payload must be a plain path
of canonical field names followed by the dialect and instruction names;
unit variants, named fields, multiple tuple fields, generic arguments, and
other Rust type forms are rejected. The finalizer resolves the selector
against the inventory and reports missing and repeated selectors. For each
selected entry it replaces the authored payload with the entry's canonical
runtime type and creates a matching flat surface variant with its surface
type. Its parser combines only selected branches and implements
`SurfaceInstruction` for the surface enum. It also emits the composite's
`HasInstructionSet` association. The selector is macro input, never an
unresolved path in the emitted Rust item.

Parsing consumes the entry's ordered source steps and delegates the leaf to
the dialect surface type's `Parse` implementation. It never invokes a child's
whole instruction-set parser, so omitted entries remain unparseable through
this enum. The authored variant, not a payload marker, retains the selected
route. Resolution and execution remain handwritten for the initial phase.

## 8. Finding inventories across crates

A component or composite exports a `#[doc(hidden)] pub` sibling namespace,
for example `__vihaco_inventory_cpu` next to `CPU`. The namespace convention
is the same for components and composites: a parent can see a field's type
path, but cannot reflect on whether that type was declared with
`#[component]` or `#[composite]`. The namespace contains
definition-site type aliases for exported entries and re-exports its
inventory macro under a fixed name, such as `__entries!`. A caller carries
the namespace path with the collected inventory. This keeps emitted type
paths valid when the consumer uses a module-qualified path, a re-export, or
a different crate name.

For a direct field type path such as `crate::devices::CPU<B>`, `#[composite]`
can derive the default namespace path
`crate::devices::__vihaco_inventory_cpu`. Rust type aliases and imported
renames do not reveal their target to a procedural macro. Therefore a field
using such a type needs an explicit inventory-path override, proposed as
`#[device(1, inventory = crate::devices::__vihaco_inventory_cpu)]`.
`#[instruction_set(composite = Alias)]` needs the same override, proposed as
`inventory = path` in its attribute. The override points to the producer's
hidden namespace and does not change the field's canonical selector or
source spelling. Compile tests should pin this path convention and both
override forms.

Stable `macro_rules!` also requires a macro exported to another crate to have
a backing name at the defining crate root. A nested `pub use` alone cannot
export a local `macro_rules!` macro across crates. Each producer therefore
has one hidden crate-root backing macro and a module-qualified re-export.
The default backing name includes the producer kind and declared name, for
example `__vihaco_component_CPU_inventory` or
`__vihaco_dialect_cpu_inventory`. Two same-named producers of the same kind
in different modules of one crate would still collide at the root. V1 offers
an explicit `#[vihaco(inventory_id = unique_ident)]` override on the
declaration. That identifier determines both the backing macro name and the
hidden sibling namespace name, so it also resolves a namespace collision
between types whose names differ only by case after conversion to snake
case. Compile tests pin the collision diagnostic and override. Generation
must not silently rely on a source-position hash or silently overwrite an
export. The generated namespace path remains module-qualified. A consumer
that refers to a producer with a non-default inventory ID uses the explicit
`inventory = path` override rather than deriving the default namespace.

These hidden aliases and macro exports are technically reachable Rust items.
`#[doc(hidden)]` keeps them out of ordinary documentation, but does not make
them private. They are a versioned compiler protocol; no author-written
instruction, marker, or route type depends on their names.

## 9. Diagnostics and IDE behavior

An authored instruction-set payload looks like a Rust type path but denotes
an inventory selector; ordinary Rust name resolution cannot validate it or
provide go-to-definition before macro expansion. Inspect rust-analyzer's
completion and navigation on these paths in the first spike. Component
selectors should still retain direct Rust references where possible.
Finalizers should attach errors to the original payload path tokens when
their spans survive macro forwarding; trybuild tests must pin the actual
spans. If forwarding loses a useful span, retain a small direct check at the
authored attribute rather than reinstating per-entry callback code.

Finalizer errors should distinguish an unknown dialect instruction, an
unavailable component capability, a malformed tuple selector, a valid leaf
behind a `#[loadable]` boundary, and a repeated authored selector. The
compiler's ordinary path error is appropriate for an inaccessible or missing
inventory namespace. Duplicate canonical runtime types still require a
type-level check because aliases can make different token paths denote the
same type.

The goal is less generated clutter, not invisible generated code. Fully
expanding macros will still show inventory exporters, hidden aliases, and the
actual public enums. The measurable improvement is that producer exporters
have no per-instruction selection arms, composite lifting creates no per-entry
types, and ordinary completion centers on authored variants and leaf types.
Inspect whether hidden alias paths in generated enum payloads undermine that
goal before finalizing the public spelling.

## 10. Compatibility and deferred work

This proposal replaces the earlier requirement that every composite lift
create a distinct runtime and surface newtype. The replacement invariant is:
**every available entry has a distinct logical selector; every
selected executable choice has a distinct authored enum variant**. That is
sufficient for the initial parsing, author-owned resolution, and manual
dispatch flow. A later route implementation may bind routes to these variants
and keep its type markers private behind an opaque route return type. If a
future public API genuinely needs an individually named lifted payload type,
add that API for the demonstrated case rather than exposing all markers now.

The older `component!` and current `#[composite]` path generate aggregate
instruction enums and `HasInstructionSet` implementations. The new
`#[instruction_set]` must not produce a conflicting implementation. Keep the
new inventory path opt-in during migration, preserve existing behavior for
legacy consumers, then move `HasInstructionSet` ownership to the authored
instruction set before removing the old generated modules.

Routes, work mappings, bytecode opcode assignment, and a general arbitrary
child parser endpoint are outside this first implementation. In particular,
bytecode must eventually distinguish two authored variants with the same
canonical leaf payload. No leaf opcode assignment is implied by inventory
order.

## 11. Implementation sequence

1. **Prove the protocol.** Implement a passive dialect exporter and a
   callback-to-procedural-finalizer spike. Test one dialect, an aliased import,
   and a cross-crate call. Check selector spans and macro-expanded IDE output.
2. **Centralize the IR.** Add a versioned token parser and internal
   `Inventory`/`Entry`/`SourceStep` structs in `vihaco-runtime-derive`.
   Keep parsing, selection, validation, and rendering separate functions.
3. **Convert `#[component]`.** Collect full dialect inventories, resolve
   selectors in its finalizer, preserve generic coverage and semantic
   duplicate checks, and export the normalized component inventory. Remove
   dialect per-name selection arms after compatibility tests migrate.
4. **Build composite exports.** Collect eligible child inventories, prepend
   field selectors and source steps, and export the result without lift
   wrappers. Preserve device-code validation, source aliases, and loadable
   sections.
5. **Add `#[instruction_set]`.** Parse each authored variant's single tuple
   payload as a logical selector, resolve it against the composite inventory,
   and replace it with the canonical runtime type. Emit the surface enum and
   selected parser branches, and associate the enum with its composite. Add
   an end-to-end two-level manual-dispatch example.
6. **Migrate legacy paths and docs.** Update the guides, examples, and
   doctests once the new path is proven. Retire the
   old aggregate composite instruction generation only after its consumers
   have migrated.

## 12. Validation gates

Tests should cover explicit and wildcard selection order; dialect aliases and
module-qualified paths; semantic duplicate types; unknown and malformed
selectors; a missing inventory provider; generic component coverage; two
sibling fields of the same type; two entries with equal canonical payload
types; recursive source prefixes and aliases; `#[loadable]` exclusion;
selected-subset parsing; a dialect mnemonic prefix pair such as `halt` and
`halt_all`; cross-crate inventory expansion; root backing-macro name
collisions and overrides; `HasInstructionSet` migration coexistence; and
instruction-set diagnostics for unit, named, or multi-field variants and
non-path payloads.

Before considering the implementation complete, run the repository's format,
Clippy, all-target tests, doctests, and SPDX header check. Compare a small
rust-analyzer expansion and completion workflow before and after the change;
generated item count alone does not prove the IDE experience improved.
