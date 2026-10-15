# Hiding declaration infrastructure

Research date: 2026-09-30. Three parallel agents explored namespace/exporter
hygiene, compiler-resolved producer discovery, and type identity/editor/parser
behavior. Production sources were not changed during this research.

Never stage or commit anything under `design/`, including this record.

## Requirement and recommendation

Users should name dialects, components, fields, instructions, and machines.
They should not need companion namespace paths, exporter IDs, type-tree layout,
or rules for relocating their Rust paths into generated modules.

Recommend ordinary Rust instruction payloads with a separate semantic field
target. The spelling below is illustrative; the exact attribute grammar has
not been chosen or implemented:

```rust,ignore
use vendor::Device as ImportedDevice;
type SpecializedDevice = ImportedDevice<u8>;

#[composite]
pub struct Machine {
    #[device]
    cpu: SpecializedDevice,
}

#[instruction_set(composite = Machine)]
pub enum MachineInstruction {
    #[target(cpu)]
    Halt(control::Halt),
}
```

For nesting, the target might be `#[target(controller::cpu)]`. The target is
a route through actual device fields. `control::Halt` is the actual instruction
type exported by a dialect, resolved by Rust in the authored scope. Neither
path names a fabricated companion tree.

## Static topology is an additive extension

The user explicitly requires the work in `../vihaco-static-top-experiment`
to be additive to this proposal. The base declaration system must stand on
its own: complete component dialects, complete eligible child availability,
authored instruction selections, semantic field targets, and paired surface
parsing. Existing loadable-field boundaries remain intact.

Static topology can extend execution with explicit route bindings, message
construction, effect handling, work, continuations, backends, profiles, and
drivers. Those extensions must not make route functions, work enums, backend
sets, or profiles mandatory for ordinary use of the base system.

In particular, do not replace complete composite availability with the entries
of an advertised child route interface. Public child routes and their entry
parsers can provide an additional encapsulated forwarding API. They do not
restrict the field-target selection supported by the base proposal.

Reuse or adapt the experiment's typed field-access relationship rather than
introducing unrelated field systems. Instruction identity remains the actual
Rust payload type; explicit route identity adds behavior choices that may
share that payload type. Route-specific source bindings and work-origin tags
preserve those choices when the extension is used. Their validation must
coexist with the base capability and selection checks.

The base parser uses the instruction's associated surface type plus field
prefixes/aliases. An extension can additionally select a child entry's parser
to preserve route-specific source spelling. It need not reinstate companion
namespace inference or callback inventories.

The exact execution-extension API and the adapter between the field contracts
remain design work. The additive relationship is a requirement, not an optional
alternative. This supersedes the discussion recommendation to substitute
advertised child route entries for complete child availability.

## How the implementation would connect declarations

1. A dialect generates each real runtime instruction and its paired surface
   type. An internal instruction trait associates the runtime type with its
   surface type, dialect identity, and canonical name.
2. A dialect associates an ordered list of all its instruction types. Recursive
   trait obligations verify that a component implements `Execute<I>` for every
   leaf, under its original generic bounds. An ordinary function body enforces
   these obligations without needing to call it.
3. A component implements a support trait for each declared dialect. An extra
   `Execute<I>` implementation alone does not grant inventory membership.
4. A composite implements field-traversal traits whose associated child types
   are its actual device field types. Internal exact character-list keys encode
   field names without hash collisions. Names and aliases are associated data.
5. The instruction-set macro translates the semantic target into traversal
   projections, checks declared dialect support and execution, and keeps the
   authored runtime payload type in the enum.
6. Surface payloads use the instruction's associated surface type. Selected
   parser branches call its existing parser and obtain nested field prefixes
   and aliases through the traversal metadata.
7. Hierarchical associated-constant capability descriptions retain full
   availability, declaration order, field names, and source aliases. Safe const
   validation can reject duplicate canonical selectors and ambiguous field
   spellings without collecting child inventories as proc-macro tokens.

Traits and supporting data may need public visibility across crates; macros
own them and documentation hides them. Users do not supply implementations or
paths to this infrastructure. Physical generated modules can still exist where
useful, but their layout is not a declaration requirement.

This uses Rust's existing associated types and constants rather than asking a
proc macro to perform type resolution. [Rust Reference: associated items](https://doc.rust-lang.org/reference/items/associated-items.html).

## Alternatives explored

| Approach | Result |
| --- | --- |
| Same-name inventory macro facade | Ordinary import aliases and chained reexports work. Rust type aliases do not alias macros. Backing exported macro names still collide, and `$crate` identifies a crate rather than the defining module. |
| Automatic exporter IDs | Token hashes collide for identical declarations; location-based naming does not guarantee unique macro-generated declarations. Mixed-site spans and anonymous constants did not isolate crate-root exports. |
| Larger VM declaration macro | A single declaration can see its hierarchy and allocate local names. Feasible, but introduces a larger declaration boundary and was not the preferred option for independent Rust types. |
| Selector spelling rewritten into trait projections | Can hide namespace discovery and preserve `field::dialect::Leaf`. Requires additional selection machinery, and editor navigation was weaker in the bounded probe. |
| Actual instruction type plus field target | Compiler handles aliases and generics; payload editor navigation reaches the real leaf. No full child-token enumeration is needed for selected parsing. Recommended. |

Naive recursive type-list lookup has overlapping trait implementations. Coverage
over the whole list does not have that problem. The recommendation avoids a
generic leaf-name lookup by using the actual instruction type; complete
same-named/disjoint dialect validation instead operates on const descriptions.

The crate-root behavior of exported macros is specified in the
[Rust Reference](https://doc.rust-lang.org/reference/macros-by-example.html#path-based-scope).

## Evidence

Probes ran on local stable Rust 1.96. They are mechanism/output-shape tests,
not completed production proc-macro implementations.

| Case | Observed result |
| --- | --- |
| External producer, renamed dependency/import, generic type alias, nested fields | Compiles and runs with associated runtime/surface projections. |
| Whole-dialect execution coverage under generic bounds | Passes; adding an uncovered leaf fails with the missing `Execute<NewLeaf>` obligation. |
| Declared membership independent of execution | A selected instruction from an undeclared dialect fails. |
| Same canonical dialect name, disjoint leaves | Passes const validation. |
| Same canonical dialect name, overlapping leaves | Fails const evaluation with `duplicate canonical instruction selector`. |
| Unknown device field | Fails its traversal obligation. |
| Ambiguous sibling canonical/alias source spelling | Fails const validation. |
| Selected parser with associated surface type | Canonical and nested aliased prefixes produce the same surface instruction; unselected leaf rejects. |
| Alias alternatives sharing a prefix | Passes when each alternative includes its `::` separator before combining. |
| Editor on actual runtime payload | Hover, completion, and definition reach the real leaf. |
| Editor on rewritten selector/projection | Navigation points into hidden keys or an associated type declaration. |
| Missing route diagnostic wrapper | Primary diagnostic says `machine does not provide device field route controller::cpu`; hidden route-key bound is suppressed. |

Artifacts:

- `/tmp/vihaco-name-probe`: same-name facades, exporter collisions, coverage.
- `/tmp/vihaco-semantic-inventory-probe`: cross-crate projections, coverage,
  list-lookup failures, canonical duplicates, const metadata, diagnostics.
- `/tmp/vihaco-type-identity-probe`: nested fields, generics/aliases,
  availability, surface types, hierarchical descriptions, Chumsky parser.
- `/tmp/vihaco-hidden-infrastructure`: independent generic associated-constant
  capability tree and ordered traversal.

Reproduce the combined positive parser/type probe with:

```sh
cargo run --offline --manifest-path /tmp/vihaco-type-identity-probe/Cargo.toml
```

Diagnostic wrappers use `diagnostic::on_unimplemented` and
`diagnostic::do_not_recommend`; the latter hides irrelevant blanket-impl
suggestions. They reduce visible internals but do not promise every compiler
note will omit helper names. Real diagnostics need fixture review.
[Rust Reference: diagnostic attributes](https://doc.rust-lang.org/reference/attributes/diagnostics.html).

### Rust 1.98 follow-up

The combined parser/type probe also passed with `cargo +1.98.0 run --offline
--manifest-path /tmp/vihaco-type-identity-probe/Cargo.toml --quiet`.
On that compiler, the `const-type-id.rs` and `lookup-overlap.rs` probes still
reject const TypeId equality and overlapping recursive lookup respectively.
A separate stdin probe confirmed that an unused generic associated constant
containing `panic!` is not evaluated merely because its type is instantiated.
These constraints remain architectural concerns on 1.98.

Source-location methods on proc_macro::Span are stable since 1.88, so 1.98
permits using them without compatibility guards. They do not provide a
compiler-resolved producer namespace or a guaranteed unique exporter ID.
[Span documentation](https://doc.rust-lang.org/stable/proc_macro/struct.Span.html).
Rust 1.98 also makes some ambiguous glob-import cases hard errors, reinforcing
the benefit of preserving authored payload paths in their original scope rather
than relying on broad parent imports.
[Release notes](https://doc.rust-lang.org/stable/releases.html#version-1980-2026-08-20).

The recommendation remains actual instruction types plus semantic field targets.

## Boundaries and remaining acceptance work

- Verify real expansions across crates, including effect/message constraints,
  bytecode derives, discriminants, visibility, raw identifiers, conditional
  items, and disabled/loadable fields.
- Establish the intended compiler policy before selecting a compatibility
  matrix. Rust 1.85 is the edition-2024 minimum mentioned in AGENTS.md and
  mise.toml, not an explicit `rust-version` commitment for these macro crates.
  Tooling uses `stable`. The separate parser crates explicitly declare 1.75.
  The research should not constrain the new runtime architecture to 1.85
  without a project decision to support it.
- Force const validation deliberately. Generic associated constants are lazy;
  declaring one does not validate every instantiation. Concrete instruction-set
  targets can force complete validation, while generic declarations must still
  check all information known at declaration time. Define enforcement for
  generic composites used without an instruction-set association.
- Preserve duplicate selected-route detection when instruction paths are
  aliases. Compare canonical instruction metadata and semantic target routes,
  rather than authored path tokens.
- Fresh generated dialect leaves have nominal Rust identity and one instruction
  association. Repeating aliases retains that association; repeated dialect
  support or duplicate canonical selectors can reject repetition. This argument
  assumes generated producers are authoritative. A custom-producer extension
  API would need a separate consistency contract and identity strategy.
- Test large dialects and deep routes for compiler costs and practical error
  quality. Const duplicate checking need not be quadratic forever.
- Repeat editor checks on actual proc-macro expansions; current evidence uses
  small declarative macros representing the two output shapes.

## End-of-file checkpoint

Research is complete and recommends replacing the namespace-inference and
exporter-ID dependency with compiler-resolved capabilities and semantic targets.
This recommendation is ready for review; it does not approve or commit Step 3,
choose final attribute grammar, or revise all remaining implementation steps.
The existing Step 3 source changes remain uncommitted. No PR was created.
All files under `design/` remain excluded from staging, commits, and PRs.
