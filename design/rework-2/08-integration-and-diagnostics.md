# Step 8: CPU migration, cross-crate integration, diagnostics, and editor evidence

**PR:** [PR 5: integration, migration, and cleanup](pull-request-plan.md#pr-5-integration-migration-and-cleanup).

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
**Prerequisites:** Steps 3–7 committed and active; static fixtures available.
**Next:** [Step 9](09-migration-cleanup-and-release-gates.md).

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

Migrate the shipped CPU to the typed declarations and deliver usable CPU support
in both mutually exclusive cfg-selected feature modes. Close the gap between
isolated mechanism proofs and actual public macros using real CPU consumers.
Users should get ordinary Rust type behavior and errors about VM declarations,
not advice to locate modules or understand encoded keys.

## Integration matrix

Cover facade and direct runtime; renamed Cargo dependencies; chained reexports;
module/use/type aliases; two same-named declarations in different modules;
generic/associated child types; lifetime/const defaults and original where
clauses; nested aliases; complete overlap outside selection; loadable boundaries;
custom framework roots; required/sparse/parent-local device codes and keys
stable under reordering; complete versus selected order; syntax::AuthoredEnumName
identity/visibility and module-name collisions; and supported scopes.
Instruction sets use module scope for syntax reexports; function-local
component/composite cases remain separate. Preserve real authored payload and
framework-root scope behind the syntax reexport.

Use fixed producer/consumer fixture crates with checked-in manifests/source.
Keep fixture locks/artifacts out of commits where inappropriate. Run separate
fixture graphs through top-level commands or CI jobs. Tests must not write
source/build artifacts or invoke custom Cargo/rustc compilation runners. Avoid
parallel suites that duplicate behavioral tests already owned by a macro.

## CPU execution source and stack constraint

Use the `impl Execute<...> for CPU` blocks in the sibling
`../vihaco-execute-bench` worktree's `crates/vihaco-cpu/src/component.rs` as the
source for the typed CPU execution implementations. This means the actual
working-tree files, including their pending edits, rather than only committed
HEAD. At this scope amendment, the worktree is on
`rob/execute-directly-experiment`, HEAD
`a0a36b13726fa1639c809da863324942ac0452b7`, with uncommitted CPU changes.
Read it without modifying its files or history. Port the relevant execution
blocks selectively and adapt them to this plan's current `Execute<I>` contract;
do not cherry-pick the whole experiment or import its unrelated API changes.

Preserve `CPU::stack` exactly as represented on main: `pub(crate) stack:
Vec<Word>` with the existing `Word` representation, in both feature modes.
The reference main revision is `cf70ca8e2f27a844329cbe24542873bf648204de`;
its CPU `data.rs` and `component.rs` own the safe stack behavior to retain.
Do not import the benchmark worktree's `Stack<Word>`/`Stack<T>` wrapper, separate
stack pointer, unchecked indexing, or unsafe stack implementation. This step
changes the execution declarations, not the stack representation.

Replace every unsafe operation encountered in the reused execution blocks with
its safe counterpart from main. Use the existing checked stack/local access
helpers and safe Vec operations, preserving operand checks, stack-underflow and
bounds errors, frame/operand boundaries, operand order, and instruction results.
For example, translate unsafe `pop`, `replace_top_two_with`, and
`try_replace_top_two_with` paths to main's safe pop and arithmetic helpers or
equivalent checked safe operations. Removing an `unsafe` block alone is not a
port of its semantics. Do not assume verified bytecode permits skipping main's
runtime checks, and do not reimplement an unsafe operation behind a safe signature.

Verify normal execution and error cases in both CPU modes, including operand
underflow and invalid local access. Review the migrated CPU diff against main to
confirm `CPU::stack` remains `Vec<Word>` and no benchmark stack machinery or
unsafe operations were introduced. These constraints are part of Step 8's exit
criterion and must survive Step 9 cleanup.

## Implementation tasks

1. Implement the `vihaco-cpu` migration, including its dependencies and feature
   forwarding. Add a nondefault `vihaco-cpu/typed-declarations` feature that
   forwards to `vihaco/typed-declarations`. Gate the legacy CPU declarations,
   instruction types, and mode-specific implementations/tests with
   `#[cfg(not(feature = "typed-declarations"))]`; gate the typed replacements
   with `#[cfg(feature = "typed-declarations")]`. Select public exports under
   the same flag and share ordinary state/execution helpers where appropriate.
   Only one version is compiled in each mode. The same CPU crate must build and
   be usable in both the existing feature-off graph and the new feature-on graph. A dependency can enable CPU's
   feature indirectly through Cargo unification; that graph must also work.
   Enabling the facade/runtime feature alone does not set CPU's local cfg flag.
   Document coherent opt-in for graphs containing CPU: enable CPU's feature,
   which forwards the required framework feature. Include a runtime-only opt-in
   graph to expose and document the unsupported mixed configuration; do not
   advertise automatic reverse feature propagation. Coherent graphs must not
   leave CPU's `component!` invocation targeting an attribute macro or break
   CPU composites.

   - Declare actual public CPU instruction leaves in complete typed dialects,
     with complete `Execute<I>` coverage and ordinary operand types. Preserve
     the existing CPU state, host-VM interfaces, and execution behavior. Reuse
     the sibling benchmark worktree's `impl Execute<...> for CPU` blocks under
     the execution-source and safe-stack constraints above.
   - Review the distinction between parsed symbolic operands and resolved encoded
     instructions. Use ordinary authored resolution and encoding code to bridge
     those stages using the user-amended `$0:SourceType` pattern annotations.
     Unannotated bindings retain their types; typed bindings generate
     distinct source/runtime operands and require explicit consumer resolution.
     Do not introduce a CPU resolution context, inventory protocol, compatibility
     adapter, or unresolved executable operand wrapper.
   - Preserve encoded opcode values, payload widths/layouts, decode/encode
     behavior, and section routing. Cover the CPU's existing source patterns,
     typed operations, sugar, labels, calls, constants/string references, and
     function resolution; document any deliberate feature-on source/API changes.
   - Integrate the migrated CPU with authored selected instruction sets, nested
     and aliased composite targets, `HasInstructionSet` where needed, explicit
     resolution/execution, messages, `StepOutcome`, effects, observers, and
     bytecode/SST multisection loading. Update `GeneratedComponent`/dispatch
     consumers and required association bounds in their owning modules.
   - Preserve feature-off CPU behavior while the gate exists. Each mode selects
     its implementation directly; do not introduce renamed legacy macros,
     wrappers, or compatibility exports to keep old declarations alive in the
     new mode. Removing the feature-off implementation remains stabilization work.
   - Add ordinary integration tests and a compiled public example using the
     shipped CPU, not a toy replacement. Assert actual parsed/resolved variants,
     destinations, CPU state, control-flow outcomes, and effects. Verify encoded
     round trips and loader/section boundaries in both supported feature graphs.
2. Extend actual cross-crate fixtures with type/parsed-value assertions for the
   complete integration matrix. Prove imported aliases need no override and
   same-named declarations need no unique exporter ID.
3. Add reviewed trybuild fixtures for malformed declarations and semantic
   failures: missing full coverage, undeclared dialect, wrong target, complete
   overlap, duplicate selected pair, section exclusion, and ambiguous spelling.
4. Improve primary errors using semantic wrapper diagnostics and preserved
   spans where demonstrated useful. Hide irrelevant blanket-impl suggestions.
   Check compiler notes honestly; do not promise that every helper name vanishes.
5. Test cargo check and actual codegen/use for concrete generic validation.
   Confirm declaration-local source checks happen even if no set is defined.
   State the supported timing rather than advertising universal eager generic
   validation.
6. Run actual rust-analyzer hover/completion/definition on component dialect
   paths, real instruction payloads, target fields where supported, and paired
   surface types. Record version and operations. Output-shape probes alone
   do not establish proc-macro behavior.
7. Exercise a bounded large dialect/deep composite/many-choice fixture. Check
   coverage/const validation/parser build cost for avoidable regressions.
   Improve demonstrated hotspots without speculative trie or reflection machinery.
8. Fix owning modules and associated docs as needed; do not mask compiler/editor
   failures with blanket lint allowances or accepted unresolved names.

## Verification

Review snapshots line by line and run the actual fixture crates offline when
dependencies are present. Keep shared-source fixtures static.
Exercise isolated `typed-declarations` off/on configurations through both
public roots. Also test a graph where another dependency enables the feature:
Cargo unification selects the experimental contract for the shared instance.
Verify coherent support/macro forwarding; do not promise simultaneous old/new
contracts. Current default consumers and opt-in new consumers have separate
build/test fixtures, without compatibility macros or adapters. For CPU consumers,
indirect opt-in must enable `vihaco-cpu/typed-declarations`; a dependency's enabled
feature cannot turn on the CPU crate's local cfg flag in reverse.

Run the default workspace gates and feature-on clippy, all-target tests, and
doctests with CPU included. Enable `vihaco-cpu/typed-declarations` for the feature-on
graph so its cfg selects the typed CPU and forwards the facade/runtime feature.
CPU exclusions used in Step 7 no longer satisfy this step. Remaining unmigrated
legacy documentation callers may use a documented separate feature-off graph.
Run the shipped CPU's own all-target tests/doctests in both modes and the public
CPU example, including actual encoded round trips and section loader behavior.

Repository gates remain required before committing. Record editor behavior
separately from compiler correctness and list any bounded limitation at an
authored path. No source claims should say users need to know a helper tree.

## Exit criterion

The shipped CPU's legacy and typed versions are selected by mutually exclusive
cfg feature gates and work in their coherent Cargo graphs. Typed execution
uses the benchmark worktree's per-instruction implementations adapted to this
contract, while both modes retain main's `CPU::stack: Vec<Word>` and safe
operations/checks. No unsafe benchmark stack implementation is ported. Actual public CPU
consumers pass parsing/resolution/execution, encoding, effect/observer, and loader
checks. CPU is included in the feature-on test/doc/lint graph. Actual generated
declarations pass the integration matrix with reviewed semantic diagnostics and
recorded editor evidence. Remaining limits are
specific and do not require user knowledge of library internals.
Report the eligible test/fix/doc changes and proposed dedicated commit.

## End-of-file checkpoint

**Status:** implemented; awaiting user review and explicit commit approval.
See [Step 8 results](step-8-results.md) for eligible files, checks, and editor limits.
**Proposed dedicated commit:** `feat: migrate CPU to gated typed declarations and verify integration`.
Report completed implementation, eligible files, checks, and material limits;
wait for explicit approval before committing. If the final public change is
breaking, adjust the conventional marker/footer to the reviewed diff.
Never stage or commit anything under `design/`. A completed report is not
commit approval, and no next step starts while this checkpoint is pending.
