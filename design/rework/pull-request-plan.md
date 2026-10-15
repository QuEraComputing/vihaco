# Pull request plan

Group the twelve work steps into five PRs. Merge them in dependency order:
**shared inventory/dialect → components → composites → instruction sets →
migration closure**. The numbered step files provide implementation detail;
these PR boundaries define what a reviewer receives and what must work at merge.

There are twelve review checkpoints and eleven expected implementation commits:
four in PR 1 (Steps 2–5),
two in PR 2, one in PR 3, two in PR 4, and two in PR 5. Follow the
[per-file review and commit workflow](README.md#per-file-review-and-commit-workflow):
finish one numbered file's work, report it for user review, obtain explicit
approval to commit eligible changes, and create that commit before starting the
next file. Design-only steps instead require review approval and a no-commit
checkpoint; Step 1 has no eligible changes to commit.

Never stage or commit any file under `design/` at any point in this plan,
including plans, contract records, probes, fixtures, and results. Exclude them
from every commit and PR. Do not move local planning artifacts elsewhere or
create empty commits merely to satisfy a step's commit boundary.

Create PR 1 from `main` and stack each later PR on its predecessor. Keep shared
infrastructure changes in PR 1 unless a consumer exposes a
specific missing operation. A follow-on PR can add that operation with its
consumer tests without introducing a second inventory protocol.

## Branch bases and PR targets

**Reference code:** `rob/inventory-test`, including the reviewed working-tree
changes described in the [source reference note](README.md#source-references-and-branch-context).
**Implementation root:** the then-current `main`.

The branch names below are suggested names for new implementation branches;
none is a base on the draft branch.

| PR | Suggested implementation branch | Create from | Initial PR target |
| --- | --- | --- | --- |
| 1 | `rework/inventory-dialect` | `main` | `main` |
| 2 | `rework/component-dialects` | `rework/inventory-dialect` | `rework/inventory-dialect` |
| 3 | `rework/composite-inventory` | `rework/component-dialects` | `rework/component-dialects` |
| 4 | `rework/instruction-sets` | `rework/composite-inventory` | `rework/composite-inventory` |
| 5 | `rework/migration-cleanup` | `rework/instruction-sets` | `rework/instruction-sets` |

Review each PR's diff against its immediate base. Merge from the bottom of the
stack. After a predecessor merges, rebase and retarget the remaining PRs onto
the updated base, preserving the dependency order and rerunning affected checks.
Read draft source through a separate reference checkout or recorded revision
when a path is absent from the implementation checkout.

## Requirements for every PR

Keep each numbered file's eligible changes outside `design/` in its own reviewed
and approved commit. Record a no-commit checkpoint for design-only work.
PR-level review does not replace the user checkpoint after each step. Do not
combine or squash step commits without explicit user approval.

Include focused behavioral tests, reviewed compile-fail fixture changes, and
documentation for the behavior changed in that PR. Run the repository gates in
[Step 12](12-migration-cleanup-and-release-gates.md#required-checks) and the
relevant independent feature checks. Use
[Step 11](11-diagnostics-cross-crate-and-editor.md) for cross-crate and editor
checks wherever tree placement or authored paths change.

Migrate affected callers in the PR that changes their contract. Keep the
workspace and doctests passing at each merge boundary. There should be no
temporary public syntax or parallel inventory grammar. Any mode/feature boundary
needed to preserve `main`'s consumers must be the explicit migration boundary in
this plan, with tests and a recorded retirement decision.

Each PR description should state its concrete outcome, the meaningful contract
changes, validation performed, and any unresolved editor or migration limits.
Use the suggested titles below as starting points and adjust them to the final
diff. Breaking public changes need a conventional breaking marker or footer.

## PR 1: shared inventories and the dialect producer

**Primary steps:** [1](01-contracts-and-baseline.md),
[2](02-inventory-model-and-protocol.md), [3](03-paths-names-and-options.md),
[4](04-transport-and-type-trees.md), [5](05-dialect.md).
**Dependency/base:** `main`; source reference `rob/inventory-test`.
**Suggested title:** `refactor: centralize instruction inventories and rebuild dialect expansion`.

### Scope

Establish the contract/probes, then replace the inventory model, protocol
implementation, path/naming support, passive exporter, callback collector, and
type-tree helpers. Rebuild `dialect!` as the first complete producer using that
infrastructure. This gives the shared system a real user in the same PR.

Adapt consumers actually present on `main` wherever the shared changes require
it. The draft's component attribute, composite inventory, and instruction-set
consumers are not on the inspected `main`; introduce their clean implementations
in PRs 2–4. Do not import the draft's partial component API or `select_names`
helper just to remove them in PR 2.

Review this PR as one infrastructure change with a leaf producer demonstrating
the contract. Step 1 has a completed and approved no-commit review checkpoint;
Steps 2–5 each have a separate completed, reviewed, and approved implementation
commit. Internal preparation must keep each commit buildable and the complete
public API working at merge.

### Merge criteria

- One validated model, grammar, exporter, and collector are used by all active
  inventory producers and consumers.
- Dialect output preserves leaf types, operand mappings, parser behavior,
  declaration order, and cross-crate runtime/surface identity.
- Path rebasing, custom roots/IDs, imported aliases, and malformed inventories
  have focused coverage; the selector-root probe's decision is recorded.
- Existing `main` consumers and doctests still build, while the new dialect
  producer and inventory fixtures exercise the shared system.
- Source/guide changes describing this PR's behavior are included; component
  membership is introduced with the whole-dialect contract in PR 2.

## PR 2: whole-dialect component contracts

**Primary steps:** [6](06-component-input-and-first-expansion.md) and
[7](07-component-finalization.md).
**Dependency:** PR 1.
**Suggested title:** `feat!: require complete dialects for components`.

### Scope

Implement the component attribute with the nonempty dialect list
`#[instructions { arith, boo, foo }]`. Keep the initial expansion and finalizer
in this PR: ordered dialect paths, immediate complete trees, uniform `Execute<I>`
coverage, duplicate validation, and component inventory export form one contract.
Step 6 commits the tested internal declaration/first-expansion implementation;
Step 7 completes the finalizer and activates the new public attribute. Review
and approve each step before its separate commit; sharing a PR does not combine
their work or review checkpoints.

The reference's named selection, wildcard modes, selection tags, overlap and
unknown-leaf logic, `select_names`, and direct explicit coverage are omitted from
the clean implementation. Remove any such machinery already present on the
actual stack base rather than adding compatibility support. Every declared
dialect contributes every leaf in declaration order, and adding a dialect leaf
adds coverage obligations to all consuming components.

Migrate all affected component declarations and missing `Execute` impls now,
including composite/instruction-set fixtures, the cross-crate producer crate,
direct-runtime tests, and compiled guide examples when present. Port relevant
draft fixtures directly to whole-dialect syntax; fixtures requiring PRs 3–4's
new macros land with those PRs rather than making PR 2 depend on later output.
Rewrite obsolete component
diagnostic fixtures around the new grammar and complete coverage contract.
Do not postpone those changes to PR 5 or weaken coverage to preserve a fixture.

### Merge criteria

- Whole-dialect syntax is the only supported component declaration contract.
- Component trees and inventories expose exactly the same complete leaves.
- Missing coverage fails for any leaf, including one omitted by all downstream
  authored instruction sets and one covered only for a narrow generic parameter.
- Repeated dialect aliases and complete-inventory collisions are rejected;
  distinct same-named dialects with disjoint complete leaf sets still work.
- Module-path editor probes, cross-crate exports, updated callers, guides, and
  doctests pass or have explicitly recorded editor verification limits.

## PR 3: recursive composite lifting and section boundaries

**Primary step:** [8](08-composite.md).
**Dependency:** PR 2.
**Suggested title:** `refactor: rebuild recursive composite inventory lifting`.

### Scope

Rebuild composite parsing, field validation, first-expansion mounts, collection,
and finalization around one typed field model. Use the complete component
inventory contract and recursively prefix child selectors, routes, and source
steps in field order.

Preserve loadable section boundaries and existing bytecode/SST loader behavior.
Adapt the retained feature-off legacy expansion to the shared field model while
it exists. Include namespace overrides, generic field handling, and the
one-parse sanitization change in this PR.

On a base where composite inventory mode is new, introduce its planned opt-in
boundary and keep legacy composite consumers working. Do not switch those
consumers to authored instruction sets before PR 4 provides that macro.

### Merge criteria

- Nested and cross-crate composites re-export complete inventories with correct
  order, paired types, source aliases, and sibling route distinctions.
- Empty and loadable-only inventories work; loadable fields trigger no parent
  inventory request or type-tree reference.
- Bytecode and SST own/child section loading and rejection of unexpected
  children retain their tested behavior.
- Device/helper/alias/section validation and relevant feature configurations
  pass; the composite guide describes the resulting behavior.

## PR 4: authored instruction sets and selected parsers

**Primary steps:** [9](09-instruction-set-declarations-and-selection.md) and
[10](10-selected-surface-parsers.md).
**Dependency:** PR 3.
**Suggested title:** `refactor: rebuild authored instruction sets and selected parsers`.

### Scope

Rebuild declaration parsing, immediate runtime enum emission, checked full-selector
selection, paired surface variants, and the `HasInstructionSet` association.
Include parser generation in the same PR so the authored set has a complete
usable API at merge.

Step 9 commits the tested internal declaration, selection, and enum emitters;
Step 10 adds selected parsing and activates the complete public macro. Complete
and report each step for user review, then obtain approval for its separate
commit before moving on.

Resolve both payload trees through shared support, validate selector/runtime
identity, and reject duplicate producer entries before indexing. Compose only
selected leaf parsers with field-prefix alternatives, correct boundaries, and
practical branch/alias counts beyond tuple-choice limits.

Include the compiled end-to-end example: mapped operands, two composite levels,
sibling fields sharing a leaf, explicit resolution, and manual dispatch. Update
instruction/parser guides and affected examples with this PR.

Migrate consumers that can now use the complete authored-set API and settle
default feature exposure together with those callers. Final mode retirement
and any remaining legacy migration stay in PR 5.

### Merge criteria

- Public runtime enums remain at authored scope with the expected names,
  visibility, match diagnostics, and concrete payload identity.
- Available inventory remains complete; authored-set omissions only restrict
  that set's variants/parser and do not restrict parent lifting.
- Selected-only parsing, mixed aliases, prefix-related mnemonics, large choice
  lists, and embedded SST parsing pass with exact variant/value assertions.
- Direct-runtime and cross-crate APIs, selected-path editor probes, and all
  affected compile-fail/doctest examples pass or record editor limits explicitly.

## PR 5: final migration and integration closure

**Primary steps:** remaining [11](11-diagnostics-cross-crate-and-editor.md) audit
and [12](12-migration-cleanup-and-release-gates.md).
**Dependency:** PR 4.
**Suggested title:** `refactor: finish macro migration and remove obsolete plumbing`.

### Scope

Complete the cross-cutting audit: remove residual unused machinery, reconcile
historical design status, finish overview/migration documentation, and record
the final compiler, parser, editor, and feature evidence. Earlier PRs already
own their relevant behavior tests and documentation; this closes remaining
integration gaps.

Use the migration inventory from Step 1 to decide the bounded feature-mode
retirement described in Step 12. If retirement needs no new routing or bytecode
semantics, include consumer migration, marker-trait simplification, feature/export
cleanup, and legacy composite removal together here. Add a breaking-change footer
or change the title to reflect that public behavior change.

If retirement requires that deferred design work, retain and verify the existing
modes, document their exact remaining consumers/blockers, and track their removal
as a separate future task. Removing the older `component!` macro is still a
separate API decision; do not make completing this reimplementation depend on
redesigning aggregate encoding.

### Merge criteria

- The final responsibility/contract audit agrees with the implemented macros;
  no partial component support or duplicate inventory machinery remains.
- Examples, migration notes, public re-exports, Cargo features, and historical
  design status accurately describe the final supported configuration.
- Every required repository gate and relevant independent feature check passes;
  editor evidence includes tool versions and any unverified operations.
- Any retained legacy mode has a concrete follow-up record, and any removed
  mode has its affected consumers migrated in this PR.
