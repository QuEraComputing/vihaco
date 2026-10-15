# Step 8: reimplement `#[composite]`

**PR:** [PR 3: recursive composite lifting and section boundaries](pull-request-plan.md#pr-3-recursive-composite-lifting-and-section-boundaries).

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

**Prerequisites:** [paths](03-paths-names-and-options.md),
[transport/trees](04-transport-and-type-trees.md), and
[component finalization](07-component-finalization.md).
**Next:** [instruction-set selection](09-instruction-set-declarations-and-selection.md).

## Outcome

Generate a complete recursively composable inventory from non-loadable device
fields, while preserving device validation and bytecode/SST section loading.
Parse the composite declaration once and derive every output from one field
model.

## Starting points and lessons

The work currently spans [lib.rs](../../crates/vihaco-runtime-derive/src/lib.rs),
[attr_composite.rs](../../crates/vihaco-runtime-derive/src/attr_composite.rs),
[fields.rs](../../crates/vihaco-runtime-derive/src/attr_composite/fields.rs),
[loaders.rs](../../crates/vihaco-runtime-derive/src/attr_composite/loaders.rs),
and [composite_inventory/mod.rs](../../crates/vihaco-runtime-derive/src/composite_inventory/mod.rs).

The public entry point reparses input for sanitization. Fields are represented
as device tuples plus a separate loadable list; eligibility repeatedly searches
that list. Tree mounts reparse token tags that were just produced from fields.
The new field model can eliminate all three patterns.

## Field model

Keep the sanitized original `ItemStruct`, plus ordered device records:

```text
DeviceField {
    name: Ident,
    ty: Type,
    code: u8,
    aliases: Vec<LitStr>,
    inventory_override: Option<Path>,
    loadable_section: Option<SectionName>,
    attribute_span,
}
```

Non-device fields remain in the original struct. A field’s loadable status is
part of its own record; a derived loader view can borrow those records. Use
named fields as required today. Do not sort devices by their numeric code.

## Implementation tasks

1. Parse arguments and struct once. Validate named-field struct shape and
   consume `device`, `loadable`, and supported `vihaco` attributes while retaining
   the authored item’s other attributes and generics.
2. Parse one device attribute per field; diagnose repeated helper attributes at
   the later occurrence. Validate `u8` code range, unique codes, source names and
   aliases, and duplicate/malformed inventory overrides.
3. Preserve loadable validation: it requires a device field; its optional
   section name cannot be empty or contain `/`; section names are unique.
   Apply the source-spelling and conditional-membership policy from Step 1.
   Continue validating all device metadata even when the field is loadable.
4. Derive eligible fields once by filtering `loadable_section.is_none()`.
   Infer or use their child namespaces through shared path support. Concrete
   generic fields have declaration-fixed inventories; bare type parameters,
   imported renames, aliases, and custom IDs need explicit provider paths.
5. Build first-expansion runtime, surface, and selector mounts directly from
   eligible field records. Mount each child under its canonical field name.
   Loadable devices must trigger no parent inventory request or tree reference.
6. Emit the sanitized struct, generic `Composite` marker impl, and inventory
   namespace. Invoke shared collection with field-name/alias tags in declaration
   order. Empty eligible lists go straight to finalization.
7. Give the finalizer only the inventory ID and data actually needed to
   interpret responses. Remove currently parsed-and-discarded root/composite
   context fields unless the chosen validation needs them.
8. Finalize with the shared prefix operation: field name on logical selector
   and type route, and one source step on the front of source steps. Concatenate
   children with checked selector uniqueness and export through shared support.
9. Keep current loaders attached to this declaration using the loadable field
   view. Preserve own-section-first loading, named child delegation, rejection
   of unexpected children, method/impl bounds, and original field type arguments.
10. Adapt the feature-off legacy path to consume the same parsed field model
    while it remains. Do not retain a second parser or change the legacy mode
   as an unreviewed side effect; Step 12 owns its migration/removal decision.

If this PR is the first introduction of composite inventory mode on the stack,
keep its planned boundary opt-in until the authored-set API lands in PR 4.
Retain the `main` consumers of legacy aggregate output at this merge boundary.

## Recursive composition

For a leaf `cpu::Add`, the component route might be
`provider_0::cpu::Add`. A `Stream` with `core` exports:

```text
selector:    core::cpu::Add
type_route:  core::provider_0::cpu::Add
source_steps: core/c
```

A parent `Hydra` with `stream_a` adds that field to both paths and adds
`stream_a/a` before `core/c`. It retains the same concrete leaf types.
Two sibling fields containing the same component are valid because their
logical selectors differ. Do not apply component semantic-type duplicate
checks to this lifted inventory.

A child’s authored set is irrelevant to lifting. Parent selection must be
able to use an available child capability even if the child’s own executable
enum omits it. Test that distinction explicitly.

A component child exposes every leaf of every declared dialect. The composite
does not filter those leaves or relax their `Execute` obligations; only a later
authored instruction-set enum selects individual executable routes.

## Tests

- One child, sibling repeated child types, two or three composite levels,
  exact field/child order, aliases at each level, and original paired types.
- Empty composites, state-only named structs, and loadable-only composites.
- A loadable child without a discoverable inventory must still be valid if
  its loading contract is met. Its parent must never try to collect it.
- Cross-crate component and composite children, concrete generic fields,
  generic composites, type parameter plus explicit inventory, imported aliases,
  custom IDs/roots, and all supported relative namespace paths.
- Child availability independent of its authored executable set.
- Runtime bytecode/SST loader delegation and unexpected-section rejection,
  using `multisection_bytecode.rs` and the existing SST fixtures.
- Duplicate device/loadable helpers, codes, aliases, section names, malformed
  overrides, invalid namespace inference, missing inventory, logical selector
  collision, and generated namespace collision.
- Conditional-item cases chosen in Step 1; helper attributes are removed once.

Compile the legacy mode separately while it exists; a unified workspace Cargo
invocation can enable inventory mode for dependencies and hide legacy breakage.

## Exit criterion

One declaration parse feeds marker, tree, inventory, and loader output. A parent
in another crate can collect and re-export a child composite with preserved
order, types, aliases, and loadable boundaries. Inventory mode generates no
aggregate enums or `HasInstructionSet` association.
