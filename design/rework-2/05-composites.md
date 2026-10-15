# Step 5: typed composite fields and complete child capabilities

**PR:** [PR 3: typed composite fields and complete capabilities](pull-request-plan.md#pr-3-typed-composite-fields-and-complete-capabilities).

**Review/commit:** Complete this file's work and report it for user review.
If there are eligible changes outside `design/`, wait for explicit approval
before its dedicated commit. If only design changes exist, record a no-commit
checkpoint and wait for review approval before the next step. See
[workflow](README.md#per-file-review-and-commit-workflow).

**Commit exclusion:** Never stage or commit any file under `design/` at any
point in this plan, including plans, contracts, probes, fixtures, and results.
Exclude them from every implementation commit and PR. Do not move local design
artifacts elsewhere or create an empty commit to satisfy a step boundary.

**Source references:** current checkout and pinned behavioral reference;
see [branch context](README.md#source-references-and-branch-context).
**Prerequisites:** Step 4 committed; Step 1 feature activation boundary accepted.
**Next:** [Step 6](06-instruction-sets.md).

## Outcome

Represent a composite through its actual fields and child types. Typed field
relationships and hierarchical descriptions replace sibling namespace discovery and
lifted runtime/surface trees. Preserve full eligible availability and loaders.

## Field model

Parse each named field once into a small record with identifier/type/span,
device metadata, canonical source name/aliases, and optional loadable section.
Store eligibility once. Ordinary fields stay untouched.

Target identity is the exact unraw Rust field name. Source aliases are literal
segments, independent of type identity. Preserve device codes and section names
as domain metadata; validate them even when a loadable field contributes no
parent instructions.
Every device requires an explicit u8 code, including loadable sections. Codes
are unique within the parent and are the Field keys for eligible devices.
Do not allocate implicit codes or declaration-order indices. Loadable devices
retain their codes as metadata but expose no parent Field relation.

## Implementation tasks

1. Parse/strip device/loadable/root helpers and preserve ordinary field/item
   attributes. Reject duplicate helpers/codes/aliases/section names, unsupported
   shapes, and inconsistent loadable metadata.
2. Generate Field<const CODE: usize> relations for eligible devices, using the
   explicitly authored u8 device code as the key (emitted as a usize literal).
   Emit hidden public `__vihaco_field_<name>` usize constants containing that
   same code, with narrowly scoped
   non_upper_case_globals allowances. Preserve the unraw field identifier's case
   and Unicode spelling; do not uppercase, hash, or encode names into types.
   Each relation associates the actual authored syn::Type and source spellings
   in the owning declaration scope. Do not relocate the child type or emit getters.
   Use literal device codes in impls and generic descriptions, not Self-associated
   const expressions involving unresolved generic parameters. Internal helper
   names may not collide with authored associated items.
3. Project nested child types through those relations. Imports, type aliases,
   generic parameters, associated types, and qualified types are Rust's job;
   retain needed original domain bounds rather than restrict types to paths
   from which a sibling module can be guessed.
4. Compose full hierarchical descriptions using only canonical field names
   and child references; parsing aliases remain in Field::SOURCE. Include every
   eligible child capability, regardless of its authored executable set.
   The same instruction through different fields remains valid.
5. Exclude ordinary and loadable fields from target traversal and parent
   capability lifting. A loadable child need not supply parent capability
   metadata if it satisfies its loader contract.
6. Preserve loader/program-counter/section behavior and child instruction-set
   associations. No implicit instruction dispatch or aggregate ISA generation
   is required for new availability.
7. Validate all declaration-local source/code/section collisions immediately.
   Use standard string/character operations for empty/whitespace/control/::
   source segments. These local checks are not duplicated in runtime support.
   Force complete nongeneric descriptions where possible; generic-dependent
   descriptions are forced by concrete set/reporting boundaries. Do not rely
   on an unused associated const as proof.
8. Keep the typed emitter internal until Steps 6–7 complete the experimental
   public contract. The feature-off public macro keeps its current behavior.
   Gate preparation under `typed-declarations`; test parsing/emission in process.
   Do not add custom Cargo/rustc subprocesses, file-writing compiler harnesses,
   or generated-binary execution inside tests. Do not register a public placeholder macro or invent a
   temporary namespace option to make this step independently consumable.
9. Document the relationship between field types, full capability metadata,
   and executable sets. Complete public docs/activation land with Step 7.

## Verification

Compile empty/state-only/loadable-only composites, multiple same-type fields,
nested children, imported aliases, generic/associated child fields, defaults,
lifetimes/consts, raw identifiers, source aliases, and custom framework roots.
Assert full order and exact child/runtime types. Check sparse codes, 0/255,
code stability under field reordering, and parent-local reuse of the same code.
Reject missing codes on eligible and loadable devices and all duplicate codes,
including collisions across loadable section boundaries.

Reject target lookup on ordinary/loadable fields and sibling alias collisions.
Test alias collisions under different parents correctly remain local.
Check section loaders and current public composite fixtures with the feature
off; check typed fixtures with it on. Do not add compatibility aliases.
Probe checked concrete reporting for generic descriptions and full overlap
through nested children.

## Exit criterion

A tested typed emitter and nested field projections preserve actual fields, child
identity, complete order, aliases, and section boundaries without trees or
callbacks. Feature-off public composites still work. State explicitly whether
public activation awaits Step 7, and obtain dedicated commit approval.

## End-of-file checkpoint

**Status:** approved and committed as `7f63618`. See
[Step 5 results](step-5-results.md). Public activation awaits Step 7.
**Proposed dedicated commit:** `refactor: prepare typed composite capability traversal`.
Report completed implementation, eligible files, checks, and material limits;
wait for explicit approval before committing. If the final public change is
breaking, adjust the conventional marker/footer to the reviewed diff.
Never stage or commit anything under `design/`. A completed report is not
commit approval, and no next step starts while this checkpoint is pending.
