# Step 7: selected surface parsers and public activation

**PR:** [PR 4: authored instruction sets and selected surface parsing](pull-request-plan.md#pr-4-authored-instruction-sets-and-selected-surface-parsing).

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
**Prerequisites:** Step 6 committed; complete emitter/migration policy reviewed.
**Next:** [Step 8](08-integration-and-diagnostics.md).

## Test implementation policy

Do not add custom test harnesses that write generated source or build artifacts,
invoke Cargo/rustc subprocesses, or execute generated fixture binaries. This
includes rustc-stdin compilation and nested Cargo feature-graph runners.
Use in-process parsing/emission assertions, ordinary unit/integration tests and
doctests, and established trybuild fixtures for compiler diagnostics. Run
independent feature graphs and checked-in fixture crates as top-level
verification commands or CI jobs, never from inside a test. Do not reintroduce
the removed compilation runners in a later step.

## Outcome

Complete the paired surface parser and activate the new declaration API under
nondefault `typed-declarations`.
Each branch uses an actual instruction association and its target field
metadata. Parsing needs no full-child enumeration or nested child surface enum.

## Parser algorithm

For each authored choice, in enum order:
1. Obtain each target field's canonical source name and aliases from its typed
   relation, keyed by the explicit device code, in outer-to-inner order.
2. Parse one spelling plus :: for each field segment.
3. Call the selected instruction's associated Surface::parser().
4. Wrap the concrete surface payload in the matching authored surface variant.

Combine only selected branches. Stably prioritize longer associated source
mnemonics for parser alternatives, preserving authored variant and description
order. Associated aliases can be static slices, so
build a small boxed alternative chain where needed. Each alternative includes
its separator before .or; a shorter name must not commit before a longer alias
sharing its prefix. Avoid constructing all alias combinations in macro tokens.

## Implementation tasks

1. Keep parser emission separate from declaration/type checks and input parsing.
   It consumes authored choices plus shared projected associations, not external
   descriptor tokens or a flattened full inventory.
2. Use the existing Parse trait/Chumsky error contract and associated surface
   type. Box branches at the narrow boundary needed for heterogeneous parser
   composition; support sets beyond tuple-choice limits.
3. Preserve dialect grammar, operands, aliases, backtracking, whitespace, and
   source framing. Test real consumed values and variants, including malformed
   and trailing input; do not accept a shorter alternative accidentally.
4. Implement SurfaceInstruction integration required by typed SST parsing.
   The generated VM surface enum is publicly accessed as syntax::AuthoredEnumName,
   reexporting the hidden enum from its authored scope like dialect leaves.
   Generate the complete selected parser without manual operand conversion;
   do not reintroduce a public SurfaceInstruction suffix or move payload types
   into the syntax module.
5. Activate #[instruction_set] and the new #[composite] behavior together only
   when the full public contract compiles. Reexport through facade/direct
   runtime under the coherent `typed-declarations` gate; preserve item-level
   gating. Feature-off #[composite] keeps its current emitter.
6. Keep existing encoded/CPU/aggregate/loadable examples in feature-off builds;
   test new declarations in separate feature-on builds. Audit shared runtime
   and loader consumers for required association bounds. Do not introduce
   renamed legacy macros, compatibility wrappers, or adapters. Document the
   feature-unification boundary and the plan to remove the old implementation
   after stabilization; old declarations need not compile in the new mode.
7. Add a complete opt-in public example: a `#[dialect]` module containing
   unrestricted pub `#[instruction]` structs and ordinary operands, complete
   component
   coverage, nested/aliased fields, explicit selected ISA, Parse, authored
   Resolve, and ordinary explicit execution. No inventory/helper path appears.
8. Repeat actual macro cross-crate and editor smoke checks now. Internal fixture
   parsing/emission checks from Steps 5–6 are not sufficient evidence of real
   public expansion. Run separate fixture graphs through top-level commands.

## Verification

Assert canonical and aliased nested prefixes yield the same selected variant
and operand. Omitted leaves and routes reject despite full availability.
Test negative and identifier operands, punctuation/Unicode aliases, shared prefixes,
unit leaves, many branches, parser errors, and typed module construction.

Compile facade/runtime/renamed dependencies, type aliases, generic concrete
machines, raw fields, and item-level conditions through actual macros.
Run current bytecode/loader tests and workspace doctests with the feature off;
run new examples/tests/doctests with it on. Include indirect opt-in coverage.
Verify same actual instruction selected at two fields preserves runtime and
surface variant identity; aliases do not change destination.

## Exit criterion

Public component/composite/instruction-set declarations work end-to-end with
selected-only parsing and explicit application resolution/execution.
The experimental public path requires the feature opt-in, VM concepts, and
ordinary Rust names.
All owning migration/docs/gates pass; report and wait for dedicated commit approval.

## End-of-file checkpoint

**Status:** approved and committed as `c38b7c8`.
See [Step 7 results](step-7-results.md) for eligible files, checks, and limits.
**Proposed dedicated commit:** `feat: activate gated typed VM declarations and selected parsing`.
Report completed implementation, eligible files, checks, and material limits;
wait for explicit approval before committing. If the final public change is
breaking, adjust the conventional marker/footer to the reviewed diff.
Never stage or commit anything under `design/`. A completed report is not
commit approval, and no next step starts while this checkpoint is pending.
