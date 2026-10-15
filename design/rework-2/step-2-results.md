# Step 2: runtime support results

**Recorded:** 2026-10-01.
**Status:** implementation committed as 072166e; stopped before the next step at user request.
**Branch/base:** rob/types-and-dialect-declarations at main cf70ca8.
**Commit:** 072166e, `feat: add gated typed instruction capability contracts`.
Nothing under `design/` was staged or committed. No branch change, push, or PR
has been made.

## Implemented behavior

The nondefault typed-declarations feature forwards from facade to runtime to
derive. Runtime opt-in also enables derive, including with no default features.
Without opt-in the new exports are absent and Component/Composite retain their
HasInstructionSet supertraits. With opt-in the markers are independent of an
executable set. The one current test relying on inherited associated types now
uses an explicit HasInstructionSet bound in both modes.

Execute<I> borrows the instruction and leaves Message, Effect, and Error to each
implementation. No common effect/message shape or adapter to GeneratedComponent
is introduced. Existing CPU, dispatch, observers, aggregate macros, and loaders
pass in both modes at this runtime-only boundary. New declaration macros are
not exposed yet; their owning steps will activate complete implementations.

One document-hidden __private support module is available through both roots:
actual instruction/surface/dialect associations; nominal declared support;
direct dialect coverage bounds; parent-local usize field indices; and actual child-type
associations with source names. Field getters, mutable traversal, and the unused
path-list type were removed to keep only what the current macros need. These
internal contracts may change with their owning macros. There is no namespace
inference, callback inventory, producer ID, or reflection tree. Manual producers are not
supported merely because cross-crate generated contracts need public visibility.

Borrowed static descriptions retain full dialect/field order and canonical names.
Parsing aliases remain solely in Field::SOURCE. Const validation checks selector
overlap across complete declared dialects and recurses through every child.
Validation compares each dialect pair once and skips instruction comparisons
when their canonical heads differ. Matching heads use a separate, simple
pairwise mnemonic check. Const-compatible standard slice operations replace
the four-counter loop. The check needs no allocation, sorting, or extra metadata;
worst-case comparisons remain quadratic in instruction count, not quartic.
The same instruction at different field destinations and same-head disjoint
dialects remain valid. Source-name syntax, empty declarations, duplicate leaves,
and local alias collisions are owned by the dialect/composite macros in Steps
3/5; their runtime copies and hand-written Unicode byte classification were
removed. Those owning checks use standard string/character methods.

Internal __private::capabilities::<C>() lazily reports canonical target/dialect/instruction
rows in declaration order. It and InstructionCapability are exported only under
__private through runtime/facade; the inventory WIP did not provide a public
reporting API. Aliases do not duplicate rows. Reporting forces
validation for the concrete monomorphization at code generation; unused generic
constants and cargo check alone do not promise universal eager validation.
Owning macros must additionally force fixed declaration/concrete-set validation.

The useful WIP root behavior was ported selectively: typed module paths,
renamed facade/runtime discovery, explicit self/super/crate overrides, invalid
root diagnostics, duplicate-key diagnostics, and exact helper stripping. None
of the WIP's inventory options or protocol machinery was imported.

## Scope boundary

This is declaration feature parity with the proposed inventory-based system.
No topology, profile, backend, driver, wire, continuation, message-builder,
route-work, forwarding, or route/export/source/work attribute API from the
sibling static-topology experiment is introduced. Mutable field access is
intentionally deferred. Future execution features remain additive work. No compatibility layer or alternate legacy macro was added.

## Minimal support audit

Each remaining item has a concrete owning-macro use:

| Item | Why it remains |
| --- | --- |
| InstructionSpec | Dialect-generated surface pairing and nominal membership; selected parsers read MNEMONIC for ordering. |
| DialectSpec | Nominal identity and complete ordered leaf description consumed by component declarations. |
| Supports<D> | Declared membership, distinct from having an arbitrary Execute impl; coherence rejects repeated dialect aliases. |
| Coverage<C>, require_coverage | A generated dialect impl requires Execute<I> for every leaf; the component checks under original bounds. |
| require_available | Selected terminal fields must both declare the instruction's dialect and execute its type. |
| Field<const INDEX: usize> | Parent-local indices associate authored field types and parser source names. Hidden named constants resolve indices through actual parent types and aliases. Ordinary/loadable fields have no mapping or constant. No character or path marker type remains. |
| Leaf | Two named strings define a canonical selector for reporting and cross-dialect overlap checks. No convenience derives are required. |
| Description, Device, Capabilities | Recursive complete descriptions preserve generic child types and declaration order. Device stores only the canonical name and child description. |
| validate | Checks selector overlap unavailable to a proc macro after cross-crate/alias type resolution; concrete association/reporting forces it. |
| equal | Stable str/slice PartialEq is unavailable in const evaluation. The small byte comparison serves only those forced compile-time checks. |

Removed Cons, End, the Instructions associated type, DialectSpec::NAME, Leaf's
unused derives, duplicated device alias arrays, and runtime source-shape/alias
validation. The earlier mutable-access machinery remains removed. Direct
Coverage impls also improve diagnostics by pointing to the generated Execute
bounds rather than recursive list nodes.

A compiler probe of `const fn equal(a: &str, b: &str) -> bool { a == b }` rejects
with E0658 on the local stable Rust 1.96 toolchain: PartialEq is not const-stable.
[The official tracking issue](https://github.com/rust-lang/rust/issues/143874)
records const trait support. No dependency or nightly feature was added to
replace a small private comparison.

## Verification

Local compiler: rustc 1.96.0 (ac68faa20 2026-05-25), Cargo 1.96.0.
Host: macOS ARM64. The changed coverage/key and overlap diagnostics were reviewed and
their snapshots updated; final checks run without overwrite:

| Check | Result |
| --- | --- |
| Workspace format | Pass |
| Workspace clippy, warnings denied, default and typed modes | Pass |
| Workspace all-target tests, default | Pass |
| Workspace all-target tests, typed | Pass |
| Workspace doctests, default and typed | Pass, 31 / 32 doctests |
| SPDX license check | Pass |
| Runtime no-default and derive-only checks | Pass independently |
| Runtime no-default + typed tests/doctests | Pass |
| Derive crate no-default check and typed tests | Pass, 3 tests |
| Facade no-default check | Pass |

Seven typed runtime tests cover actual values, full order, borrowed/associated
child-type projections, repeated instructions at distinct targets, empty nested
descriptions, complete unselected overlap validation, and parent-local named
indices through concrete nested aliases. Disjoint selectors
include case-sensitive and Unicode comparisons; the same mnemonic under a
different head remains valid. Partial overlap late in a dialect is rejected. Their trybuild test checks six
reviewed negative fixtures: missing coverage, insufficient generic bounds,
undeclared dialect despite Execute, duplicate dialect aliases, unknown fields,
and forced overlap. Source spelling/alias diagnostics belong to future owning
macro tests rather than manual runtime description fixtures.

Three derive unit tests and five reviewed real-macro trybuild cases cover root
resolution, unrelated attributes, repeated overrides, invalid roots, and
unsupported options. Existing root override execution tests still pass.

Custom test harnesses that invoke Cargo and create filesystem artifacts have
been removed at user request, together with their fixture workspaces. The
existing observer diagnostic test now uses a static trybuild fixture instead
of creating a temporary Cargo project. Normal integration tests and static
trybuild fixtures retain runtime behavior and compile-fail coverage. Feature
combinations are checked directly by CI/tool commands, rather than by nested
Cargo invocations inside tests. No Rust test directly invokes Cargo or writes
temporary crates. Isolated feature-unification and cargo check/build timing
scenarios are no longer part of the production test suite.

CI now runs default and typed lint/tests/doctests, including both test modes on
Linux/macOS/Windows, and independent feature checks. Those remote jobs have not
run here. Parser crates and their explicit Rust 1.75 requirement were unchanged.
After the minimal support audit, workspace format, default/typed lint, tests,
doctests, license checks, and independent feature checks all passed. Logs are
under /tmp/vihaco-rework2-step2-audit-checks. The later validation-loop refactor
checks are under /tmp/vihaco-rework2-step2-validation-checks. Internal-only reporting export
checks are under /tmp/vihaco-rework2-step2-private-capability-checks. The revised Step 1 probes matched
all 90 compiler outcomes. git diff --check passed.

The later index revision removes Char/Key entirely and changes Field to a usize
const generic. Static fixtures model hidden public per-field index constants;
the bounded instruction-set prototype now reads them through each actual parent
type. Different parents reuse indices and field order may change; consumers
resolve the named constant rather than guess a value. Raw, case-sensitive, and
Unicode names retain their exact unraw spelling. Duplicate-pair coherence uses
const-local tuples of standard unit-array types, without another runtime type.
Generic mappings retain literal indices, so original borrowed/associated types
remain available without unstable generic const expressions. Current instruction
sets are nongeneric and use concrete machines. Real composite/set emission still
belongs to Steps 5–7; no public placeholder macro was added.

The revised probes match all 94 compiler outcomes, including exact index
spellings and rejection of parser aliases as target fields. Cross-crate index
evidence remains in local probes; the removed Cargo fixture harness is excluded
from this implementation commit.
For this revision, workspace format, default/typed clippy with warnings denied,
default/typed all-target tests and doctests, SPDX checking, and runtime
feature-off/derive-only checks passed. Runtime trybuild passed without overwrite
after review of the updated Field<0> diagnostic. Logs are under
/tmp/vihaco-field-index-checks; probe evidence is under
/tmp/vihaco-rework2-step1-target/evidence. git diff --check passed.

## Checkpoint

The user approved the dedicated commit and requested a shortened message.
Commit 072166e retains the title and the first two sentences of its body.
The final default/typed workspace lint, all-target tests, and doctests passed,
along with independent feature checks, format, license, and pre-commit hooks.
Final logs are under /tmp/vihaco-runtime-commit-checks. The runtime feature is
opt-in; its accidental default activation was removed before this verification.
Only local untracked design files remain in the working tree.
The declaration macros, generated grammar, and real macro editor behavior stay
with Steps 3–8. Step 3 has not started and must not start until the user asks.
Design files remain local and excluded from any implementation commit.
