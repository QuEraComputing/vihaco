# Step 2: runtime associations and capability validation

**PR:** [PR 1: typed instruction relationships and dialect declarations](pull-request-plan.md#pr-1-typed-instruction-relationships-and-dialect-declarations).

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
**Prerequisites:** Approved Step 1 contract and mechanism proofs.
**Next:** [Step 3](03-dialect.md).

## Outcome

Provide the smallest shared Rust contracts that all owning macros need.
Types carry identity; ordered static data carries complete descriptions.
There is no serialized inventory protocol or compiler source discovery.
Keep declaration feature parity with the inventory-based proposal. Do not
introduce the sibling static-topology experiment's topology/profile/backend/
driver/wire/continuation/route-work model or its execution attributes. These
remain later additive work. Field relations provide only child types and source
names needed by target helpers; mutable access is deferred until required.

## Model and ownership

Keep semantic records small, along these lines:

```text
InstructionSpec: Surface, Dialect, canonical dialect/leaf names
DialectSpec: nominal identity, ordered dialect description
Supports<Dialect>: declared component membership
Coverage<Component>: generated dialect impl with complete Execute bounds
Field<const INDEX: usize>: actual child type, canonical name and aliases
Capabilities: ordered hierarchical description and forced validation
```

Names above are internal design names, not promised user extension traits.
Use one document-hidden runtime support module accessible through both public
roots. Keep the documented Execute<I> interface outside that hidden surface.

Use parent-local usize field indices without hashes or character marker types.
The composite macro assigns eligible fields indices in declaration order and
emits hidden public `__vihaco_field_<name>` associated constants. Preserve the
unraw identifier's case and Unicode spelling. Target macros read those constants
from each actual concrete parent type, including aliases and Child projections;
users never spell or coordinate the numeric indices. Generated Field impls and
generic descriptions use literal indices, preserving original generic bounds
without unstable generic const expressions. Initially instruction sets are
nongeneric and their machine types are concrete.
Keep field relationships limited to child-type projection and source metadata.
Do not add getters, access traversal, or borrowing APIs until a macro needs them.
These generated internal contracts may change with their owning macros; manual
user implementations are unsupported.

Descriptions retain borrowed static dialect/leaf lists and child references.
Vec/map construction is unnecessary for immutable generated descriptions.
A reporting iterator may flatten them in deterministic order at use time;
it is not a global registry or instruction dispatch mechanism.

## Implementation tasks

1. Define nondefault `typed-declarations` in runtime/derive/facade with coherent
   forwarding. Gate new support and public exports. Add Execute<I> under that
   gate if absent, preserving the reference's input/message/effect/error
   contract and eyre conventions. Feature-off GeneratedComponent remains
   unchanged; do not silently wrap every effect in a new shape.
2. Implement the proved instruction/dialect/support/coverage relationships
   behind the same gate. Feature-on Component/Composite markers have no
   HasInstructionSet supertrait; feature-off markers retain current bounds.
   Audit shared consumers for explicit association bounds where needed, without
   adding a compatibility layer. Follow the [Step 1 feature decision](step-1-results.md#caller-and-abi-migration-boundary).
   Dialect leaves remain unrestricted pub. Generate Coverage<C> directly for
   each dialect identity with one C: Execute<I> bound per leaf. No instruction
   type list, list nodes, terminator type, or blanket traversal is needed.
3. Implement parent-local field indices, actual child-type associations, and source
   metadata. Nested target types resolve through associated Child projections.
   Preserve authored lifetimes and generic bounds without requiring 'static.
   No field getter or mutable traversal is needed by the current macros.
4. Add ordered static descriptions and safe const validation for overlapping
   canonical selectors across dialects. Store only canonical names in device
   descriptions; parsing aliases remain in Field::SOURCE. Source-name syntax,
   nonempty declarations, and local alias collisions are owning-macro checks
   using standard string/character methods. Do not duplicate them in runtime.
   A small byte comparison is needed because str/slice PartialEq is not stable
   in const evaluation; do not add a dependency just for this operation.
5. Encode validation forcing at the boundaries proved in Step 1. Keep raw
   descriptions behind generated support. Internal reporting of a concrete
   generic type must use a validated entry point, not silently skip checks.
6. Keep runtime data errors eyre and authored proc-macro syntax errors syn::Error.
   No invalid macro input should panic in the proc-macro process.
7. Port useful common root/attribute behaviors selectively from the preserved
   shared branch following Step 1's classification. Root paths are dependency paths,
   not component companion-module paths. No producer-ID option is supported.
8. Export the support through facade/runtime consistently, including renamed
   dependencies. Preserve root override behavior for existing macros.
9. Document generated-only producer authority and const-validation timing.
   Do not make a public manual inventory API merely because helpers need pub.

## Verification

Use static impl fixtures to test real alias identity, nested typed fields,
ordered traversal, disjoint same-head dialects, complete overlap rejection,
unknown fields, and validation forcing. Declaration-local source spelling and
alias diagnostics are covered by the owning macro steps, not manual runtime
description fixtures.
Test that a component with an extra Execute impl still lacks declared support.
Check errors in generic coverage without adding the missing bound to the check.

Test isolated `typed-declarations` off/on and indirect unification graphs.
Run feature checks as direct CI/tool commands. Do not construct workspaces,
write files, or invoke Cargo from Rust tests; use static fixtures and trybuild
for compiler diagnostics.
Feature-off API checks must run without a feature-on workspace member masking
the result. Test facade, direct runtime, renamed deps, no-default-features, and derive
exports where affected. Keep helpers available in the runtime configurations
needed by downstream generated code. Do not use token rendering or const
TypeId equality as acceptance assertions.

## Exit criterion

A small compiler-resolved support surface and ordered validated descriptions
work with real typed fixtures. Default execution and feature-specific repository
gates pass.
No exporter, callback collector, type tree, or producer namespace helper exists
in the new support. Report this step for its dedicated commit approval.

## End-of-file checkpoint

**Status:** implemented and committed as 072166e after explicit user approval.
**Evidence:** [Step 2 results](step-2-results.md).
**Dedicated commit:** `feat: add gated typed instruction capability contracts`.
All required checks passed; local design artifacts were excluded from the commit.
The user explicitly requested stopping here. Do not start Step 3 until asked.
Never stage or commit anything under `design/`.
