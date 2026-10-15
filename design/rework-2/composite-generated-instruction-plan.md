# Composite-generated flat instruction sets

Status: concrete plan for the current declaration system. User decisions are
complete eligible-device enumeration, a flat representation, predictable
companion lookup, explicit device codes, and ordinary void-returning Step and
Dispatch. The implementation details are implemented for review in the experimental feature:
hidden inventories, flat runtime/surface aggregates, parsing, and void Dispatch.
Hierarchical wire codecs and new module-resolution contracts remain separate
reviewed work.

This plan develops [Composite instruction sets, field markers, and
routes](composite-dispatch-and-step.md) and uses the current Step and Dispatch
contracts. The owned-execution proposals
in [Resolution, Step, and generated Dispatch](resolve-step-dispatch.md) are
outside this work.
It replaces the authored instruction-selection approach in the earlier
rework-2 plans for aggregate generation. It also revises their prohibition on
inventory token transport: a narrow, hidden callback protocol will provide
the entries needed to generate a complete flat enum.

## Intended behavior

`#[composite]` generates a hidden instruction enum containing the complete
available instruction set of its eligible devices.

Traversal follows these rules:

1. Visit every field marked `#[device(...)]` in declaration order.
2. Skip any field also marked `#[loadable]`, including its entire subtree.
3. For a component, include every public instruction leaf marked by its
   declared dialects, in dialect-list and leaf-declaration order.
4. For a nested composite, recursively include its eligible devices.
5. Ignore ordinary state fields and public Rust items that are not declared
   instruction leaves.

Availability is independent of a child's selected executable instruction set
and independent of the owning machine's Step coverage. Repeated instances of
the same component contribute separate destinations.

The runtime representation must be one flat enum. Every variant carries the
actual leaf instruction directly. A nested composite or dialect enum is never
the payload of a parent variant. Empty eligible inventories produce an empty,
uninhabited enum.

Users interact with specific instructions through
`Step<field!(Machine::destination), Instruction>`. They do not need generated
enum names, variant names, inventory indices, or callback invocations.

## Why an additional contract is needed

The existing capability descriptions already enumerate canonical field paths,
dialect names, and instruction mnemonics while respecting loadable boundaries.
They do not enumerate the actual Rust instruction types.

`InstructionSpec` provides the surface type and dialect association once a
runtime leaf type is known. It does not enumerate all leaf types from a child
component type. String metadata cannot reconstruct Rust paths reliably through
imports, aliases, private modules, and crate boundaries.

Associated type lists and indexed associations can expose actual leaf types to
ordinary Rust. They cannot alone provide a proc macro with the number and
structure of enum variants to emit: macro expansion does not evaluate an
associated constant or expand an associated type into its constituent tokens.
Rust also rejects macro invocations inside an enum as a way to splice variants.

Under the requirements of separate declarations, cross-crate children, and a
complete flat enum, the demonstrated approach is to collect entries through
hidden callbacks, then emit the whole enum in one operation. This is a tested
approach, not a claim that every possible callback-free architecture is
impossible.

## Hidden type associations

Use one document-hidden support trait under the existing `__private` surface:

```rust,ignore
pub trait LeafAt<const SLOT: usize> {
    type Instruction: InstructionSpec;
}
```

The dialect's existing `__VihacoDialect` is the authoritative provider. Add one
LeafAt implementation per marked leaf, in declaration order. Do not add a
second independent instruction list or duplicate the actual leaf definitions.

Each component and composite gets a `Provider` type in its generated inventory
companion. The final collector assigns local slots in complete traversal order
and emits forwarding LeafAt implementations for that Provider. Component slots
forward to dialect providers; composite slots forward to eligible child
providers. These implementations are emitted in the owning declaration's
original scope, where its authored paths and private modules are accessible.

For example, generated beside a component:

```rust,ignore
impl framework::__private::LeafAt<0>
    for __vihaco_inventory_Cpu::Provider
{
    type Instruction =
        <arithmetic::__VihacoDialect as
            framework::__private::LeafAt<0>>::Instruction;
}
```

Downstream payloads use the public companion Provider projection. The callback
accepts its provider path from the caller rather than reconstructing an
absolute path to the producer's enclosing module. This is important for
same-named declarations in different modules and private dialect modules.

Slots are local lookup positions, not opcodes, bytecode IDs, or public API.
Preserve source-order enumeration without assigning numeric encoding identity.
Public leaves in private modules remain usable through public associated-type
projections. Use unrestricted pub, document-hidden support items where public
associated types or downstream expansion require them. Manual producer
implementations remain unsupported.

## Hidden enumeration callbacks

Use one narrow protocol for declaration-produced inventory entries:

- Dialects enumerate their ordered leaf entries and type projections.
- Components combine the inventories of their declared dialects.
- Composites combine eligible child inventories, prefixing field paths and
  marker chains at each nesting level.
- A collector passes the complete records to one final proc-macro emitter.

Callbacks carry structural entries. They do not perform runtime reflection,
evaluate capability constants, discover source files, or determine Step
behavior. Preserve token hygiene through forwarding; do not stringify and
reparse payload types or framework roots. Use hygienic producer references
such as `$crate` where the generated macro needs to refer to its own crate.

Keep continuations and accumulator structure internal to the protocol. Use a
shared collector rather than introducing different callback conventions for
each declaration kind. The generated runtime enum remains flat even though
inventory collection traverses a hierarchy.

## Predictable companion lookup

Use these expected access paths:

| Declaration | Enumeration callback | Type provider |
| --- | --- | --- |
| Dialect `arithmetic` | `arithmetic::__vihaco_inventory::__enumerate!` | `arithmetic::__VihacoDialect` |
| Component `Cpu` | `__vihaco_inventory_Cpu::__enumerate!` | `__vihaco_inventory_Cpu::Provider` |
| Composite `Machine` | `__vihaco_inventory_Machine::__enumerate!` | `__vihaco_inventory_Machine::Provider` |

Component and composite companions are siblings of their declarations. A
qualified child such as `hardware::devices::Cpu<T>` maps to
`hardware::devices::__vihaco_inventory_Cpu`; strip generic arguments only from
the companion lookup. Dialect companions are inside their authored modules.
Preserve original generic arguments and bounds for actual child type checks.

Unraw the declaration identifier when constructing the companion name. Keep
case and Unicode spelling; do not use PascalCase normalization or hashing for
the expected companion path. Resolve self/super/crate paths in the declaration's
original scope using the same scope-preservation rules as existing emitters.

Imported renames and type aliases need an accompanying companion reexport:

```rust,ignore
use hardware::Cpu as Host;
use hardware::__vihaco_inventory_Cpu as __vihaco_inventory_Host;
```

A type alias does not automatically alias its companion. Existing field
companion reexports are still needed where field traversal needs them. No
inventory selector attribute, registry, or source discovery is introduced.

The first implementation handles named declaration paths, including named
generic types such as Cpu<T>. A bare type parameter or qualified associated
type does not identify a declaration companion. Keep this as an explicit
unsupported inference case for flat enum generation; report the authored child
span and the need for a named declaration companion. Do not guess from a
generic default, inspect trait implementations, or introduce a shape override
in this work. Existing capability-only behavior for such types must be audited
and its affected callers reported before public activation; do not describe
this change as preserving that support.

### Public aliases and backing macro names

Stable cross-crate macro_rules callbacks need macro_export backing definitions,
whose names occupy the producing crate's root macro namespace. Their backing
names are not the expected lookup paths in the table above.

Reserve `__vihaco_inventory_export_` for backing names. Allocate a unique suffix
inside the emitting proc-macro process; an encoded declaration-origin prefix
may additionally aid diagnostics. Reexport each backing macro as __enumerate
inside its fixed companion. The allocation is only name disambiguation: it
stores no inventory registry, changes no entry order, and participates in no
instruction, field, variant, or bytecode identity. Backing symbol spelling is
not promised reproducible; companion access paths are.

The probe showed why origin-only names are insufficient: repeating one
macro_rules declaration template reuses its source location and collides.
A unique backing suffix allowed both expansions to compile and kept their
companion access paths unchanged. Production verification must include clean
and incremental builds and reordered macro-generated declarations.

Reject generated-name collisions visible within the parsed declaration. Let
Rust diagnose sibling namespace collisions that the attribute cannot inspect,
retaining declaration spans. Reserve generated prefixes rather than silently
renaming a public companion and breaking predictable lookup.

## Shared entry model

Build one internal record for every destination field path and actual
instruction leaf. Use the same records for enum and Dispatch generation.

Each record contains:

- The canonical destination field segments and complete field marker type.
- The actual runtime leaf type, expressed directly or through an association.
- Dialect and leaf identity information needed for internal naming.
- A deterministic, collision-free generated variant identifier.
- The hierarchical device-code path and dialect/instruction encoding
  associations needed for bytecode generation.
- Available declaration spans for diagnostics.

Variant names use readable, case-preserving segments. Start with `V_`, then
join each canonical field segment, canonical dialect head, and authored leaf
identifier with `__`. Remove Rust's `r#` spelling prefix before encoding a
segment. Escape every underscore inside a segment as `_u`; leave its other
identifier characters, including Unicode, unchanged.

For example:

| Destination and leaf | Generated variant |
| --- | --- |
| cpu / arithmetic / Add | V_cpu__arithmetic__Add |
| controller / cpu / arithmetic / Add | V_controller__cpu__arithmetic__Add |
| cpu_2 / arithmetic / Add | V_cpu_u2__arithmetic__Add |

This encoding is reversible: encoded segments contain no `__`, and each `_u`
inside a segment decodes to one underscore. It preserves segment boundaries,
case, and underscore spelling, so different paths do not collapse into the
same identifier. `r#type` and `type` are the same Rust name, not separate
identities. Do not capitalize or concatenate segments without separators.

Source aliases and imported type spellings do not change these names. Existing
selector validation rejects overlapping canonical leaves; do not use an
inventory ordinal to conceal a semantic duplicate. Generated names are internal
and do not define bytecode identity.

Source aliases do not produce extra entries or new field identities. Existing
declaration validation remains authoritative for conflicting selectors and
duplicate dialect declarations.

## Enum and Dispatch emission

### Aggregate types

Emit the complete enum after collection. Define hidden sibling types named
`__vihaco_<Composite>Instruction` and `__vihaco_<Composite>SyntaxInstruction`,
with the composite identifier unrawed. Define them in the owning declaration's
original scope and mark generated support items doc(hidden).

Both enums use the same variant identifiers and record order. Runtime payloads
are Provider LeafAt projections; syntax payloads are their InstructionSpec::Surface
projections. Generate Clone, Debug, and PartialEq where the existing leaf
contracts provide them. Do not add blanket From<Leaf> conversions: one leaf
may have several destinations.

Use HasInstructionSet::Runtime and HasInstructionSet::Syntax to expose the two
types, and use that same Runtime type as Dispatch::Instruction. Do not create
another public aggregate naming API or a shared syntax module that collides
when several composites occupy one module. Reconcile the authored
instruction_set association for these composites explicitly during migration;
there cannot be two HasInstructionSet implementations for the same type.

The current dialect contract uses nongeneric instruction leaves. Preserve a
fixed inventory for named generic declarations and avoid adding unused generic
parameters or a fake PhantomData instruction variant to the flat enum. Preserve
all actual declaration bounds on Field, capability, and forwarding impls.
Block changes that make the leaf set vary by an unresolved type parameter.

### Step and Dispatch

Generate a direct Dispatch impl for the actual composite, preserving only its
original generic parameters and bounds. Every match arm calls Step for that
composite's exact field marker and leaf type. Rust checks all these calls when
the declaration is compiled, even if Dispatch is never invoked or the generic
composite is never instantiated. Do not hide missing coverage behind additional
Step where-clauses, generic DispatchTo adapters, or a conditional blanket impl.
HasInventory nominal provider equality also rejects a companion reexport
belonging to a different child declaration.

Use the existing production contracts without changing their returns:

```rust,ignore
impl framework::Dispatch for Machine {
    type Instruction = <Self as framework::HasInstructionSet>::Runtime;

    fn dispatch(&mut self, instruction: Self::Instruction) {
        match instruction {
            __vihaco_MachineInstruction::V_cpu__arithmetic__Add(value) => {
                <Self as framework::Step<
                    field!(Machine::cpu), arithmetic::Add,
                >>::step(self, value);
            }
            // One arm for every other entry.
        }
    }
}
```

Each arm is emitted from the same record as its variant. Preserve the complete
nested marker route. The generated call returns (); there is no execution
wrapper, scheduling input, driver, or common execution-outcome requirement.

Missing Step coverage is a trait error at Dispatch emission, identifying the
field marker and actual leaf type. Missing Execute coverage retains the
existing Step terminal-child checks. Add trybuild cases using authored field
spans and original leaf associations; do not generate fake Step implementations
or restrict the enum to leaves whose Steps happen to exist.

Every generated composite with eligible leaves must provide complete Step
coverage, including intermediate composites. Empty/state-only/loadable-only
composites have empty aggregates and need no Step implementations. Inventory
availability remains based on declared instructions and does not shrink to
match the available Steps. Missing-Step tests must declare the composite with
an empty main and no Dispatch use; generic tests must prove that narrower
specialized implementations do not satisfy a generic declaration.

## Hierarchical bytecode identity

Keep the existing agreed encoding direction:

```text
outer device code → nested device code → … → dialect ID → instruction ID → payload
```

Flattening the Rust enum does not flatten this identity. Encoding and decoding
must use the record's hierarchical identity, never enum declaration order,
Rust discriminants, generated names, or inventory lookup indices.

Require an explicit u8 code on every device, including loadable devices.
Validate uniqueness among all sibling devices. Do not allocate codes from
field order or equate them with marker type identity. Preserve section boundaries.

Dialect and leaf numeric identity remain authored codec policy. This work
allocates no dialect IDs or instruction IDs from inventory slots, declaration
order, or variant names, and does not replace existing authored wire schemas.
The aggregate records retain the code path and leaf identity so a later codec
integration can consume authored numeric associations. Automatic wire-codec
emission is outside the initial slice; do not advertise bytecode support for
an aggregate until its hierarchical codec is implemented and tested.

## Implementation order

1. Implement the specified LeafAt support and uniform callback/collector format.
   Test predictable companions, backing export uniqueness, and alias reexports.
2. Extend dialect emission with authoritative leaf slots and its producer.
   Preserve existing parser and leaf behavior.
3. Extend component emission to combine declared dialects, generate its Provider
   forwarding impls, and publish its companion. Preserve complete Execute
   coverage under the original bounds.
4. Extend composite emission to collect eligible children, prefix complete field
   paths, forward Provider slots, and emit the complete flat runtime enum and
   type association. Require explicit device codes and report unsupported child
   inference forms before public activation. Direct Dispatch checks require full
   Step coverage for every generated entry.
5. Generate the paired surface aggregate and parsing from those same records,
   associate it through HasInstructionSet, and migrate authored aggregate-set
   callers. Preserve explicit leaf/module resolution; there is no implicit
   conversion or assumption that a parsed item always emits one instruction.
6. Implement Dispatch emission using the existing void-returning contracts.
   Preserve original generic bounds and require full Step coverage at
   declaration time, even without any executable use.
7. Implement hierarchical encoding in a separate reviewed slice using authored
   wire identities and exact-byte fixtures. Surface/module resolution plumbing
   that needs new public contracts receives its own review rather than being
   invented inside the enum generator.

Future execution lifecycle and routing changes remain outside this plan.
Keep the experimental feature boundary until explicitly changed. Run the
required repository checks for each implementation slice, including feature-off
and feature-on graphs. Do not introduce a compatibility layer, remove existing
APIs incidentally, or commit without explicit approval. Never stage or commit
files under design/.

## Verification and existing evidence

The temporary workspace at `/tmp/vihaco-composite-enumeration-probe` contains
manually authored experimental inventory contracts around actual declaration
macros, a collector, and a final enum/dispatch proc macro. It is evidence of
feasibility, not implemented declaration-macro generation or durable repository
test coverage.

The earlier enum/dispatch probes used hex variant names; their passing results
prove mapping and type transport, not the revised readable spelling. A focused
check of the readable encoding tested 60,879 short segment paths for collisions
and round-trip decoding. Production emission tests must exercise this spelling.

The first nine-test probe passed with clippy on local Rust 1.99.0. Further
current-scope probes test fixed companions for same-named declarations in
separate modules, repeated macro-template expansions with unique backing
exports, and actual production Step/Dispatch implementations with void returns.
A source-origin-only export naming experiment failed for repeated templates
and is not the selected convention. The enumeration and dispatch probes cover:

- Complete cross-crate enumeration, independent of a narrowed child set.
- Exclusion of loadable boundaries and ordinary state.
- Associated type-list traversal and indexed enum payload projections.
- A flat callback-generated enum with all six destination/leaf Dispatch arms
  exercised for repeated component instances.
- Generic child instances, renamed dependencies, and imported callback aliases.
- Deeper field-marker paths, Unicode names, and an empty inventory.
- Associated projections exposing leaves from a private dialect module.
- Macro input containing expression/type spellings rather than evaluated
  capability descriptions.

Separate compiler probes demonstrated that an associated type expression does
not expand into a token list, macros cannot splice enum variants, and raw paths
to a private dialect module fail downstream privacy checks.

The production implementation needs ordinary integration tests, in-process
emission tests, and established trybuild fixtures. Include deterministic naming
and segment-boundary collisions, raw identifiers, multiple modules with the
same declaration names, public/private scopes, companion alias behavior,
unsupported associated-child inference diagnostics, named generic bounds,
unavailable inventories, missing Step coverage, and feature-off/on behavior.
Verify payload identity and Dispatch
destinations rather than token formatting.

Run independent feature graphs as top-level commands. Do not introduce custom
tests that launch Cargo/rustc, write generated compilation fixtures, or execute
generated fixture binaries. The temporary probes were built with top-level
Cargo commands, not subprocesses invoked by tests.

## Decision summary and scope

| Decision | Choice |
| --- | --- |
| Availability | Complete eligible device tree; skip loadable subtrees |
| Type authority | Dialect LeafAt slots, forwarded through component/composite Providers |
| Enumeration | One hidden callback protocol and whole-enum final emitter |
| Lookup | Fixed companion module plus __enumerate and Provider |
| Macro exports | Unique private transport suffix; stable companion alias |
| Renames/aliases | Corresponding companion reexports |
| Unknown child declaration | Diagnose; no guessed inventory or new override syntax |
| Variant naming | Readable case-preserving segments, __ separators, and _u underscore escaping |
| Aggregate access | Existing HasInstructionSet and Dispatch associated types |
| Dispatch | Current Step/Dispatch with ordinary void returns |
| Coverage | Every generated composite declaration, including unused generic declarations |
| Device codes | Explicit u8 codes on every device, unique per parent |
| Dialect/leaf wire IDs | Preserve authored codecs; no inventory-derived allocation |
| Surface representation | Paired flat enum from the same records; explicit resolution |

The current-scope choices are specified. Follow-on implementation reviews cover
public activation/migration, complete parser/module integration, and actual
hierarchical codec support. The probes are evidence for the mechanisms they
exercise, not evidence that those integrations or the production declaration
emitters have already been implemented.
