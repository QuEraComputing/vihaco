# Pull request plan

Group the nine review steps into five stacked PRs:
**instruction relationships/dialects → components → typed composite support
→ instruction sets/parsing → integration/migration**.

There are eight expected new implementation commits, one per Step 2–9.
Step 1 is a no-commit review checkpoint. Preserve approved `fbfec16` and WIP
`7d36633` on `rob/shared-inventories-and-dialect`. The new branch starts at
main and inherits neither commit. Port useful behavior selectively according
to the [Step 1 classification](step-1-results.md#classification-of-the-prior-work).

Never stage or commit any file under `design/` at any point in this plan,
including plans, contracts, probes, fixtures, and results. Exclude them from
every PR. Do not relocate design artifacts or create empty commits to satisfy
the expected count.

Each step needs its own completed review and explicit commit approval before
the next step starts. See [workflow](README.md#per-file-review-and-commit-workflow).
PR review does not replace these checkpoints. Do not combine or squash commits
without explicit approval.

## Branch bases and PR targets

The implementation branch is `rob/types-and-dialect-declarations`, reset by
the user to main at cf70ca8. The prior shared branch supplies read-only audit
material. Do not import its WIP commit wholesale or rewrite either branch.

| PR | Suggested branch | Create from | Initial target |
| --- | --- | --- | --- |
| 1 | `rob/types-and-dialect-declarations` | `main` at cf70ca8 | `main` |
| 2 | `rework-2/component-dialects` | PR 1 branch | PR 1 branch |
| 3 | `rework-2/composite-fields` | PR 2 branch | PR 2 branch |
| 4 | `rework-2/instruction-sets` | PR 3 branch | PR 3 branch |
| 5 | `rework-2/integration-migration` | PR 4 branch | PR 4 branch |

Inspect the current base and pending diff before creating branches. Preserve
approved history and unrelated user work. The reference draft is read-only
behavioral evidence, never a base to merge wholesale.

Review against the immediate predecessor. Merge from the bottom of the stack.
After a predecessor merges, rebase/retarget dependents in order, preserve
separate reviewed step commits, and rerun affected checks.

## Requirements for every PR

Each PR contains its meaningful source changes, focused static fixtures,
reviewed diagnostic snapshots, and public docs for the behavior it activates.
Every step commit is buildable and passes the repository gates in the overview.
Run relevant feature configurations, direct-runtime/facade checks, and actual
cross-crate tests in the owning PR, not only at the end.

Keep runtime and surface payload paths resolved by Rust. Reject any change
that requires users to supply a companion namespace or globally unique ID.
Gate all new behavior under nondefault `typed-declarations`, forwarding it
coherently through facade/runtime/derive. Feature-off behavior stays current;
feature-on marker/composite contracts are experimental. Test isolated graphs
and indirect feature unification. Do not introduce public placeholder macros,
alternate token protocols, renamed legacy macros, or compatibility adapters.

Use draft PRs once there is a concrete reviewable diff. Before marking ready,
finish all assigned steps, reconcile the public migration boundary, and update
the title/body around the final implementation. Summarize changed behavior,
validation, and material limits. Breaking changes require a conventional
breaking marker/footer and the release process; do not publish by hand.

## Test implementation policy

Do not add custom test harnesses that write generated source or build artifacts,
invoke Cargo/rustc subprocesses, or execute generated fixture binaries. This
includes rustc-stdin compilation and nested Cargo feature-graph runners.
Use in-process parsing/emission assertions, ordinary unit/integration tests and
doctests, and established trybuild fixtures for compiler diagnostics. Run
independent feature graphs and checked-in fixture crates as top-level
verification commands or CI jobs, never from inside a test. Do not reintroduce
the removed compilation runners in a later step.

## PR 1: typed instruction relationships and dialect declarations

**Steps:** [1](01-contracts-and-baseline.md),
[2](02-runtime-associations-and-validation.md), [3](03-dialect.md).
**Base:** `main` at cf70ca8; prior preparation is not inherited.
**Suggested title:** `feat: add gated typed dialect instruction associations`.

Establish a small runtime support surface and the first real generated producer.
Preserve authored unrestricted pub `#[instruction]` structs inside inline
`#[dialect]` modules; generate paired surface types, associated surface/dialect identity, ordered static
descriptions, and whole-dialect coverage relationships. Include any new
`Execute<I>` contract needed by those relationships, independently of legacy
generated-component execution.
Enable new Component/Composite markers without HasInstructionSet supertraits
only under the feature. Preserve current default marker semantics; shared
consumers needing associations use explicit bounds without adapters.

Merge criteria:

- Inline `#[dialect]` modules with `#[instruction]` unit/tuple structs work
  through facade and runtime with renamed deps, preserving ordinary Rust items.
- User operands retain correct Rust identity and parsing behavior.
- Ordered descriptions and validation have semantic tests.
- No exported inventory macro, callback collector, type tree, or namespace
  inference is introduced.
- Existing feature-off behavior/docs remain working; new tests opt in.
- Private/restricted instructions reject; no operand remapping helper is added.
- Step 1 has an approved no-commit checkpoint; Steps 2 and 3 each have an
  approved dedicated implementation commit.

## PR 2: complete component capabilities

**Step:** [4](04-components.md).
**Base:** PR 1.
**Suggested title:** `feat: declare complete component dialect capabilities`.

Activate the complete component attribute behind the feature in one step.
Generate declared support,
complete coverage under original bounds, and ordered descriptions. Adapt marker
supertrait assumptions explicitly if the actual base requires it.

Merge criteria:

- Only whole-dialect component declarations are supported.
- Missing execution for any leaf fails, including a newly added/unselected leaf.
- Imported dialect aliases preserve identity; repeated aliases and complete
  overlapping canonical selectors reject; same-head disjoint leaves work.
- Generic coverage is proved rather than assumed in added bounds.
- No component aggregate instruction enum or user namespace override is required.
- Affected callers/docs and all feature checks pass.

## PR 3: typed composite fields and complete capabilities

**Step:** [5](05-composites.md).
**Base:** PR 2.
**Suggested title:** `refactor: prepare typed composite capability traversal`.

Implement field-type associations, recursive capability descriptions, aliases, and section
boundaries with real compiler fixtures. This PR may keep the new composite
emitter internal because the existing public macro must remain working until
PR 4 activates the complete authored-set/parser API. Its title and docs must
say preparation if public activation has not occurred.

Merge criteria:

- Nested actual child types, including aliases and associated/generic fields,
  work without locating any companion module.
- Full child availability is independent of child executable selections.
- Ordinary/loadable fields are excluded from parent selection.
- Every device requires an explicit u8 code; eligible Field relations and hidden
  constants use that code directly. Codes remain stable under reordering and
  are unique locally, including loadable devices.
- Static field/code/source/section checks remain complete.
- Validation timing for generic descriptions is accurately documented/tested.
- Existing public composite and loader fixtures still pass; no future macro or
  fake parser is exposed to make the preparation usable.

## PR 4: authored instruction sets and selected surface parsing

**Steps:** [6](06-instruction-sets.md), [7](07-selected-parsers.md).
**Base:** PR 3.
**Suggested title:** `feat: select gated VM instructions by type and device field`.

Retain real runtime payload types, pair surface types through instruction
associations, validate targets/support/full concrete capabilities, and generate
selected-only parsers. Activate the new composite and set attributes together
under `typed-declarations` when their full public contract works. Keep current
feature-off #[composite] behavior, and exercise current and experimental callers
in separate builds. No compatibility export or wrapper is introduced. Document
indirect activation through Cargo feature unification.

Merge criteria:

- Authored enum payload navigation retains real instruction paths.
- Runtime/surface variant identity and HasInstructionSet association agree.
- Whole-machine overlap checks cannot be bypassed by selecting only a safe leaf.
- Nested aliases, omitted leaves, malformed operands, and large selected sets
  behave correctly in actual parsers.
- Concrete generic machines and payload/type aliases work across crates.
- Base use requires only the core VM declarations and existing runtime contracts.
- Current public callers/ABI/sections pass with the feature off; new callers
  pass with it on, including dependency-driven activation.
- Both steps have separate review/commit approval; internal intermediate work
  does not become a partial public attribute.

## PR 5: integration, migration, and cleanup

**Steps:** [8](08-integration-and-diagnostics.md),
[9](09-migration-cleanup-and-release-gates.md).
**Base:** PR 4.
**Suggested title:** `refactor: complete typed VM declaration migration`.

Close actual compiler/editor/diagnostic gaps, update remaining callers/guides,
and remove any unused private preparation or allowances accidentally ported
from the prior branch; it is not inherited on the new branch.
Implement migration of the shipped `vihaco-cpu` in Step 8 so it works with
`typed-declarations` enabled, including indirect activation, while preserving its
feature-off behavior and encoded opcode/section semantics. Include a public
feature-on CPU consumer/example and actual resolution/execution/loader coverage.
Gate the legacy CPU under `cfg(not(feature = "typed-declarations"))` and the
new CPU under `cfg(feature = "typed-declarations")`; exports and mode-specific
tests use the same flag. Add the nondefault CPU feature forwarding to the
facade/runtime. Test indirect activation of CPU's feature and document that
runtime-only opt-in does not enable CPU's local feature in reverse. Step 9
preserves the completed migration during final cleanup. CPU migration cannot
be deferred to stabilization or replaced by an audit. Use the per-instruction
`impl Execute<...> for CPU` blocks from `../vihaco-execute-bench`, adapted to the
current contract. Preserve main's `CPU::stack: Vec<Word>` and checked safe
operations in both modes; replace unsafe operations and exclude the benchmark
stack representation and stack-pointer machinery.
Keep the nondefault feature and current default implementation during
stabilization, without compatibility exports or adapters. Record migration work
for remaining consumers. After stabilization, migrate those consumers and remove
the old implementations and gate rather than maintaining two permanent APIs.

Merge criteria:

- Public tests cover renamed dependencies/imports, type aliases, generic bounds,
  section boundaries, declaration order, and semantic diagnostic wording.
- Actual macro expansions have recorded editor evidence.
- Documentation uses only VM concepts and ordinary Rust paths.
- No active transport protocol, namespace inference, or generated-tree dependency
  remains in the new implementation.
- The shipped CPU works in both feature modes and under indirect feature
  unification. Public consumers exercise parsing, authored resolution, selected
  execution, effects/observers, encoded round trips, and multisection loading.
- CPU's stack remains main's `Vec<Word>` in both feature modes. Reused benchmark
  execution blocks use safe counterparts for every unsafe operation; stack/frame
  underflow and bounds checks retain their current behavior.
- Final feature-on test/doc/lint checks include CPU; CPU exclusions used before
  its migration do not satisfy this PR's merge criteria.
- All five repository gates and affected feature configurations pass.
- No `design/` file appears in any commit or PR diff.

## End-of-file checkpoint

This PR plan is a local coordination document. No branch, commit, push, or PR
has been created for it. Implementation begins only after the replacement
contract checkpoint is reviewed. Never stage or commit anything under `design/`.
