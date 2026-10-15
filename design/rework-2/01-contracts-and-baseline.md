# Step 1: contracts, proofs, and implementation baseline

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
**Prerequisites:** None. Read the overview and current checkout before changing implementation.
**Next:** [Step 2](02-runtime-associations-and-validation.md).

## Outcome

Turn the proposed [contract](contract.md) into a verified implementation target.
Establish which mechanisms work, where generic checks happen, and how the
current workspace can migrate without importing the reference architecture.
This step changes local design/probes only and has no expected commit.

## Baseline and pending-work audit

Record current HEAD, base, compiler, feature configurations, and dirty files.
The new branch starts at `main`; the user preserved approved `fbfec16` and WIP
`7d36633` on `rob/shared-inventories-and-dialect`. They are reference
preparation, not ancestors or typed-capability implementation on this branch.

Classify the prior branch's patch by behavior: retain useful framework-root discovery,
helper stripping, and independently useful tests only in their owning new step;
omit inferred namespaces, exporter IDs, type-tree paths, and corresponding
implementation-only assertions. Report the classification before applying it.
Do not discard unrelated work or rewrite approved history.

Inspect legacy markers, facade/runtime exports, component!, dispatch, CPU,
encoded enums, composite parsers, observers, doctests, and multisection loaders.
Build a caller/ABI migration table for stabilization. Put new APIs and marker
relationships behind nondefault `typed-declarations`, forwarded coherently
through facade/runtime/derive. Feature-off behavior stays current; unification
can select the experimental contract for the entire shared package instance.
No compatibility macros or adapters are added.
New composite activation belongs with complete authored sets/parsers in Step 7;
Step 5 may be internal preparation to avoid a half-working public macro.

Start from the actual [runtime contracts](../../crates/vihaco-runtime/src/generated.rs),
[derive entry points](../../crates/vihaco-runtime-derive/src/lib.rs),
[legacy composite emitter](../../crates/vihaco-runtime-derive/src/attr_composite.rs),
and [facade exports](../../crates/vihaco/src/lib.rs).
These links refer to this checkout, not reference-only draft modules.

## Proof tasks

1. Compile the proposed target/payload syntax with actual associated field and
   instruction relationships. Test external producers, renamed deps/imports,
   chained reexports, concrete type aliases, generic/associated child types,
   lifetime/const generics, and explicit self/super roots.
2. Prove whole-dialect coverage under original component bounds in a checked
   function body. Remove one Execute impl and add one dialect leaf; both must
   fail even if no authored set selects that leaf.
3. Prove declared support is separate from arbitrary Execute implementations.
   Test same-head disjoint dialects, complete overlaps, and repeated aliases.
4. Prove hierarchical associated descriptions preserve order and source aliases.
   Force complete validation at declaration/concrete-set/reporting boundaries.
   Test unused generic associated constants and compare cargo check with codegen
   where generic evaluation differs. Record exactly what was proved.
5. Prove selected parsers using associated surface types and field metadata,
   including nested aliases, shared-prefix spellings, omitted leaves, and more
   branches than Chumsky's tuple-choice limit.
6. Prove Rust identity checks for duplicate selected pairs through type aliases
   without TypeId equality. Generated-only producer authority is the supported
   boundary; do not invent a general arbitrary-producer reflection protocol.
7. Prove `#[dialect]` on inline modules with immediate `#[instruction]`
   unrestricted `pub` unit/tuple structs. Preserve imports, unmarked items,
   module/field visibility, docs, and runtime fields in their authored scope.
   Reject private/restricted instruction visibility. Runtime and surface fields
   have the same type by default. User amendment during Step 8 adds
   `$0:SourceType` pattern annotations for explicitly different source/runtime types;
   both types preserve the authored Rust scope. Resolution stays explicit.
   Prove surface operand scoping for self/super, generic/qself operands, imports
   inside the module, explicit parent paths, a parent type with the same name
   as an instruction, and a type named syntax. Prefer surface structs in the
   authored module over a scope bridge. Record rejection cases for out-of-line
   modules, unsupported instruction shapes, and misplaced/repeated markers.
8. Check real or representative expansions for collisions: same-named types in
   different modules, raw fields, declaration-local helper names, anonymous
   coverage checks, and component/field types with unrelated impls.
9. Prove local feature gating: new support/macros absent by default, present
   with explicit opt-in, and activated coherently through dependency unification.
   Keep production feature integration assigned to Step 2 and activation to
   the owning steps. Run current default-root fixtures without macro aliases.
10. Record actual editor hover/completion/definition for retained payload types
   and associated surface projections. Output-shape evidence is provisional
   until actual proc macros are exercised in Steps 7–8.

Use existing temporary mechanism probes as starting evidence, not as proof of
a completed implementation. Freeze surface naming, conditionals, supported
scope, framework-root overrides, and concrete machine association grammar.
Do not add user namespace overrides to rescue a failed proof.

## Verification

Run the existing workspace gates and independent relevant feature checks to
establish a baseline. Use local static fixtures for research. Keep reference
behavior separate from desired diagnostics; a historical proc-macro panic is
never an acceptance snapshot for the replacement.

The resulting matrix must distinguish passes, expected failures, unproved
mechanisms, editor limits, and migration blockers. Assign each remaining case
to a numbered step. Resolve a blocker that changes architecture here before
writing consumer implementations.

## Exit criterion

The public grammar, validation timing, identity assumptions, operand scope,
feature boundary, stabilization removal, and activation boundary have reviewed
evidence. Report the
baseline and old-patch classification. Obtain approval for this no-commit
checkpoint before Step 2.

## End-of-file checkpoint

**Status:** approved no-commit checkpoint; the user authorized Step 2 on
2026-10-01.
**Evidence:** [Step 1 results and proposed decisions](step-1-results.md),
[static probe workspace](probes/README.md).
**Commit:** none expected; Step 1 is a design-only review checkpoint.
Report its evidence and obtain review approval before starting Step 2.
Never stage or commit anything under `design/`. A completed report is not
commit approval, and no next step starts while this checkpoint is pending.
