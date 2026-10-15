# Step 7 results: selected parsing and public activation

**Date:** 2026-10-01.
**Status:** approved and committed as `c38b7c8`.
**Proposed commit:** `feat: activate gated typed VM declarations and selected parsing`.

## Behavior

The nondefault `typed-declarations` feature now activates the typed composite
emitter and public `instruction_set` attribute together. Both facade and direct
runtime expose the attribute. Feature-off composite emission remains available;
legacy aggregate/loadable tests run only in their feature-off graph. No renamed
legacy macro, compatibility wrapper, or operand adapter was added.

Each choice projects its actual target fields through explicit device codes.
Declaration checks and parser emission share those projections. The separate
parser emitter reads Field::SOURCE in outer-to-inner order and calls the actual
InstructionSpec::Surface parser. Each canonical/alias alternative consumes its
`::` separator before combining alternatives. The hidden runtime prefix helper
builds small boxed chains rather than macro-generated alias products. Branches
are boxed for heterogeneous composition and combined as an array choice, with a
stable descending associated-mnemonic-length sort. No complete child inventory
or nested surface enum participates.

The paired enum stays in authored scope, is reexported as syntax::AuthoredName,
and implements the existing Parse/Simple<char> contract and SurfaceInstruction.
Its operands and custom dialect patterns retain their existing parser behavior.
Machine/leaf type aliases, concrete generic machines, raw fields, explicit
framework roots, renamed dependencies, and indirect feature activation compile
through actual public macros. Item-level gates cover the generated parser and
marker impls as well as the original associations.

Shared composite loader emission now uses the discovered framework root's
hidden eyre reexport. Consequently, external composite users with only a
facade/runtime dependency do not need a direct eyre dependency merely to compile
generated methods. Existing loader/bytecode consumers pass in the default graph;
no further association-bound changes were needed.

The opt-in facade example declares a complete arithmetic dialect and per-leaf
Execute coverage, nested/aliased composites, and an authored selected ISA. It
parses a typed function, constructs a ParsedModule, lowers it with authored
Resolve, and explicitly executes the resulting runtime variants. The same
instruction at the primary and spare CPUs preserves distinct variant identity
and state. The example runs through an ordinary example test and cargo run.

Rust has a built-in instruction_set attribute. Documentation and actual examples
use qualified #[vihaco::instruction_set(...)] / #[vihaco_runtime::instruction_set(...)]
(or a qualified renamed dependency) to avoid imported-name ambiguity.

## Eligible implementation files

- `README.md`: opt-in usage, example link, feature-unification boundary, and
  stabilization/removal plan.
- `crates/vihaco-runtime-derive/src/attr_instruction_set.rs`: shared target
  association projections and activated expansion entry point.
- `crates/vihaco-runtime-derive/src/attr_instruction_set/emit.rs`: shared
  projections and separate parser emission.
- `crates/vihaco-runtime-derive/src/attr_instruction_set/parser.rs`: selected
  parser and SurfaceInstruction impl emission.
- `crates/vihaco-runtime-derive/src/attr_instruction_set/tests.rs`: finds the
  association structurally rather than assuming its former item index.
- `crates/vihaco-runtime-derive/src/attr_typed_composite.rs`: activated emitter.
- `crates/vihaco-runtime-derive/src/lib.rs`: coherent gated public attributes.
- `crates/vihaco-runtime-derive/src/composite_loader.rs`: rooted eyre references.
- `crates/vihaco-runtime/src/declarations.rs`: hidden boxed field-prefix parser.
- `crates/vihaco-runtime/src/lib.rs`: public reexport, documentation/doctest,
  hidden eyre reexport, feature contract documentation.
- `crates/vihaco-runtime/tests/selected_parsers.rs`: public integration tests for
  aliases, custom patterns, values, identity, error behavior, item gating, and
  30 selected branches; existing trybuild runner for selection diagnostics.
- Ten files under `crates/vihaco-runtime/tests/compile_fail/selection/`: five
  ordinary compile-fail inputs and reviewed stderr snapshots for missing target,
  source alias as target, duplicate type alias, unavailable instruction, and
  loadable traversal boundary.
- `crates/vihaco-runtime/tests/fixtures/dialects/{consumer,facade,indirect}/src/lib.rs`:
  real external public composite/selection expansions, imported instruction and
  machine aliases, raw fields, concrete generic machines, nested aliases, paired
  enum annotations, and consumed-value assertions.
- `crates/vihaco/Cargo.toml`: required-feature opt-in example registration.
- `crates/vihaco/examples/typed_declarations.rs`: complete explicit public example.
- `crates/vihaco/src/lib.rs`: public selection and hidden eyre reexports.
- `crates/vihaco/tests/composite_machine_compile_fail.rs` and
  `crates/vihaco/tests/multisection_bytecode.rs`: preserve legacy declarations in
  feature-off tests rather than compiling them under the replacement contract.

No test writes source/build artifacts, launches Cargo/rustc, or executes generated
fixture binaries. Separate fixture graphs were verified with top-level commands.
Design files and these results are local review artifacts and must never be staged
or committed.

## Verification

Compiler: `rustc 1.96.0 (ac68faa20 2026-05-25)` on local macOS ARM64.

Passed:

- `cargo fmt --all -- --check` and `git diff --check`.
- Default `cargo clippy --workspace --all-targets -- -D warnings`.
- Default `cargo test --workspace --all-targets`.
- Default `cargo test --workspace --doc` (including CPU and guide/example docs).
- `mise exec -- hawkeye check`.
- Feature-on workspace clippy, all-target tests, and doctests using
  `--exclude vihaco-cpu --exclude vihaco-doctests --features vihaco/typed-declarations`.
  Those two crates contain legacy declarations and intentionally stay feature-off.
- Public facade example's test and `cargo run -p vihaco --example typed_declarations
  --features typed-declarations`.
- Independent feature-on fixture graph: `cargo test --manifest-path
  crates/vihaco-runtime/tests/fixtures/dialects/Cargo.toml -p dialect-consumer
  -p dialect-facade -p dialect-indirect`, and corresponding all-target clippy.
- Independent feature-off fixture graph: the same manifest with
  `cargo test -p dialect-feature-off`.
- Five new trybuild snapshots produced with TRYBUILD=overwrite, inspected, then
  verified by the ordinary feature-on workspace test run. Existing snapshots
  remained unchanged.

Focused tests assert canonical and aliased nested prefixes, punctuation/Unicode
aliases, negative and identifier operands, reordered custom-pattern operands,
unit leaves, overlapping v2/v25 mnemonics, omitted leaves/routes, malformed and
trailing input, and the same actual instruction at separate destinations. A
30-branch declaration exercises composition beyond Chumsky's tuple-choice limit.

Actual editor smoke check used the installed VS Code rust-analyzer
`0.3.3065-standalone (03fcb77246 2026-09-27)` against the external fixture workspace
with proc macros enabled. The ordinary imported Add runtime type has correct
hover, definition navigation to producer source, and Add/Halt completion. The
public syntax::Program enum has hover, definition navigation to the authored
Program enum, and Program completion. Hover displays its internal
__ProgramSurface identity, consistent with the reexport implementation.
The preexisting local design/probe script was invoked directly; no editor runner
was added to tests or eligible source.

Logs and editor evidence are under `/tmp/vihaco-step7-*` (default/typed tests,
clippy, doctests, license, selected parser tests, cross-crate/feature-off fixture
checks, example execution, and editor JSON). No remote CI or cross-platform test
run was performed. Parser crates retain their Rust 1.75 metadata and source;
verification used the repository's current stable compiler.

## Checkpoint

Step 7 is complete for review. No file has been staged or committed, and Step 8
has not started. Wait for explicit approval before the dedicated implementation
commit. Exclude every file under design/ from that commit.

## Review follow-up: smaller instruction-set functions

At the user's request, the instruction-set functions are now split by responsibility.
`parse.rs` separates enum validation, surface naming, payload validation, target
parsing, and ordered choice collection from declaration assembly. `emit.rs`
separates collision-free helper names, paired surface declarations, and semantic
checks from final item assembly. `parser.rs` emits individual selected branches
through a separate helper. The generated item order, helper names, spans,
validation order, and diagnostic strings are preserved.

`crates/vihaco-runtime-derive/src/attr_instruction_set/parse.rs` is also an eligible
implementation file for the pending Step 7 commit. No unrelated files were changed
for this refactor. The ten derive unit tests, public selected parser tests and all
five unchanged trybuild snapshots, and complete facade example test pass. Typed
derive all-target clippy with warnings denied, formatting, and whitespace checks
pass. Logs: `/tmp/vihaco-step7-refactor-*`. No commit was made.

## Approved commit

The user explicitly approved committing Step 7. Commit `c38b7c8` is
`feat: activate gated typed VM declarations and selected parsing`. It includes
all 30 reviewed implementation files, the function split, and Choice/Association/Declaration
documentation. Required default and supported typed workspace checks passed again,
as did all four commit hooks. No design artifact was staged or committed.

A separate local change to `crates/vihaco-runtime/Cargo.toml` enabling
typed-declarations by default was excluded from the commit. It was temporarily
set to the reviewed opt-in default for validation and hooks, then restored exactly
after committing. Step 8 has not started.
