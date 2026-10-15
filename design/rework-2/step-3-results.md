# Step 3: dialect declaration results

**Recorded:** 2026-10-01.
**Status:** reviewed and committed as c3945f7.
**Branch:** rob/types-and-dialect-declarations, following Step 2 commit 072166e.
**Commit:** c3945f7, `feat: generate typed dialect instruction associations`.
All required gates and commit hooks passed. Nothing under `design/` was staged
or committed. No branch change, push, or PR has been made. Step 4 has not started.

## Implemented behavior

The gated `#[dialect]` attribute preserves an authored inline module, its
imports and unmarked items, and immediate unrestricted-public unit/tuple
instruction structs. Runtime leaves retain authored operand types, field
visibility, docs, and attributes. Missing Clone/Debug/PartialEq derives are
supplied without repeating authored ones. Hidden public `__NameSurface` structs and
parsers live beside the runtime leaves and use the same authored operand types.
A public `syntax` module reexports them as `syntax::Name`, including constructors,
with inline public documentation. This naming amendment was approved during
Step 3 review. The generated namespace and internal names are collision-checked;
authored `NameSurface` items are now permitted.
No operand mapping, module bridge, opcode policy, aggregate enum, conversions,
dispatch, exported inventory macro, or companion tree was introduced.

Each runtime leaf implements InstructionSpec once, associating its surface
parser, validated mnemonic, and declaration-local nominal `__VihacoDialect`.
That identity describes every immediate leaf in declaration order and implements
Coverage<C> with one Execute<Leaf> bound per leaf. The coverage parameter is
chosen to avoid shadowing authored leaf types, including `C` and the default
internal parameter name. Module import aliases do not change source identity.
Raw module names use unraw spelling; raw leaf names reject as agreed in Step 1.

An internal function-like proc-macro emitter in parser-derive reuses the actual
Parse implementation with framework-rooted paths. It validates all leaf patterns
before duplicate pattern/mnemonic checks, and emits associations/descriptions
only after validation, avoiding follow-on missing-trait errors. Independent
Parse derives keep their existing external parser/Chumsky paths. No public
parser-root attribute was added. The runtime reexports Parse under the typed
feature and enables parser-derive as an optional dependency. Parser source uses
Rust 1.75-compatible syntax. Zero-field tuple parsing now constructs `Type()`;
unit parsing still constructs `Type`.

The existing framework-root helper names a path in the annotated module's
parent scope. Emission adjusts only framework-relative roots for the extra
module level; operand paths are untouched. Crate discovery supports renamed
facade/runtime dependencies. Whole-module cfg/cfg_attr conditions retain their
ordinary behavior. Conditional leaf membership remains unsupported.

## Eligible implementation files

- `Cargo.lock`: the runtime's optional parser-derive relationship.
- `crates/vihaco-runtime-derive/Cargo.toml`, `src/lib.rs`, and new
  `src/attr_dialect.rs`: gated declaration parsing, validation, and emission.
- `crates/vihaco-parser-derive/src/{lib,codegen}.rs`: rooted internal emitter,
  shared pattern validation/mnemonic extraction, empty tuple construction.
- `crates/vihaco-runtime/Cargo.toml`, `src/{lib,declarations}.rs`: feature
  wiring, hidden emitter reexport, public attribute and Parse reexports,
  runtime documentation/example.
- `crates/vihaco/src/lib.rs`: facade attribute reexport and runnable example.
- `crates/vihaco-runtime/tests/dialect.rs`: six behavioral tests, including
  the trybuild runner, pairing, scope, ordered descriptions, and coverage.
- `crates/vihaco-runtime/tests/compile_fail/dialect/`: 27 static fixtures and
  reviewed diagnostic snapshots.
- `crates/vihaco-runtime/tests/dialect_feature_graphs.rs` and
  `tests/fixtures/dialects/`: four isolated Cargo graphs using static fixture
  packages for feature-off, renamed direct runtime, renamed facade, indirect
  feature activation, and producer/consumer alias identity. Fixture lock/build
  artifacts are ignored. The runner is included in ordinary workspace tests.

## Validation

Compiler: `rustc 1.96.0 (ac68faa20 2026-05-25)`.

Passed:

- `cargo fmt --all -- --check` (fixture workspace formatted separately).
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo clippy --workspace --all-targets --features vihaco/typed-declarations -- -D warnings`.
- `cargo test --workspace --all-targets`.
- `cargo test --workspace --all-targets --features vihaco/typed-declarations`.
- `cargo test --workspace --doc`.
- `cargo test --workspace --doc --features vihaco/typed-declarations`.
- `cargo check -p vihaco-runtime --no-default-features --features typed-declarations`.
- `mise exec -- hawkeye check`.
- `git diff --check`.

Trybuild snapshots were generated, individually inspected, and then verified
without overwrite. They pin declaration shape/visibility, marker placement,
conditional membership, collisions, raw leaves, malformed patterns before and
after duplicates, pattern binding errors, and incomplete execution coverage.
The compiler additionally reports its existing unstable-file-module error for
the out-of-line fixture, alongside the dialect's spanned inline-module diagnostic.

Behavioral tests capture actual parsed values (including reordered operands),
paired associated types, complete ordered descriptions, private operand fields,
relative/absolute/qself/generic operand paths, local parser-like operand names,
raw module identity, nested declaration scopes, same-named dialects, runtime instruction operands retaining their identity,
syntax reexports and constructors, authored old surface names, function-local
modules, existing derives, unmarked parser helpers, and whole-module gating.
Cross-crate fixtures depend only on renamed framework crates; no direct parser
or Chumsky dependency is needed for dialect parsing. Rustdoc examples hide an
explicit existing root override because rustdoc's test crate differs from the
package library that proc-macro-crate reports as `Itself`.

## Checkpoint and limits

This completes Step 3's declaration/pairing/description/coverage exit criterion.
The initial rejected shapes and conditional membership policy are unchanged.
All checks ran locally on macOS; Linux/Windows and parser MSRV execution were
not separately run. Source syntax in parser crates remains within their declared
MSRV. Existing standalone Parse derive root behavior is preserved.

User approved the dedicated implementation commit; it is recorded above.
Every `design/` artifact was excluded. Step 4 remains unstarted.

## Later test-policy correction

On 2026-10-01 the user requested removal of custom Cargo/rustc test runners.
The nested `dialect_feature_graphs.rs` Cargo runner is removed in the pending
Step 6 changes. Its checked-in fixture crates remain available for top-level
verification commands or CI jobs. Earlier nested-runner results above are
historical evidence. Future tests must not write generated source/build
artifacts or launch custom compilation subprocesses.
