# Type trees for instruction inventories

> **Status:** experimental type-tree implementation on `rob/inventory-test`.
> **Scope:** dialect, component, composite, and instruction-set inventories.
> **Current implementation:**
> [instruction-set-implementation-plan.md](instruction-set-implementation-plan.md)
> and [structured-instruction-inventories.md](structured-instruction-inventories.md).

The experiment implements first-expansion runtime and surface trees, replaces
per-entry type aliases with `type_route`, retains ordered inventories, and
accepts selector-shaped instruction-set payloads without `#[selector]`.
Same-named dialects in one crate currently need distinct
`#[vihaco(inventory_id = ...)]` values because their exported inventory backing
macros share the crate's macro namespace.

## Why consider this

The former inventory protocol transported a logical selector, a runtime type,
a surface type, and source-prefix metadata for every instruction. Its type
fields were definition-site `RuntimeN` and `SurfaceN` aliases. Collector macros
recorded each producer namespace so later finalizers could qualify those aliases.
This works across crates, but rust-analyzer did not navigate from the original
selector-shaped enum payload through component and composite aliases generated
by later macro expansions. The earlier `#[instruction_set]` implementation used
a concrete Rust payload and a separate logical selector:

```rust
#[vihaco::instruction_set(composite = Hydra)]
pub enum HydraInstruction {
    #[selector(stream_a::core::cpu::ConstI64)]
    LoadA(leaf::cpu::ConstI64),
}
```

The concrete payload had useful hover and go-to-definition behavior. The
`#[selector(...)]` path was macro metadata and received no semantic tokens or
navigation. First-expansion trees now give inventory producers real Rust paths
to their runtime and surface structs and support
`LoadA(stream_a::core::cpu::ConstI64)` as the authored API. A parallel
first-expansion selector tree gives rust-analyzer a path to navigate and the
runtime enum a concrete payload type. The finalizer uses the provider-specific
route for the surface enum.

The implemented protocol and collector are in
[`inventory/mod.rs`](../crates/vihaco-runtime-derive/src/inventory/mod.rs).
Its producers are [`dialect`](../crates/vihaco-runtime-derive/src/dialect/mod.rs),
[`component_attribute`](../crates/vihaco-runtime-derive/src/component_attribute/mod.rs),
and [`composite_inventory`](../crates/vihaco-runtime-derive/src/composite_inventory/mod.rs);
[`instruction_set`](../crates/vihaco-runtime-derive/src/instruction_set/mod.rs)
consumes the final composite inventory.

## Terms and invariants

These paths serve different purposes and should remain distinct in the data
model:

| Term | Example | Meaning |
| --- | --- | --- |
| Dialect identity | `vendor_a::cpu` | The particular dialect declaration; `vendor_b::cpu` may be a different dialect. |
| Logical selector | `stream_a::core::cpu::Add` | The canonical route used to select an available instruction. It omits source aliases and internal tree branches. |
| Type route | `stream_a::core::provider_0::cpu::Add` | A path within the producer's hidden type tree to the concrete runtime and surface structs. |
| Source steps | `stream_a`/`a`, `core`/`c` | Canonical field names and accepted source aliases used by the parser. |

The type route is relative to an inventory producer namespace. Both type
trees have the same route suffix:

```text
namespace::__types::runtime::<type_route> → dialect runtime struct
namespace::__types::surface::<type_route> → dialect syntax struct
```

At the dialect producer, the route can equal the selector. The component
tree in this proposal adds an internal provider branch for each dialect, so
its exported route differs from the selector even when names are unique.
Those branches express dialect provenance and are not source-language syntax.

The inventory remains the authority for availability and order. The type
tree supplies resolvable Rust types; its mere existence does not select an
instruction for an authored instruction set.

## Proposed producer output

### Dialect

`dialect!` already knows the generated runtime struct and its matching syntax
struct. Alongside its existing `__entries!` export, its first expansion would
produce two public, doc-hidden module trees. For `dialect! { cpu { Add, Halt } }`:

```text
cpu::__private::__types::runtime::cpu::Add  → cpu::Add
cpu::__private::__types::surface::cpu::Add  → cpu::syntax::Add
```

The leaves must be `pub use` re-exports of the concrete structs. A `type`
alias stopped rust-analyzer navigation at the alias in an isolated probe.
The dialect supplies the canonical `cpu` node even if a component imports the
dialect as `local_alias`.

### Component

`#[component]` knows its authored dialect paths in its first expansion. It
can mount each dialect's runtime and surface trees under a distinct internal
provider branch, using the same sibling inventory namespace it already
exports:

```rust
// Illustrative output for a component using two different `cpu` dialects.
pub mod __vihaco_inventory_controller {
    pub mod __types {
        pub mod runtime {
            pub use crate::vendor_a::cpu::__private::__types::runtime as provider_0;
            pub use crate::vendor_b::cpu::__private::__types::runtime as provider_1;
        }
        pub mod surface {
            pub use crate::vendor_a::cpu::__private::__types::surface as provider_0;
            pub use crate::vendor_b::cpu::__private::__types::surface as provider_1;
        }
    }
    // The ordered __entries! export remains.
}
```

Thus `provider_0::cpu::Add` and `provider_1::cpu::Halt` stay in different
modules. The provider labels may be generated identifiers or explicit stable
component-local names; neither should appear in source text. A component
finalizer associates each collected dialect record with its provider branch,
then emits each selected entry's logical selector and type route. Its
`Execute<I>` coverage check uses the runtime tree path.

The exact tree/availability relationship depends on the component API. If
components must execute complete dialects, a provider tree and its exported
inventory contain the same instructions, and one re-export mounts the whole
dialect. If partial `#[instructions { cpu::{Add, ...} }]` selection remains,
the first expansion may expose the whole dialect tree while the inventory
lists only selected leaves. The finalizer must continue rejecting a selector
absent from that inventory. A precisely filtered first-expansion tree would
avoid editor suggestions for unavailable leaves but needs more generation
work, especially for wildcard selection and renamed dialect imports. The
type-tree refactor does not require deciding the whole-dialect rule first.

### Composite

`#[composite]` knows non-loadable device fields and their child inventory
namespaces in its first expansion. It can re-export a child's whole runtime
and surface trees under the field name, without enumerating its leaves:

```rust
pub mod __vihaco_inventory_stream {
    pub mod __types {
        pub mod runtime {
            pub use crate::leaf::__vihaco_inventory_controller::__types::runtime as core;
        }
        pub mod surface {
            pub use crate::leaf::__vihaco_inventory_controller::__types::surface as core;
        }
    }
}
```

The composite finalizer still collects child entries. It prepends `core` to
each logical selector and type route, and prepends a `SourceStep` containing
`core` and its source aliases. A parent composite does the same for its own
field names, such as `stream_a` or `stream_b`. Loadable fields contribute no
tree branch or inventory entries. Sibling CPU components remain separate:
`first::provider_0::cpu::Add` and `second::provider_0::cpu::Add` do not
collide. Existing `inventory = path` overrides and relative-path rebasing
would be used for both collection and tree re-exports.

For one leaf, the path changes at each producer boundary as follows:

| Producer | Logical selector | Type route |
| --- | --- | --- |
| `cpu` dialect | `cpu::ConstI64` | `cpu::ConstI64` |
| `Cpu` component | `cpu::ConstI64` | `provider_0::cpu::ConstI64` |
| `Stream` composite | `core::cpu::ConstI64` | `core::provider_0::cpu::ConstI64` |
| `Hydra` composite | `stream_a::core::cpu::ConstI64` | `stream_a::core::provider_0::cpu::ConstI64` |

Each row uses its own producer namespace as the root of the type route. The
parser's `source_steps` also grow at the composite rows: `core` and then
`stream_a`, each with its accepted source aliases.

## Revised inventory protocol

The shared entry could shrink from:

```text
entry {
    selector: { core::cpu::Add },
    runtime: { Runtime0 },
    surface: { Surface0 },
    source_steps: { ... },
}
```

to:

```text
entry {
    selector: { core::cpu::Add },
    type_route: { core::provider_0::cpu::Add },
    source_steps: { ... },
}
```

Given the collected producer namespace, one shared helper would construct
the runtime and surface paths by appending `type_route` to the corresponding
tree roots. `CollectedInventory::resolve` would no longer resolve per-entry
aliases. `Inventory::prepend` would prefix both paths at each composite
field. The collector still records producer namespaces, tags, and ordered
responses so a finalizer can find the appropriate tree and metadata.

| Current mechanism | Proposed treatment |
| --- | --- |
| `Entry.runtime` and `Entry.surface` | Replace with one `Entry.type_route`. |
| Exported `RuntimeN` and `SurfaceN` aliases | Replace with first-expansion runtime and surface module re-exports. |
| `CollectedInventory::resolve` alias qualification | Replace with a shared tree-path builder. |
| Component named selection and `Inventory::select_names` | Retain if partial component selection remains; remove only if the whole-dialect rule is adopted. |
| `__entries!`, `collect`, component/composite finalizers | Retain: Rust macros cannot enumerate module members, and consumers need ordered entries. |
| `SourceStep`, source aliases, selected parser branches | Retain: Rust module names cannot represent all accepted source spellings. |

The instruction-set finalizer would look up an authored logical selector in
the composite's complete inventory. Its selected entry supplies the type
route for the runtime payload check and the paired surface parser type.
Selected variants, parser order, source aliases, and `#[loadable]` exclusion
retain their current semantics. This tree does not generate execution
dispatch or operand resolution.

## Worked examples

The Rust declarations below use current macro syntax; omitted `Execute`
implementations make the excerpts schematic. The type routes and generated
modules are **proposed output**, not paths available in vihaco today. The
first example is shortened from the
[`instruction_set` integration test](../crates/vihaco/tests/instruction_set.rs);
that test exercises its selectors and source aliases. In the trees below,
`→` means a `pub use` re-export of the module or concrete struct, not a
copied type definition. `provider_0` and `provider_1` are illustrative
internal names.

### Two CPU fields in a nested machine

```rust
mod leaf {
    dialect! { cpu { ConstI64(i64), Halt, } }

    #[component]
    #[instructions { cpu::{ConstI64, Halt} }]
    pub struct Cpu { pub value: i64 }
    // Execute<cpu::ConstI64> and Execute<cpu::Halt> implementations.
}

#[composite]
struct Stream {
    #[device(1, alias = "c")]
    core: leaf::Cpu,
}

#[composite]
struct Hydra {
    #[device(1, alias = "a")]
    stream_a: Stream,
    #[device(2, alias = "b")]
    stream_b: Stream,
    #[device(3)]
    #[loadable]
    auxiliary: Stream,
}

#[vihaco::instruction_set(composite = Hydra)]
enum HydraInstruction {
    LoadA(stream_a::core::cpu::ConstI64),
    LoadB(stream_b::core::cpu::ConstI64),
}
```

The two selections have the same concrete payload struct, but identify
different device fields. The proposed Hydra inventory and tree would contain:

| Variant | Logical selector | Type route in `__vihaco_inventory_hydra` |
| --- | --- | --- |
| `LoadA` | `stream_a::core::cpu::ConstI64` | `stream_a::core::provider_0::cpu::ConstI64` |
| `LoadB` | `stream_b::core::cpu::ConstI64` | `stream_b::core::provider_0::cpu::ConstI64` |

The proposed expansion mounts one producer's tree inside the next. The
resulting Hydra tree is conceptually:

```text
leaf::cpu::__private::__types
├── runtime::cpu::{ConstI64, Halt}  → leaf::cpu::{ConstI64, Halt}
└── surface::cpu::{ConstI64, Halt}  → leaf::cpu::syntax::{ConstI64, Halt}

leaf::__vihaco_inventory_cpu::__types
├── runtime::provider_0             → leaf::cpu::__private::__types::runtime
└── surface::provider_0             → leaf::cpu::__private::__types::surface

__vihaco_inventory_stream::__types
├── runtime::core                   → leaf::__vihaco_inventory_cpu::__types::runtime
└── surface::core                   → leaf::__vihaco_inventory_cpu::__types::surface

__vihaco_inventory_hydra::__types
├── runtime
│   ├── stream_a → __vihaco_inventory_stream::__types::runtime
│   │   └── core::provider_0::cpu::{ConstI64, Halt}
│   └── stream_b → __vihaco_inventory_stream::__types::runtime
│       └── core::provider_0::cpu::{ConstI64, Halt}
└── surface
    ├── stream_a → __vihaco_inventory_stream::__types::surface
    │   └── core::provider_0::cpu::{ConstI64, Halt}
    └── stream_b → __vihaco_inventory_stream::__types::surface
        └── core::provider_0::cpu::{ConstI64, Halt}
```

The `Stream` and `Hydra` mounts are module re-exports; they do not generate
new `ConstI64` or `Halt` structs. The `auxiliary` loadable field contributes
no branch.

For `LoadA`, the runtime path would be
`__vihaco_inventory_hydra::__types::runtime::stream_a::core::provider_0::cpu::ConstI64`;
the matching syntax path would use `__types::surface` with the same suffix.
Both re-exports ultimately point to types generated by `leaf::cpu`. `LoadB`
starts with `stream_b`, so the two field routes never collide.

Source aliases stay in `source_steps`, not in either path above. The current
parser accepts both `stream_a::core::cpu.consti64 4` and
`a::c::cpu.consti64 4` for `LoadA`; it accepts
`stream_b::core::cpu.consti64 7` for `LoadB`. The `auxiliary` field is
`#[loadable]`, so this example has no `auxiliary::...` instruction entry or
tree branch.

### Two dialects both named `cpu`

Suppose `vendor_a::cpu` declares `Add` and `vendor_b::cpu` declares `Halt`.
A `Controller` component selects both dialects, with the corresponding
`Execute` implementations:

```rust
mod vendor_a { dialect! { #[vihaco(inventory_id = vendor_a_cpu)] cpu { Add, } } }
mod vendor_b { dialect! { #[vihaco(inventory_id = vendor_b_cpu)] cpu { Halt, } } }

#[component]
#[instructions { vendor_a::cpu::*, vendor_b::cpu::* }]
struct Controller;
```

Its proposed inventory preserves the dialect of each leaf:

| Logical selector | Type route in `__vihaco_inventory_controller` | Original dialect |
| --- | --- | --- |
| `cpu::Add` | `provider_0::cpu::Add` | `vendor_a::cpu` |
| `cpu::Halt` | `provider_1::cpu::Halt` | `vendor_b::cpu` |

Here the proposed component tree has two separate provider mounts:

```text
__vihaco_inventory_controller::__types
├── runtime
│   ├── provider_0 → vendor_a::cpu::__private::__types::runtime
│   │   └── cpu::Add  → vendor_a::cpu::Add
│   └── provider_1 → vendor_b::cpu::__private::__types::runtime
│       └── cpu::Halt → vendor_b::cpu::Halt
└── surface
    ├── provider_0 → vendor_a::cpu::__private::__types::surface
    │   └── cpu::Add  → vendor_a::cpu::syntax::Add
    └── provider_1 → vendor_b::cpu::__private::__types::surface
        └── cpu::Halt → vendor_b::cpu::syntax::Halt
```

The two `cpu` nodes are under separate provider modules. There is no merged
`cpu` module whose leaves appear to come from one dialect. Mounting this
component in a composite field named `controller` prepends that field to
both paths: selector `controller::cpu::Halt` maps to type route
`controller::provider_1::cpu::Halt`. A second `Controller` field named
`backup` would get its own `backup::...` paths.

If `vendor_a::cpu` and `vendor_b::cpu` both define `Halt`, both entries would
claim the exact selector `cpu::Halt`. The inventory would reject that
collision until a separate disambiguating selector policy exists.

### A visible type that is unavailable to the component

If partial component selection remains supported, this component selects
only one instruction from a dialect that defines two:

```rust
dialect! { cpu { ConstI64(i64), Halt, } }

#[component]
#[instructions { cpu::Halt }]
struct StopOnly;
// Execute<cpu::Halt> implementation.
```

The simplest proposed tree and finalized inventory would disagree on the
available leaves:

```text
__vihaco_inventory_stop_only::__types
├── runtime::provider_0 → cpu::__private::__types::runtime
│   └── cpu::{ConstI64, Halt} → cpu::{ConstI64, Halt}
└── surface::provider_0 → cpu::__private::__types::surface
    └── cpu::{ConstI64, Halt} → cpu::syntax::{ConstI64, Halt}

finalized StopOnly inventory
└── selector: cpu::Halt; type_route: provider_0::cpu::Halt
```

The simplest first-expansion tree would re-export the whole dialect. Rust
path completion could therefore offer both `provider_0::cpu::Halt` and
`provider_0::cpu::ConstI64`. The finalized inventory would contain only
`cpu::Halt`. If a composite has a `stopper: StopOnly` field, an instruction
set selecting `stopper::cpu::ConstI64` must still fail inventory validation,
despite that Rust type being reachable in the tree. `stopper::cpu::Halt`
would be valid. This illustrates why the type tree cannot replace the
ordered inventory as the availability check.

If components are changed to execute complete dialects, the `StopOnly`
declaration would instead need `Execute` for both instructions. Its tree and
inventory could then expose the same set of leaves. That rule is an
independent API decision.

## Same-named dialects and true selector collisions

The `Controller` example above shows why distinct dialects can share the
name `cpu` when their full selectors differ. A single glob-imported `cpu`
module from each dialect is ambiguous in Rust (E0659), even though the leaves
differ. Merging the leaves into one `cpu` module would incorrectly portray
the two dialect declarations as one. Separate provider branches preserve
their identities; the inventory maps each selector to the correct branch.

Supporting two dialects with the same full selector would require an explicit
disambiguating selector name and a specified source-text policy; the type tree
alone cannot choose between them. This is distinct from two CPU components
in separate composite fields, where `first::cpu::Halt` and
`second::cpu::Halt` are already different selectors.

## Authored syntax and editor behavior

The instruction set now uses the selector path directly:

```rust
LoadA(stream_a::core::cpu::Add),
```

The first expansion emits the runtime enum at the authored scope and maps its
payload through the `__selectors` tree. That tree follows component and
composite fields, including merged same-named dialect roots. The finalizer
uses provider-specific routes such as
`stream_a::core::provider_0::cpu::Add` for the surface enum. The first
expansion also emits the authored path in a type position to preserve editor
completion and navigation. Keeping the runtime enum at its authored scope
lets rustc suggest `HydraInstruction::Add(_)` in match diagnostics.

## Evidence and limits

The initial tests behind this proposal were isolated under `/tmp`; the later
implementation and editor probes ran against vihaco's macros:

- A three-crate workspace used procedural macros to emit a dialect tree, a
  component tree via an imported dialect alias, and two composite tree levels
  across a crate boundary. `cargo check --workspace --offline` passed.
- rust-analyzer 0.3.3065 followed runtime and surface `pub use` chains. On
  `stream_a::core::cpu::ConstI64` it classified the prefix segments as
  namespaces and the leaf as a struct; leaf hover displayed the concrete
  dialect struct, and go-to-definition reached the authored dialect macro
  token. A `type` alias stopped navigation at the alias in an earlier probe.
- A Rust compiler probe showed two glob-imported `cpu` modules were
  ambiguous (E0659). Manually merging leaves compiled but violated the
  intended dialect identity model.
- A procedural-macro rewrite probe with distinct provider branches compiled
  and preserved hover/navigation on complete authored `cpu::Instruction`
  suffixes. Variant hover displayed the hidden route.
- Earlier attempts to navigate through the current staged component and
  composite `RuntimeN` aliases returned no hover or definition, despite
  successful Rust compilation.
- In the integrated `instruction_set` and `inventory_type_tree` fixtures,
  rust-analyzer 0.3.3065 returned hover for the authored selector path
  segments, completion for the selected instruction, and go-to-definition
  from the authored leaf to its dialect declaration. This worked through
  nested composites, merged same-named dialects, and a cross-crate producer.
  Variant hover now names the authored enum and the generated selector-tree
  payload path.

The probes establish the integrated callback/finalizer placement and editor
navigation for the tested forms. They do not establish that every possible
alias combination works or that variant hover can preserve the authored
payload spelling.

## Implementation and acceptance checks

The first-expansion runtime and surface trees, `type_route` transport,
ordered inventory collector, and selector-shaped syntax are implemented.
The tests cover two composite levels, sibling fields using the same component
type, a cross-crate component imported under an alias, runtime and surface
types, two distinct same-named dialects with disjoint leaves, and duplicate
full selectors. Editor probes cover go-to-definition and completion on the
authored selector path. Repository format, Clippy, target-test, doctest, and
license checks are run separately. Compile-time or editor-performance
improvement is not assumed; compare generated code size and build times if
those become reasons to adopt the tree.
