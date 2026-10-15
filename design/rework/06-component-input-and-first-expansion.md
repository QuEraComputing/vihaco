# Step 6: component input and first expansion

**PR:** [PR 2: whole-dialect component contracts](pull-request-plan.md#pr-2-whole-dialect-component-contracts).

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
[transport/trees](04-transport-and-type-trees.md), and [dialect](05-dialect.md).
**Next:** [component finalization](07-component-finalization.md).

## Outcome

Turn the authored struct and `#[instructions { arith, boo, foo }]` into an
ordered list of complete dialect providers. Emit their runtime/surface trees
immediately so editor behavior survives staged inventory collection. All
per-instruction coverage is generated uniformly after the inventories arrive.

## Current implementation and lessons

[selector.rs](../../crates/vihaco-runtime-derive/src/component_attribute/selector.rs)
currently implements individual, grouped, and wildcard instruction selection.
Replace it with a parser for dialect module paths. The draft also reparses
paths several times and independently constructs request, provenance, and
selector-tree data. Make one typed dialect record feed those uses.

There are no authored leaf names in the new component attribute, so its direct
explicit leaf checks are unnecessary. Preserve authored dialect path tokens in
first-expansion module re-exports for navigation; keep inventory-driven leaf
coverage in one finalizer path.

## Input and expansion model

Use an ordered record resembling:

```text
DialectRequest {
    dialect: Path,
    authored_span,
    provider: ProviderId,
}
ComponentPlan {
    sanitized_struct,
    root,
    producer_names,
    requests,
    mounts,
}
```

Provider IDs follow dialect-list order. Repeating a dialect is an error,
including through different aliases. IDs are not deduced from import aliases
and do not need to be globally stable. At the transport boundary, serialize only
the provider/request identity needed by the finalizer; there is no selection tag.

## Implementation tasks

1. Parse an `ItemStruct` once and reject attribute arguments/non-struct input
   with authored spans. Support named, tuple, and unit component structs as
   today; preserve their visibility, fields, generics, and ordinary attributes.
2. Consume exactly one brace-list instructions attribute. Parse a nonempty
   comma-separated list of unparameterized module paths with an optional
   trailing comma. Support qualified/absolute paths and imported dialect aliases.
   Use `Path::parse_mod_style`; reject the old list-of-leaves/group/wildcard
   grammar instead of normalizing it to a new form.
3. Store a `syn::Path` for each dialect. Construct structural authored-path keys
   for early duplicate-dialect checks; preserve a leading absolute root where
   it affects path identity. Rust alias equality is a later semantic check.
4. Reject syntactically repeated dialect paths. A path such as `arith::Add`
   has the same token shape as a qualified module path; do not guess from its
   capitalization that it is a leaf. Resolve its required dialect inventory
   namespace/tree through Rust, which rejects an instruction struct used as a
   dialect. Add a targeted fixture explaining the expected dialect module.
5. Emit the sanitized struct at its authored scope and its generic `Component`
   marker impl. Consume only `instructions` and supported `vihaco` options.
6. Bind each complete dialect namespace using Step 3's scope support. Preserve
   its authored path and spans in real module references. Remove the direct
   explicit leaf coverage function, its path collection, and the selection enum.
7. Construct the component inventory namespace and immediate runtime/surface
   provider mounts from the typed requests, using the shared destination-scope
   operations. Every mounted dialect leaf must be in the finalized inventory.
8. Emit the immediate canonical selector view according to the Step 1 probe.
   Support same-named dialects with disjoint complete leaf sets; keep full-selector
   collision detection in the finalizer. Avoid blanket lint allowances that
   conceal actual unresolved ambiguity.
9. Start the shared collector with one request per listed dialect and a
   compact typed context needed for final coverage/export. Forward component
   identifier and generics, framework root, and inventory ID; do not forward
   the entire struct merely to reparse its name and bounds.

## Conditional items and editor constraints

Apply Step 1’s item-attribute policy to the entire generated declaration:
struct, marker impl, inventory module, and collector invocation.
If the struct is disabled, no helper should refer to it. Do not mirror arbitrary
derive or field attributes onto unrelated helper items.

Incomplete dialect-path editing is an editor usability case, separate from the
accepted compiled grammar. Preserve resolvable names before an unfinished path
where practical, and test completion for dialect modules and qualified paths.
Do not accept malformed completed input simply to make editor expansion succeed.

The canonical root of an imported dialect may differ from its authored path’s
last segment. The selector view must follow the recorded merge decision; an
unconditional `pub mod <import alias>` merge cannot establish canonical selector
identity. A type route keeps its provider branch regardless of how the editor
view is assembled.

## Verification

When the attribute is new on the stack, this step implements and tests the
internal declaration model and first-expansion emitter. Inspect the generated
Rust AST and collection context without registering a public macro that would
invoke an unfinished finalizer. Run the full compilation, coverage, conditional
item, and editor cases below in Step 7 when the complete attribute is activated.
If a working attribute already exists, keep it working and run those cases now.

- Parse and inspect the initial expansion as Rust syntax: struct remains at
  authored scope; complete trees already exist; there is no direct leaf coverage
  or aggregate enum/`HasInstructionSet` generation.
- Compile generics with lifetimes, consts, defaults, and where clauses. Include
  `self`/`super` bounds and blanket versus overly narrow `Execute` impls.
- Exercise dialect-list order, qualified dialect paths, import aliases,
  and custom IDs/roots; accept bare dialect names as the primary syntax.
- Pin duplicate helper attributes/dialects, empty lists, generic paths,
  missing commas, old wildcard/group grammar, and instruction types used as
  dialect modules. Reject the old `#[instructions { arith::Add }]` contract.
- Compile the item-level conditional cases from Step 1.
- Check navigation and completion on authored dialect module paths and on
  downstream instruction-set leaf paths using the same editor fixture.

## Exit criterion

One typed dialect list drives requests and complete tree mounts.
The first expansion still exposes useful Rust paths to the editor, and all
scope calculations use shared support. When the attribute is new on the stack,
the tested emitter remains internal until Step 7 supplies its complete-inventory
finalizer; public expansion and editor acceptance are verified then. Complete
this step, report it for review, and obtain approval for its own commit before
starting Step 7. Both commits belong to PR 2. Preserve any working public API
already present while restructuring it; do not introduce a partial-selection
compatibility mode.
