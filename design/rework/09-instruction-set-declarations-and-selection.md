# Step 9: instruction-set declarations and selection

**PR:** [PR 4: authored instruction sets and selected parsers](pull-request-plan.md#pr-4-authored-instruction-sets-and-selected-parsers).

**Review/commit:** Complete this file's work and report it for user review. If
there are eligible changes outside `design/`, wait for explicit approval before
its dedicated commit. If the step changes only `design/`, record a no-commit
checkpoint and wait for review approval before starting the next step. See
[workflow](README.md#per-file-review-and-commit-workflow).

**Commit exclusion:** Never stage or commit any file under `design/` at any
point in this plan, including plans, contract records, probes, fixtures, and
results. Exclude them from every implementation commit and PR. Do not move
local planning artifacts elsewhere or create an empty commit merely to satisfy
a step's commit boundary.

**Source references:** `rob/inventory-test`; see [branch context](README.md#source-references-and-branch-context).

**Prerequisites:** [model](02-inventory-model-and-protocol.md),
[paths](03-paths-names-and-options.md), [transport](04-transport-and-type-trees.md),
and [composite](08-composite.md).
**Next:** [selected parsers](10-selected-surface-parsers.md).

## Outcome

Rebuild `#[instruction_set]` around a validated authored declaration and an
ordered selected-entry model. Emit the runtime enum immediately at the authored
scope; resolve the paired surface variants after inventory collection.

## Current implementation and lessons

[instruction_set/mod.rs](../../crates/vihaco-runtime-derive/src/instruction_set/mod.rs)
already preserves the correct runtime enum placement and rewrites selector-shaped
payloads through `__selectors`. It also handles surface naming, a concrete generic
target, one composite request, and the `HasInstructionSet` association.

The same module currently parses input again in the finalizer, constructs a
lookup map that can overwrite duplicate inventory selectors, and builds surface
paths independently of shared type resolution. Split those responsibilities and
validate producer input before indexing it.

## Authored grammar and declaration model

Required argument: `composite = TypePath`. Optional argument:
`inventory = ModulePath`. Continue accepting a concrete generic target’s
terminal arguments; do not infer generic impl parameters from the composite
declaration, which is unavailable to the macro.

The enum is nongeneric and nonempty. Every variant has one tuple field containing
a plain, relative logical selector with at least the dialect/leaf suffix. It
contains no generics, qualified self, leading absolute root, or contextual root
segments. Reject unit, named, multi-field, reference, and other payload forms.
Reject the historical `#[selector]` attribute with a helpful current-syntax error.

Keep an ordered model:

```text
InstructionSetDeclaration {
    runtime_item,
    target: Path,
    namespace: Path,
    surface_name: Ident,
    root: Path,
    variants: Vec<AuthoredChoice>,
}
AuthoredChoice { name, selector, docs, authored_span }
SelectedChoice { authored_choice, runtime_path, surface_path, source_steps }
```

Preserve the original runtime enum’s visibility, docs, supported derives,
variant metadata, and payload spans. The surface enum has its own fixed
`Debug`/`Clone`/`PartialEq` derives and mirrors variant docs, not every runtime
attribute. Reject unsupported discriminants and conditional enum/variant/field
membership explicitly. Keep Step 1’s attribute policy authoritative.

## First-expansion tasks

1. Parse arguments, options, and enum once; validate duplicate/unknown keys,
   namespace path grammar, enum shape, variant shape, and repeated selectors.
   Early duplicate detection can use structural keys before any producer runs.
2. Discover the target namespace using Step 3 support or use its override.
   Overrides resolve aliases/custom IDs; they do not change selector identity.
   An override’s relationship to a target declaration is author-supplied:
   token macros cannot reflect on an imported type to prove its provenance.
3. Rewrite each runtime payload to
   `<namespace>::__selectors::<authored selector>`. Retain authored selector
   tokens in type positions and preserve their spans for editor mapping.
4. Emit the runtime enum at the original scope. Keep match diagnostics naming
   `HydraInstruction::Variant(_)` rather than an internal helper module’s enum.
5. Emit the helper module and one shared collection request for the composite’s
   complete inventory. Forward ordered choices, root, and surface information;
   avoid reserializing a full enum only to rediscover its validated grammar.
6. Re-export the surface enum with authored visibility and the shared naming
   rule. Emit `HasInstructionSet` for the concrete target at authored scope.
   Preserve ordinary coherence errors for repeated sets on one target or
   attempts to associate a foreign type in another crate.

The current direct type-reference checks can remain where they measurably help
editor navigation. Avoid redundant checks with no separate purpose once the
runtime enum already refers to the same selector paths.

## Finalizer selection tasks

1. Parse the shared envelope and require exactly one collected response. Recheck
   context invariants needed to prevent hidden-finalizer panics, including a
   nonempty choice list and valid routes.
2. Validate the received inventory, then build the shared checked full-selector
   index. Duplicate entries must be errors, even if the authored set selects
   only one of them or both entries have the same route.
3. Walk authored choices in declaration order. Resolve each full selector;
   reject any unavailable path at its authored span. Do not compare only leaf
   names or use a reachable type-tree member as evidence of availability.
4. Resolve runtime and surface paths through the same shared builder and the
   chosen entry’s `type_route`. Preserve source steps unchanged.
5. Prove that the selector-tree runtime payload equals the inventory runtime
   payload. A local identity trait with `impl<T> Same<T> for T` can express a
   compiler obligation without public marker types. This catches contradictory
   hand-written producers or a bug in canonical selector merging; token string
   equality cannot prove Rust type identity.
6. Produce ordered `SelectedChoice` records. Emit one matching surface variant
   per authored variant with the paired surface leaf type; hand these records
   to the parser emitter introduced in Step 10.

Availability errors cannot reliably distinguish an unknown instruction, an
omitted component dialect, and a loadable boundary from current protocol
data alone. Report “absent from the composite inventory” and include the full
authored selector. Do not add origin metadata solely to promise distinctions
that this design has not required.

## Tests

- Exact public runtime/surface names, visibility, docs, enum path in match errors,
  and preserved runtime derives; include an enum without an `Instruction` suffix.
- Choices ordered differently from inventory, two variants with one canonical
  payload through sibling fields, and same-named dialect provenance.
- Concrete generic targets, module-qualified targets, imported target aliases
  with overrides, custom roots/IDs, and direct runtime use.
- An instruction from a dialect omitted by the component is absent from both
  generated trees and inventory and cannot be selected.
- Available instructions omitted from the authored instruction set do not parse
  through its surface enum, while remaining available for parent lifting.
- A parent selecting an available leaf omitted from the child’s authored set.
- Duplicate exported inventory selectors and contradictory selector/runtime
  tree types, including a valid paired surface route.
- Empty/generic enums, malformed variant shapes, old selector attributes,
  source aliases in selectors, unavailable/loadable paths, cfg membership,
  missing/inaccessible namespaces, generated-name collisions, and multiple sets.
- Malformed direct hidden-finalizer invocations produce diagnostics, not panics.

Where a parser emitter already exists on the stack, keep it connected during
internal restructuring. Otherwise keep the new declaration, selection, and enum
emitters internal in this step. Test their models and emitted Rust syntax;
exercise selection and runtime/surface type identity with compile fixtures that
do not depend on an unfinished parser. Step 10 activates the complete public
macro and runs the parser-dependent and full macro cases listed above. Do not
publish a parser-less macro or add a placeholder parser for this boundary.

Complete this step, report it for review, and obtain approval for its dedicated
commit before starting Step 10. Both commits belong to PR 4, whose public API
must include the working selected parser at merge.

## Exit criterion

The public runtime enum remains immediate and at the authored scope. Every
surface variant comes from an ordered, validated inventory selection, and its
runtime counterpart is proved to be the same concrete leaf. There is one
instruction-set association and no implicit resolution, dispatch, or bytecode
generation. If the macro is new on the stack, these properties are verified on
the internal emitter output here and on the activated public macro in Step 10.
