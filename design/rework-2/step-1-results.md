# Step 1 results and proposed decisions

**Recorded:** 2026-10-01.
**Status:** approved no-commit checkpoint; user authorized Step 2 on 2026-10-01.
**Scope:** local design, static fixtures, and isolated mechanism models only.
No production source, branch, commit, push, or PR was changed by this work.
Nothing under `design/` may be staged or committed.

## Checkout and baseline

The user reset `rob/types-and-dialect-declarations` to `main` while this audit
was running. The final implementation baseline is
`cf70ca8e2f27a844329cbe24542873bf648204de` on that branch. Its tracked worktree
and index are clean. The existing untracked files are the local design records.

The prior work remains on `rob/shared-inventories-and-dialect`:
`7d36633f3048789a54c00247d6cadc5b171a49c7` (WIP), following approved preparation
`fbfec16`. Neither commit is an ancestor of the new implementation branch.
They were inspected read-only with `git show`/`git diff`; do not cherry-pick
them wholesale or count them as replacement-plan implementation commits.
The separate historical behavioral reference remains `rob/inventory-test` at
`2b87ad69e0a312c62c2157b6317b48b386215cdd`.

Compiler: `rustc 1.96.0 (ac68faa20 2026-05-25)`.
Cargo: `cargo 1.96.0 (30a34c682 2026-05-25)`.
Platform: local macOS, ARM64. Parser crates retain their explicit Rust 1.75
requirement; no 1.75 toolchain or other operating system was tested here.

After the reset, all five baseline gates passed:

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --offline --workspace --all-targets -- -D warnings` | Pass |
| `cargo test --offline --workspace --all-targets` | Pass, 186 test results reported |
| `cargo test --offline --workspace --doc` | Pass, 31 doctests |
| `mise exec -- hawkeye check` | Pass, 85 files, no conflicts |

Independent checks passed for runtime without default features, runtime with
only `derive`, ABI without default features, and the facade. Separate fixture
packages also check actual renamed facade/runtime dependencies and explicit
`self`, `super`, and `crate` framework-root overrides. Their tests exercise
the current default composite emitter directly, including its aggregate parser
and field source alias. No compatibility macro alias is exported.
The current composite fixture supplies `eyre` directly, as its emitter requires;
this is separate from the new parser-root fix.

Baseline logs are in `/tmp/vihaco-rework2-step1-main-baseline/`. Earlier logs
in `/tmp/vihaco-rework2-step1-baseline/` describe the WIP checkout before the
user reset and are not the final baseline.

## Classification of the prior work

This classification precedes any production port. No old patch has been applied.

| Prior change | Decision and owner |
| --- | --- |
| Typed framework-root options, dependency rename discovery, duplicate/invalid root diagnostics in `common.rs` | Port the root-only behavior selectively in Step 2. Root discovery already exists on `main`; strengthen it without producer IDs. |
| Attribute stripping and the composite wrapper's use of it | Retain the behavior in owning macro work. Preserve unrelated attributes; strip each consumed helper once. |
| Renamed-root fixture packages, root diagnostic tests | Reuse their behavioral cases in Steps 2–3, not their dependency on the old inventory model. Local root fixtures verify the useful cases now. |
| Surface enum naming helper | Retain the public naming rule directly in Step 6. It does not require the path-inference module. |
| `inventory_id`, producer options, backing exporters, provider segments | Omit from the new branch. No user producer IDs or exported transport macros. |
| `infer_namespace`, inventory namespaces, selector/type-tree paths, `NamespaceBridge`, module-depth rebasing | Omit. Real payloads and fields stay in authored scope; surface structs can stay there too. |
| Collected inventory records, token protocol, callback finalizers | Omit. They are not runtime relationships or immutable descriptions. |
| Relative-path/source-spelling structural validation | Port only the checks needed by field targets and metadata in Steps 2/5/6. Do not port wrapper types just to reuse tests. |
| Broad dead-code and unused-export allowances; tree/path/exporter fixtures | Omit with the unused machinery. Keep behavioral coverage in typed tests. |

Because the new branch starts at `main`, Step 9 audits for accidental imports
of obsolete machinery rather than deleting preparation that is not present.
The approved and WIP commits remain intact on the old branch.

## Proposed grammar

The original surface naming decision below was amended during Step 3 review:
surface definitions remain in the authored scope as hidden `__NameSurface`
types, publicly reexported as `dialect::syntax::Name`. See the current contract.


| Declaration | Decision |
| --- | --- |
| Dialect | Bare `#[dialect]` on a nonempty inline module; immediate bare `#[instruction]` unrestricted pub unit/tuple structs are leaves. Private/restricted marked structs reject. Module and field visibility retain ordinary Rust meaning. |
| Operand | One authored Rust field type is used in runtime and surface structs. No operand attribute or remapping option is supported; no special rejection logic is needed. Custom Parse and application Resolve use ordinary Rust. |
| Leaf pattern | Existing `#[pattern = "..."]` on the instruction struct; consumed by dialect emission and forwarded to the real surface parser derive. |
| Surface leaf | `InstructionNameSurface` in the same authored module, with unrestricted pub visibility and authored field visibility. No generated `syntax` module or operand bridge. Reject collisions with authored items. |
| Component | Bare `#[component]`, exactly one nonempty `#[instructions { dialect_paths, ... }]`; optional trailing comma, whole dialects only. |
| Composite | Bare `#[composite]`, immediate named `#[device]` fields. Retain optional explicit device codes and repeated `alias = "..."` values; aliases do not name targets. `#[loadable]`/`#[loadable("section")]` preserves a section boundary. |
| Set | `#[instruction_set(composite = Type)]`, optional trailing comma; a concrete Rust type including aliases and qualified associated types. One actual type payload and one `#[target(field::field)]` per variant. Initial enum is nonempty and nongeneric. |
| Feature | Nondefault `typed-declarations`, coherently forwarded through facade/runtime/derive. Default API stays current; opt-in selects the experimental contract. No compatibility layer. |
| Framework root | Preserve the existing item helper `#[vihaco(crate = module_path)]`; reject repeats and unsupported options. No inventory/namespace override. |

The existing set surface naming rule remains: `XInstruction` produces
`XSurfaceInstruction`; otherwise append `SurfaceInstruction`. There is no
public surface-name override in this initial grammar. The root helper chooses
the library dependency, not where an author's types live.

Initially reject named-field/generic instructions, out-of-line dialects,
repeated/misplaced markers, conditional membership, and raw instruction names
with a spanned diagnostic. Raw dialect and field names use their unraw identity.
The current Parse derive rejects inferred raw names and panics for an explicit
raw-name pattern; that panic is baseline evidence, never a desired diagnostic.

Whole-item `cfg`/`cfg_attr` gates the expansion. Function-local components and
sets are supported where ordinary Rust items and trait impls are legal. An
inline dialect is an ordinary module: it does not acquire access to enclosing
function locals or generic parameters that Rust modules cannot capture.

## Mechanism results

Run [the static runner](probes/run.py) as described in [its README](probes/README.md).
The revised run passed six consumer value/type tests and matched 72 compiler
outcomes: 36 static cases, each compiled in metadata and link modes. Six
additional Cargo check/build outcomes measure generic const timing, and twelve
check/build outcomes cover the local feature gate. All 90 outcomes matched.
Separate renamed-root packages each pass their current default composite test.
Fixtures remain under `design/`; artifacts/logs live under `/tmp/`.

| Concern | Evidence | Result / remaining owner |
| --- | --- | --- |
| Private module with pub leaves | `user-dialect-shape`, bounded real attribute expansion | Pass. Module visibility stays authored; instructions are explicitly pub. Full production emitter: Step 3. |
| Required public instructions | Private, pub(crate), pub(super), pub(in crate), and mixed-visibility fixtures | All fail with the visibility diagnostic. Unrestricted pub remains the authored API rule. |
| Original module operand scope | `Scoped`, parent-typed `A`, local `syntax` type, module imports, `self`/`super`, generic/qself operand types | Pass; surface structs beside runtime structs need no path rewriting or module bridge. |
| External type associations | Producer → reexport crate → renamed consumer; real payload and surface assertions | Pass. Actual facade/runtime attributes remain owned by Steps 3–7. |
| Target traversal | Nested `Field<const INDEX: usize>` projections and named index constants, associated child types, lifetime/const/default generic declarations, raw field fixture | Pass after the index revision. Type aliases and qself machine/payload types resolve through Rust. Generic relations use literal indices; target lookups use concrete types. |
| Complete coverage | Missing Execute, new uncovered leaf, generic implementation only for `T: Copy` | Expected failures at the unchecked original declaration bounds. Unused helper functions are still type checked. |
| Declared support | An extra Execute impl for an undeclared dialect | Expected failure for missing Supports; Execute alone does not grant availability. |
| Identity | Repeated dialect aliases, duplicate selected pair via payload alias, two set associations | Expected coherence failures. No TypeId, hashing, or token identity proof. Same instruction at two different targets passes. |
| Descriptions | Ordered complete leaves/fields/aliases; same-head disjoint dialects | Pass. Omitted executable leaves remain in capabilities. Cross-dialect overlaps fail forced const evaluation. Local alias collisions belong to the composite macro. |
| Whole-machine forcing | Overlap in an unselected dialect leaf at concrete set association | Expected E0080 in both modes, even when the selected leaf is otherwise valid. |
| Generic validation | Unused invalid constant; forced concrete constant; generic reporting function | Unused constant passes both modes; concrete const fails both; reporting function passes metadata and fails link/codegen. Do not promise eager checking of every generic instantiation. |
| Selected parsing | Associated surface parsers, 15 nested alias combinations, identifier/negative operands, omitted branches, trailing/malformed input | Pass, actual parsed values checked. Parser emission from the production set macro: Step 7. |
| More than 26 choices | 30 associated leaf parsers with `v1`/`v10` prefix overlaps, authored in ascending order | Pass using boxed alternatives and stable longest-mnemonic-first parser ordering. Declaration/variant/description order stays authored. |
| Renamed parser dependencies | Current Parse derive with only the reexporting framework available | Expected failure: it hardcodes `::vihaco_parser` and `::chumsky`. A framework-rooted manual Parse expansion compiles without those direct extern names. Production root support: Step 3. |
| Experimental feature | Isolated support/macro forwarding; explicit and indirect opt-in; check and build | Default support remains usable; new support/macros are absent when off, incomplete forwarding fails, and coherent explicit/indirect activation succeeds. Production facade/runtime wiring: Step 2. |
| Whole-item gating and collisions | Disabled dialect, separate same-named modules, surface-name collision | Pass / expected collision failure. Exhaustive helper/attribute diagnostics: Steps 3–8. |

The probe attributes are intentionally incomplete. They are not exported from
the real facade/runtime, do not implement all malformed-option checks, do not
provide a production selected parser, and do not establish full feature/MSRV
or migration correctness. Those checks remain assigned to their owning steps.

## Architecture decisions supported by the probes

Require unrestricted pub on every marked instruction as the authored API rule.
Generate Coverage<C> directly on the dialect identity with all Execute<I>
bounds; a type-level list and recursive list traversal are unnecessary. The
module itself may remain private. Public static descriptions contain canonical
names, not type paths. Source-name syntax and local alias collisions belong to
the owning macros and can use standard string/character methods.

Keep `Field<const INDEX: usize>` limited to actual child-type associations and source names.
Nested Child projections preserve borrowed and associated types. Mutable access
was explored initially, then removed because no current macro requires it.
Generated internal contracts may change when a future feature needs access;
manual user implementations are unsupported.

Expose each instruction's validated source mnemonic through its hidden typed
association. Build only selected branches, then stably prioritize longer
mnemonics before combining boxed parsers. The current real parser derive also
prioritizes longer mnemonics. Simply combining unit parsers in authored order
can accept a shorter prefix and fail later, outside the alternative. Field
alternatives include `::` before `.or`, preventing the analogous field-prefix
problem. This does not require full-child enumeration or alias combinations.

Step 3 must root generated Parse/Chumsky references through the existing
framework-root mechanism or an internal parser-emission hook. Do not add a
public parser-root attribute. Preserve independent parser callers and their
current default paths. Reexport the Parse derive from runtime only where the
experimental macro stack requires it; runtime currently reexports the trait.
Keep parser changes within Rust 1.75 syntax. A manually rooted expansion is
proved; integration with the real dialect/parser emitter remains Step 3 work.

Validate fixed component dialect descriptions with a declaration-level const,
independent of component state generics. A concrete authored set forces the
entire concrete machine description in a non-generic const. The supported
generic reporting entry point uses an inline const and validates at codegen;
its runtime data cannot be returned by a successful executable if invalid.
Raw descriptions and reporting both remain generated support under __private.

## Caller and ABI migration boundary

All new features are behind nondefault `typed-declarations`. Opt-in selects
one coherent experimental contract through facade/runtime/derive; Cargo
unification may activate it through another dependency. Feature-off builds
keep the current contract. Do not add legacy exports, compatibility wrappers,
or adapters. Stabilization is followed by consumer migration and removal of
the old implementation and feature gate in a deliberate breaking release.

| Caller family | Experimental boundary and stabilization work |
| --- | --- |
| Component/Composite markers and shared generic consumers | Feature-on markers omit HasInstructionSet supertraits; feature-off markers stay current. Shared consumers needing associations use explicit bounds. Verify isolated modes and indirect feature activation in Step 2. |
| Legacy component!, dispatch, GeneratedComponent, observers | Keep current default behavior. New Execute is gated and does not adapt old execution. Audit consumer conversion before stabilization removal. |
| CPU encoded enums, frame/bytecode tests | Keep current encoding, Resolve, bytes, and execution in feature-off builds. Do not substitute unencoded structs during the experiment. Record explicit migration requirements in Step 9. |
| Current aggregate composite examples and Raw devices | Keep feature-off #[composite] unchanged. At Step 7 feature-on #[composite] uses the completed typed emitter together with authored sets/parsers. Test separate builds; no renamed old macro. |
| Multisection loaders and bytecode/SST contracts | Preserve current default bytes, codes, source aliases, sections, loading order, and execution. New loadable-boundary fixtures opt in. Migrate current consumers before old implementation removal. |
| Guides and vihaco-doctests | Preserve current examples in feature-off suites and add complete opt-in examples. Explain experimental activation and eventual removal without a compatibility API. |

Steps 5–6 keep new composite/set emission internal until complete selected
parsing is ready in Step 7. Default and opt-in callers use separate fixtures;
there is no partially working public composite and no permanent dual API.

Add Execute with the reference signature: borrowed `&I`, associated Message,
Effect, and Error, and `Result<Self::Effect, Self::Error>`. Users may choose
`Effects<E>` and `eyre::Report`; do not implicitly wrap every effect. Coverage
does not invent a common message/effect/error type across unrelated Execute
impls. Future execution features can add their own constraints.

## Editor evidence and limits

Measured with `rust-analyzer 0.3.3065-standalone (03fcb77246 2026-09-27)` using
the LSP driver on the isolated workspace with proc-macro expansion enabled.

| Probe | Hover | Definition | Completion |
| --- | --- | --- | --- |
| Real Halt payload in the prototype set | Actual `proof_producer::control::Halt` | Authored producer struct | No results at this payload position |
| Chained renamed Add path | Actual `Add(pub i64)` and authored docs | Authored producer struct | Add and other real module items, including generated surface types |
| Associated `InstructionSpec::Surface` projection | Trait-associated type | Associated type declaration, not concrete AddSurface | Surface/Dialect associated items |

The JSON log is `/tmp/vihaco-rework2-step1-editor.json`. Navigation to runtime
payloads works in the prototype; projection navigation stops at the trait.
These are measured limits, not promises about the final proc macros. Actual
component paths, selected field navigation, production macro spans, and the
complete generic/error matrix must be repeated in Steps 7–8.

## No-commit checkpoint

All Step 1 mechanisms have local evidence or an explicit production owner.
The production-dependent root fix, exhaustive macro diagnostics, editor
coverage, production feature configurations, parser MSRV, and experimental activation
remain work for Steps 2–9. No unresolved mechanism requires namespace inference,
exported token inventories, public reflection trees, or a generic `'static`
restriction.

Review the updated pub-only grammar, surface naming, validation timing, and
feature/stabilization boundary before
Step 2. The user approved moving to Step 2; no implementation commit is requested
for Step 1. See the [first file's checkpoint](01-contracts-and-baseline.md#end-of-file-checkpoint).

## Later set-naming amendment

During Step 6 review the user replaced the historical set suffix naming rule
with the same syntax reexport convention used by dialects: syntax::EnumName,
backed by a hidden pub __EnumNameSurface defined in authored scope. The syntax
module preserves the runtime enum's visibility. Module scope is required for
set reexports; separate sets need separate authored modules. The current
contract and Steps 6–9 supersede earlier suffix/function-local set decisions.

## Later device-code amendment

During Step 6 review the user required explicit u8 codes on every device,
including loadable sections, and chose those codes as the parent-local Field
keys. Hidden field-name constants contain the same codes; declaration order
only controls metadata/capability ordering. The active contract and Steps 5–9
supersede earlier optional-code and declaration-order-index decisions.
