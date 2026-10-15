# Step 8 results: CPU integration and diagnostics

**Status:** implemented; awaiting review and explicit commit approval.
**Proposed dedicated commit:** `feat: migrate CPU to gated typed declarations and verify integration`.
No files have been staged or committed. Step 9 has not started. All `design/`
files remain local and excluded from the proposed implementation commit.

## Implementation

- `vihaco-cpu/typed-declarations` is nondefault and forwards the facade feature.
  Legacy and typed declarations/exports/dispatch are selected by mutually
  exclusive local cfg gates. Both coherent graphs retain the same checked CPU
  state, host-VM traits, and `StepOutcome`. Legacy aggregate enums remain only
  in the feature-off API. Typed CPU exposes dialect leaves; consumers own
  selected runtime enums, generated syntax enums, and `HasInstructionSet` associations.
- The actual CPU dialect exports 142 instruction leaves with complete ordinary
  `Execute<I>` coverage through 142 explicit impls, without execution macros.
  Typed messages are separate `None`, `FunctionInfo`, and `Print` structs with
  per-instruction associated types. `FunctionInfo` retains `local_count` for
  checked main frame semantics. `CPUMessage` and aggregate `GeneratedComponent`
  dispatch are available only in the feature-off API; typed consumers dispatch
  leaves explicitly. Wrong messages fail at compile time, covered by doctests.
  Per-instruction execution follows the working-tree
  blocks in `../vihaco-execute-bench/crates/vihaco-cpu/src/component.rs`, at
  `a0a36b13726fa1639c809da863324942ac0452b7` on
  `rob/execute-directly-experiment`, including that worktree's pending CPU edits.
  No sibling files/history were modified. Its extra experimental instructions
  and unrelated APIs were not imported.
- Unsafe benchmark stack/arithmetic operations, changed frame behavior, and
  heap operations were replaced by the existing safe main helpers. A textual
  comparison against main `cf70ca8e2f27a844329cbe24542873bf648204de` confirms
  all shared state/access/execution helpers are unchanged. Both declarations
  retain `pub(crate) stack: Vec<Word>`. CPU Rust source contains no unsafe
  operations, stack wrapper, stack pointer, or unchecked access.
- User-amended patterns annotate bindings with source types: `$0:Ident`
  or `$0: Ident`. The executable struct declares its runtime field type.
  Parser-owned grammar and validation select source parser types and populate
  paired dialect syntax fields. Named/index bindings, capture reordering,
  scoped aliases and Rust type syntax are supported. Ordinary Parse derives
  require the annotated parser result to fit the actual field, with no implicit
  conversion. CPU targets use `:Ident` with runtime `u32`, and constants use
  `:SurfaceValue` with runtime `Word`. `Constant`, `Target`, `operands.rs`, and
  the intermediate operand marker are removed. CPU owns no symbol tables,
  `Resolution`, or instruction resolution methods. Program/composite consumers
  resolve using `ProgramImage.module` tables. Source names for constants use
  module extra metadata, without duplicating stored values. Trybuild covers
  malformed/missing/unknown annotation types, mismatched standalone field types,
  and rejects unresolved syntax instructions and operands in runtime execution.
- Payload codecs preserve four-byte u32 operands, twelve-byte spans, eight-byte
  stack words, and little-endian encoding. The author owns the opcode and fixed
  width through the encoded instruction enum. The existing CPU aggregate enum
  did not assign a wire format; this change does not assign implicit opcodes to
  it. The common legacy/typed section fixture pins the same authored wire bytes
  in both graphs and decodes typed leaves against those bytes. `Label` remains
  source metadata and is resolved to a module label table rather than encoded.
- The shipped-CPU public example parses real selected variants, resolves their
  operands, routes to an aliased nested CPU, executes, checks stack/PC/outcomes,
  and round-trips exact encoded values. Common bytecode/SST multisection tests
  exercise actual CPU execution, shared context handles, observed effects,
  section exclusion, and unexpected-child errors in both graphs.
- Static consumer fixtures cover facade/direct runtime + ABI roots, renamed
  dependencies, and indirect CPU opt-in. A runtime-only opt-in package records
  the unsupported mixed graph. CPU's local feature cannot be enabled in reverse
  by a dependency's feature. The mixed graph was checked separately and failed
  with `expected macro, found attribute macro vihaco::component`, as expected.
- Cross-crate fixtures now assert chained reexports, type/module/use aliases,
  same-named dialects in distinct modules without exporter IDs, lifetime/const
  defaults, original where clauses, associated child types, parsed values,
  complete versus selected order, sparse parent-local codes, and actual stable
  field keys/types under reordering. Existing public-macro suites retain the
  remaining scope/root/visibility/loadable/overlap checks rather than duplicating
  their behavioral suites.
- Semantic `Available<I>` diagnostics name the instruction and target, with the
  irrelevant blanket implementation excluded from compiler recommendations.
  Existing undeclared-dialect snapshots were reviewed, as were new wrong-target
  and ambiguous-spelling snapshots. Existing fixtures cover missing complete
  coverage, complete overlap outside selection, duplicate selected aliases,
  section exclusion, and structural malformed declarations. Notes still expose
  some support names; no promise is made that every generated name disappears.
- CI includes CPU in the feature-on lint/test/doc graph and runs separate fixed
  public feature graphs and the real CPU example. Only legacy guide doctests
  remain in their documented separate feature-off graph.

## Verification

Compiler: `rustc 1.96.0 (ac68faa20 2026-05-25)` on macOS arm64.
The parser crates' explicit Rust 1.75 requirements and editions were unchanged.

All of the following passed:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --doc
mise exec -- hawkeye check
cargo clippy --workspace --exclude vihaco-doctests --all-targets --features vihaco-cpu/typed-declarations -- -D warnings
cargo test --workspace --exclude vihaco-doctests --all-targets --features vihaco-cpu/typed-declarations
cargo test --workspace --exclude vihaco-doctests --doc --features vihaco-cpu/typed-declarations
cargo run -p vihaco-cpu --example typed_cpu --features typed-declarations
```

CPU's own all-target suites ran in both modes, with its common documented public
example compiled as a doctest in both modes and its typed guide doctests in the
feature-on graph. The shipped CPU's reserved-local/operand underflow, invalid
local access, call/return/indirect-call, arithmetic/error, message, and heap
regressions pass through both implementations. License checking reported zero
conflicts; all CPU Rust files' SPDX headers were also inspected directly.
`git diff --check` passed. No nested Cargo/rustc test runners were added.

Separate fixture commands passed offline with the available dependencies:

```sh
cargo test --offline --manifest-path crates/vihaco-runtime/tests/fixtures/dialects/Cargo.toml -p dialect-feature-off
cargo check --offline --manifest-path crates/vihaco-runtime/tests/fixtures/dialects/Cargo.toml -p dialect-consumer -p dialect-facade -p dialect-indirect
cargo test --offline --manifest-path crates/vihaco-runtime/tests/fixtures/dialects/Cargo.toml -p dialect-consumer -p dialect-facade -p dialect-indirect
cargo test --offline --manifest-path crates/vihaco-cpu/tests/fixtures/features/Cargo.toml -p cpu-off
cargo test --offline --manifest-path crates/vihaco-cpu/tests/fixtures/features/Cargo.toml -p cpu-on -p cpu-indirect -p cpu-direct
```

The runtime-only `cpu-mixed` check failed intentionally in its separate graph.
Fixture locks/build artifacts remain ignored and outside the proposed commit.

Concrete generic fixtures pass both `cargo check` and actual compilation/use in
`cargo test`. Declaration-local source/complete-coverage checks reject malformed
components even with no set, including generic missing-coverage/complete-overlap
fixtures. Generated generic obligations retain the original bounds. Concrete
selected associations force description validation; capability reporting also
forces validation for its actual monomorphization at code generation. This is
not a claim of universal eager validation of unused generic descriptions.

The bounded fixture uses the complete 142-leaf shipped CPU, seven target
segments, 48 selected choices, and nested field aliases. It constructs/parses
all 48 selected branches and rejects an omitted leaf; assertions took about
0.01 seconds locally. One incremental compile-and-test measurement was 2.41
seconds overall (1.44 seconds reported compilation). Cross-crate incremental
check took 0.21 seconds before the final fixture additions. These are local
bounded observations, not performance guarantees. No avoidable hotspot was
shown, so no speculative trie/reflection machinery was introduced.

## Actual editor evidence

Version: `rust-analyzer 0.3.3065-standalone (03fcb77246 2026-09-27)`.

Actual LSP initialize/didOpen/didChange/hover/completion/definition requests ran
against the real repository, with CPU's local typed feature and proc-macro
expansion enabled. Completion probes also used transient incomplete buffers,
then restored the exact authored source. No source files were altered by editor
probing. Initial raw responses are in [step-8-editor-results.json](step-8-editor-results.json).
Current typed-binding probes are in [step-8-binding-editor-results.json](step-8-binding-editor-results.json).
They repeat hover/completion/definition with real proc macros enabled after
replacing operand markers with pattern annotations. In a real dialect pattern, `Ident` resolves for hover and definition. The hover
range covers the whole pattern literal, and completion returns no items. An
initial probe of the standalone enum's pattern returned no navigation; that
context does not generate alternate surface fields. Runtime and paired leaf
navigation still works; incomplete paired/target completion remains empty.
Ordinary runtime imports still return 143 items. The intermediate marker probes
are retained as historical evidence in `step-8-mapping-editor-results.json`.

| Authored context | Observed behavior |
| --- | --- |
| Component's `crate::cpu` dialect path | Hover and definition resolve; completion includes the public CPU module. Hover can also show generated dialect metadata. |
| Selected `cpu::AddI64` payload | Hover names the real struct and definition reaches its authored declaration. An incomplete payload path inside the attribute returned no completions. |
| Ordinary `cpu::` import | 143 completions: the 142 runtime leaves and `syntax`. |
| Source type `Ident` in a dialect pattern binding `$0:Ident` | Hover resolves the real Ident struct and definition reaches it. Hover range spans the whole string; completion is empty. Compiler errors identify the pattern string. |
| `cpu::syntax::ConstI64` | Hover/definition resolve, with definition reaching the authored leaf span. Hover displays the generated surface struct name. An incomplete paired import returned no completion items. |
| `#[target(controller::cpu)]` | Definition includes the actual field; hover also includes generated associations and other definitions. Incomplete target completion returned no items. |

These are specific editor limits at public authored paths. Compiler names and
editor hover are not claimed to hide every support type. The checked source
contains no accepted unresolved names or blanket lint allowances added to mask
failures. Authors use actual field names and public leaf/paired paths, without
needing to locate or understand a generated helper tree.

## Eligible implementation files

- `crates/vihaco-runtime/tests/compile_fail/dialect/binding-missing-type.rs`
- `crates/vihaco-runtime/tests/compile_fail/dialect/binding-missing-type.stderr`
- `crates/vihaco-parser-derive/tests/compile_errors/pattern_missing_annotation_type.rs`
- `crates/vihaco-parser-derive/tests/compile_errors/pattern_missing_annotation_type.stderr`
- `crates/vihaco-parser-derive/tests/compile_errors/pattern_wrong_annotation_type.rs`
- `crates/vihaco-parser-derive/tests/compile_errors/pattern_wrong_annotation_type.stderr`
- `crates/vihaco-parser-derive/tests/compile_errors/pattern_unknown_annotation_type.rs`
- `crates/vihaco-parser-derive/tests/compile_errors/pattern_unknown_annotation_type.stderr`
- `docs/src/pages/guide/parser-patterns.md`
- `crates/vihaco-parser-derive/src/binding_type.rs`
- `crates/vihaco-parser-derive/src/codegen.rs`
- `crates/vihaco-parser-derive/src/lib.rs`
- `crates/vihaco-parser-derive/tests/patterns.rs`
- `crates/vihaco-runtime-derive/src/attr_dialect.rs`
- `crates/vihaco-runtime-derive/src/lib.rs`
- `crates/vihaco-runtime/src/lib.rs`
- `crates/vihaco-runtime/tests/dialect.rs`
- `crates/vihaco-runtime/tests/compile_fail/dialect/mapping-unresolved.rs`
- `crates/vihaco-runtime/tests/compile_fail/dialect/mapping-unresolved.stderr`
- `crates/vihaco-runtime/tests/compile_fail/dialect/mapping-unresolved-operand.rs`
- `crates/vihaco-runtime/tests/compile_fail/dialect/mapping-unresolved-operand.stderr`

- `.github/workflows/ci.yml`
- `README.md`
- `crates/vihaco-cpu/Cargo.toml`
- `crates/vihaco-cpu/README.md`
- `crates/vihaco-cpu/examples/typed_cpu.rs`
- `crates/vihaco-cpu/src/component.rs`
- `crates/vihaco-cpu/src/data.rs`
- `crates/vihaco-cpu/src/instruction.rs`
- `crates/vihaco-cpu/src/lib.rs`
- `crates/vihaco-cpu/src/messages.rs`
- `crates/vihaco-cpu/src/typed.rs`
- `crates/vihaco-cpu/src/typed_execute.rs`
- `crates/vihaco-cpu/tests/fixtures/features/.gitignore`
- `crates/vihaco-cpu/tests/fixtures/features/Cargo.toml`
- `crates/vihaco-cpu/tests/fixtures/features/direct/Cargo.toml`
- `crates/vihaco-cpu/tests/fixtures/features/direct/src/lib.rs`
- `crates/vihaco-cpu/tests/fixtures/features/indirect/Cargo.toml`
- `crates/vihaco-cpu/tests/fixtures/features/indirect/src/lib.rs`
- `crates/vihaco-cpu/tests/fixtures/features/mixed/Cargo.toml`
- `crates/vihaco-cpu/tests/fixtures/features/mixed/src/lib.rs`
- `crates/vihaco-cpu/tests/fixtures/features/off/Cargo.toml`
- `crates/vihaco-cpu/tests/fixtures/features/off/src/lib.rs`
- `crates/vihaco-cpu/tests/fixtures/features/on/Cargo.toml`
- `crates/vihaco-cpu/tests/fixtures/features/on/src/lib.rs`
- `crates/vihaco-cpu/tests/call_frames.rs`
- `crates/vihaco-cpu/tests/large_selection.rs`
- `crates/vihaco-cpu/tests/sections.rs`
- `crates/vihaco-cpu/tests/typed_cpu.rs`
- `crates/vihaco-cpu/tests/support/program.rs`
- `crates/vihaco-runtime-derive/src/attr_instruction_set/emit.rs`
- `crates/vihaco-runtime/src/declarations.rs`
- `crates/vihaco-runtime/tests/compile_fail/component/undeclared.stderr`
- `crates/vihaco-runtime/tests/compile_fail/declarations/undeclared-dialect.stderr`
- `crates/vihaco-runtime/tests/compile_fail/selection/ambiguous-spelling.rs`
- `crates/vihaco-runtime/tests/compile_fail/selection/ambiguous-spelling.stderr`
- `crates/vihaco-runtime/tests/compile_fail/selection/unsupported-instruction.stderr`
- `crates/vihaco-runtime/tests/compile_fail/selection/wrong-target.rs`
- `crates/vihaco-runtime/tests/compile_fail/selection/wrong-target.stderr`
- `crates/vihaco-runtime/tests/fixtures/dialects/consumer/src/lib.rs`
- `crates/vihaco-runtime/tests/fixtures/dialects/producer/src/lib.rs`

All other pre-existing user design artifacts remain untouched. Only the local
Step 8 checkpoint/results were updated under `design/rework-2`; those are not
eligible for staging or committing.
