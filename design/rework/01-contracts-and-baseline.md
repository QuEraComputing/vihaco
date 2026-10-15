# Step 1: contracts and baseline

**PR:** [PR 1: shared inventories and the dialect producer](pull-request-plan.md#pr-1-shared-inventories-and-the-dialect-producer).

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

**Prerequisites:** none. **Next:** [inventory model](02-inventory-model-and-protocol.md).

**Checkpoint:** Work accepted in [contract.md](contract.md) and
[probes/](probes/README.md). This is a no-commit checkpoint: all changes are
under `design/`. The user authorized Step 2 after this review.

## Outcome

Establish one current behavior contract before restructuring implementation.
Keep a small set of semantic fixtures that can serve as the replacement’s
acceptance tests. This step prevents older design documents from reintroducing
per-entry aliases or the removed `#[selector]` syntax.

## Starting points

Read the [precedence table](README.md#design-baseline-and-precedence), the
[type-tree experiment](../instruction-inventory-type-tree.md), and the current
macro modules listed in the overview on `rob/inventory-test`, including its
reviewed pending draft changes. Follow the
[reference revision note](README.md#source-references-and-branch-context) to
preserve that evidence. The implementation baseline is the then-current `main`;
record its actual APIs and available files before preparing PR 1. Draft source
paths and test results are reference evidence, not an assumption that those
modules or fixtures exist on `main`.

The review has already run the focused commands in the overview successfully.
Record the implementation revision and toolchain when work begins; rerun those
checks if the source has changed. Expand validation only for a concrete gap.

## Contract to record

1. Producers respond once with their complete ordered inventory. Entries contain
   `selector`, `type_route`, and `source_steps`; there is no `version`, `origin`,
   `runtime`, or `surface` transport field.
2. `dialect!` emits unit/tuple runtime and surface structs. Operand mappings
   such as `u32 => i64` remain separate types with no automatic conversion.
3. Components accept a nonempty ordered list of whole dialect module paths:
   `#[instructions { arith, boo, foo }]`. Every listed dialect contributes all
   its instructions in declaration order and requires coverage for every leaf.
   Reject the old individual/group/wildcard selection forms. Adding a dialect
   instruction strengthens every consuming component's coverage obligation.
4. Logical duplicate selectors and duplicate canonical runtime types are
   different checks. Both are errors inside a component. A composite can expose
   one leaf through multiple field paths.
5. Composites export all eligible capabilities independently of an authored
   instruction set. Non-device fields and loadable devices contribute no tree
   mount, inventory entry, or source prefix to the parent.
6. A nongeneric, nonempty instruction-set enum contains one plain logical
   selector path per tuple variant. The target can be a concrete instantiation
   such as `Machine<u8>`. Enum/variant `cfg` remains rejected.
7. Runtime enums stay at the authored scope in the first expansion. Public
   surface names and visibility follow the current naming rule. The authored
   set alone owns the new composite’s `HasInstructionSet` association.
8. Inventories establish availability; hidden trees establish Rust type
   resolution. Generated component trees and inventories contain the same
   complete dialect leaves. Source aliases do not change selector identity.
9. Default inventory IDs come from declarations. A component/composite ID
   override changes its sibling namespace and backing macro; a dialect ID
   override changes its backing macro but preserves `dialect::__private`.

Keep the existing feature configurations while rebuilding. Their retirement is
a distinct migration step rather than an accidental consequence of moving files.

## Characterization matrix

Use the existing tests as the starting fixtures:

| Behavior | Existing evidence | Additional case |
| --- | --- | --- |
| Leaf types, mapped operands, patterns | `dialect_macro.rs` | Duplicate pattern attributes and raw identifiers. |
| Complete dialect membership, order, generics | `component_attribute.rs` (migrate draft syntax) | Dialect-list order, declaration order, duplicate dialects, and a newly added uncovered leaf. |
| Recursive lifting and loadable exclusion | `composite_inventory.rs` | Generic ancestor namespace rejection and field helper duplication. |
| Cross-crate paths and aliases | `inventory_cross_crate.rs` | Two distinct same-named dialects imported under different aliases; bare external dependency roots without an explicit `use`. |
| Separate provenance trees | `inventory_type_tree.rs` | Complete same-named dialects with disjoint leaves; any overlapping leaf is a collision. |
| Selected parsing and manual dispatch | `instruction_set.rs` | An omitted dialect is unavailable; an available leaf omitted from the authored set does not parse; contradictory runtime tree route. |
| Arbitrary branch count | `instruction_set_many.rs` | Exact variant result, not just successful parsing. |
| Errors | Four existing compile-fail harnesses | Duplicate selectors inside one received inventory, and no panic in a malformed finalizer. |

Store successful new cases as ordinary integration tests and expected
rejections as trybuild fixtures. Keep experimental failing cases separate
until the corresponding fix lands; do not commit a deliberately red workspace.
Label the issue and expected behavior in the step record.

## Resolve the selector-tree question with a small probe

The component first expansion knows authored dialect paths but cannot infer
their declared names through Rust imports. Current `__selectors` merges roots
by the terminal authored name. For aliases `a` and `b` of two different `cpu`
dialects, both trees export a canonical `cpu` root, while the grouping sees
`a` and `b`. That combination needs a reproduction before treating alias
support as complete.

Probe direct same-named paths, one renamed dialect, two renamed same-named
dialects, repeated dialects through different aliases, and collisions anywhere
in the complete dialect inventories. Check both compilation and navigation from authored
`field::cpu::Leaf` tokens.

The recommendation is to keep first-expansion provenance trees and build the
canonical selector view separately. If imports conceal the canonical root,
test a narrowly staged merge driven by inventory data or a small dialect
mount helper. Choose the least machinery that passes the editor probe. Do not
add declared-name metadata to every entry, change canonical selectors to import
aliases, or move all runtime declarations into finalizers to bypass the issue.
If neither candidate preserves navigation, record the limitation and an
explicit API decision before the component tree implementation lands.

## Attribute and migration probes

Use the exact accepted brace-list grammar `#[instructions { arith, boo, foo }]`.
The compiler probe during plan revision was reported as rejecting
`#[instructions = { ... }]` before the macro ran. Step 1's consuming-macro
probe on Rust 1.96 accepts both forms; the reference rejects the equals form
in its helper parser. The contract still accepts only the brace-list form.
`syn` can parse it as a list attribute with opaque tokens, then parse
comma-separated module paths.
Keep this grammar simple; no expression-block parser or preprocessing is needed.

Inspect item `cfg` ordering, `cfg_attr`, derives, visibility, field attributes,
and helper consumption on components/composites. Recommended policy: preserve
ordinary struct attributes and ensure item-level conditions gate the entire
expansion; initially reject conditional device/loadable membership unless its
inventory and tree output can be kept consistent. Instruction sets retain their
current explicit conditional-variant restriction.

Inventory declarations should remain module-scope items. Probe function-local
use, then document or diagnose the supported boundary; exported macros and
sibling inventories must not pretend to be locally scoped resources.

Inventory existing partial-component declarations and migrate them to complete
dialects, adding missing `Execute` impls or defining a genuinely smaller dialect
when the component should support a narrower vocabulary. Do not retain a
per-component instruction filter as a migration escape hatch.

Inventory the old `component!` and feature-off composite consumers. Record
whether retiring the mode switches can be completed inside Step 12 without
adding routing or bytecode semantics. Keep that migration bounded.

## Deliverables and exit criterion

Add the focused missing fixtures, a concise contract record in this directory,
and any small expansion/editor reproduction needed for a decision. Note which
cases are established and which still need their implementation step.

Exit when the supported syntax, protocol, naming, ordering, conditional-item
policy, and selector-tree approach are explicit. The next steps should implement
those decisions rather than discover them inside quote templates.

## Implementation checkpoint

Step 1's contract, migration inventory, toolchain/revision record, eight semantic
tests, sixteen diagnostic fixtures, and editor/attribute probes are complete in
[contract.md](contract.md) and [probes/](probes/README.md). The required repository
gates, focused reference baseline, and a fresh isolated probe run passed.
The mount-helper output shape preserves navigation; generalized helper grouping
remains an explicit Step 6 implementation check.

No commit is needed or permitted for this step: all changes are under
`design/`. The user accepted this checkpoint by authorizing Step 2.
PR 1 includes separate implementation commits for Steps 2–5 only.
