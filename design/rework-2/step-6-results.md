# Step 6 results: authored instruction types and semantic field targets

**Date:** 2026-10-01.
**Status:** approved and committed as `7e33c88`.
**Proposed commit:** `feat: prepare instruction selection by type and field`.

## Behavior

Instruction-set emission is internal and gated by `typed-declarations`. No
public instruction-set attribute or placeholder Parse implementation was added.
Public typed composite/set activation and selected parsing await Step 7.

The declaration stores the concrete machine type, original runtime enum, public
surface identifier, ordered choices, and framework root. Each choice retains
its variant, actual authored payload Type and span, and canonical field target
identifiers. Options and the enum are parsed once. The grammar requires one
`composite = <Rust type>` option, a nonempty nongeneric enum, and exactly one
tuple payload and one parenthesized target helper per variant. Conditional
membership/targets, malformed helpers/options/shapes, empty paths, discriminants,
and exact syntactic duplicates reject during parsing.

The runtime enum is reemitted in its original scope with its actual payloads,
docs, derives, visibility, and ordinary attributes. Owned helpers are consumed.
Targets project the actual parent's unraw field-name constant through
`Field<{ <Parent>::CONSTANT }>::Child` at each segment. No source alias,
external token inventory, inferred module route, Runtime projection, or
string-selected payload reconstruction is used.

The emitted terminal checks require InstructionSpec, declared Supports for the
payload's actual dialect, and Execute for its actual runtime type. Unavailable
fields use missing associated-constant diagnostics at the field lookup; support
and execution use Rust trait diagnostics. Authored spans are retained. Every
concrete association emits full-machine capability validation in an anonymous
nongeneric const, including unselected children.

A const-local coherence trait records parent-local device-code paths as tuples
of standard `[(); CODE]` types and the actual payload Type. Separate hidden const
validation compares canonical dialect/mnemonic metadata at those paths. Aliases
cannot evade duplicate checks; distinct destinations may carry the same payload.
Local helper names avoid identifiers in the authored enum, machine, and root.

The paired surface enum preserves choice order/identity, docs, visibility, and
meaningful documentation/lint attributes, with associated InstructionSpec::Surface
payloads and Clone/Debug/PartialEq. A hidden pub __EnumNameSurface is defined
beside the runtime enum, then reexported as syntax::EnumName. The syntax module
has the authored enum's visibility. HasInstructionSet::Syntax uses that public
path. Payloads and framework roots remain in their original scope. HasInstructionSet
associates the machine with the runtime and surface enums. Compiler coherence rejects another
association. Resolution, execution dispatch, and bytecode policy stay explicit.

## Structure and eligible files

Parsing and emission are separate private modules at the user's request:

- `crates/vihaco-runtime-derive/src/attr_instruction_set.rs`: shared declaration
  model and small expand entry point.
- `crates/vihaco-runtime-derive/src/attr_instruction_set/parse.rs`: typed options,
  declaration parsing, helper consumption, and structural validation.
- `crates/vihaco-runtime-derive/src/attr_instruction_set/emit.rs`: field projections,
  semantic checks, paired enums, and machine association.
- `crates/vihaco-runtime-derive/src/attr_instruction_set/tests.rs`: in-process
  structural tests and AST output-shape assertions.
- `crates/vihaco-runtime-derive/src/lib.rs` and `Cargo.toml`: gated internal
  module and dev-only syn extra-traits for AST comparisons. Unused visit-mut
  support was removed with the compilation harnesses.
- `crates/vihaco-runtime/src/declarations.rs`: hidden canonical Selection data,
  const validation, required device metadata, and device-code Field contract.
- `crates/vihaco-runtime/tests/typed_declarations.rs`: explicit sparse-code
  relationships, nested alias projections, and parent-local code reuse.
- `crates/vihaco-runtime-derive/src/attr_typed_composite.rs`: removes the custom
  Cargo/rustc test runner and its seven unused static fixtures.
- `crates/vihaco-runtime/tests/dialect_feature_graphs.rs`: removed nested Cargo
  runner. Its checked-in fixture crates remain available for top-level checks.
- `AGENTS.md`: records the future test implementation policy.

The seventeen instruction-set compilation fixtures were removed with their
runner. Eighteen eligible implementation files were committed with explicit
user approval. All design artifacts remain excluded from every implementation
commit and PR.

## Current verification and coverage

Compiler: `rustc 1.96.0 (ac68faa20 2026-05-25)`, local macOS ARM64.

The retained instruction-set tests run entirely in process. They reject malformed
options/helpers/shapes, missing/repeated/conditional targets, conditional
membership, syntactic duplicates, generic sets, discriminants, and duplicate
roots. AST assertions check actual retained payload Types, associated surface
projections, docs/visibility/attribute retention, choice order, raw target
spelling, and the public surface naming rule. The retained composite tests check
structural validation and fixed-versus-generic dependency classification.
Existing public macro integration tests, doctests, and established trybuild
fixtures remain. None of the project-authored tests now launches Cargo/rustc or
executes generated binaries.

Before the runner removal, default and supported typed workspace clippy,
all-target tests, doctests, formatting, SPDX checks, and whitespace checks
passed. Earlier custom cross-crate metadata/codegen checks also passed, but
those runners and fixtures have been removed at the user's request and do not
count as retained automated coverage. Post-removal default and supported typed workspace all-target tests and clippy,
formatting, SPDX checks, and whitespace checks passed. Logs:
`/tmp/vihaco-step6-no-runners-*.log`.

Compiler-semantic coverage for the internal set/composite emitters is deferred
to ordinary public-macro integration tests and established trybuild fixtures
when Step 7 activates the complete contract. Actual IDE evidence remains
Steps 7–8. Separate feature graphs must run through top-level verification
commands or CI jobs rather than a nested test runner.

## Review follow-up: test implementation policy

At the user's explicit request, the instruction-set and composite Cargo/rustc
compilation harnesses and the nested dialect feature-graph Cargo test were
removed. Unused compiler fixtures were deleted. The remaining plans, overview,
PR plan, and repository guidance prohibit custom test harnesses that write
generated source/build artifacts, launch Cargo/rustc (including stdin
compilation), or execute generated fixture binaries. Do not reintroduce this
pattern in future steps.

## Limits and checkpoint

Public attributes and selected parsing await Step 7. The established typed graph
still excludes legacy CPU and guide/example callers until migration. Parser
crates and their explicit Rust 1.75 requirement are unchanged. Only local
macOS/Rust 1.96 verification ran; remote cross-platform CI has not run.

The user explicitly approved the dedicated implementation commit. Step 6 is
committed as 7e33c88. Step 7 has not started. Never stage or commit anything
under `design/`.

## Review follow-up: syntax namespace

At the user's request, the paired enum now uses the dialect-style syntax
reexport convention: runtime MachineInstruction pairs with
syntax::MachineInstruction. No public SurfaceInstruction suffix is generated.
The actual hidden __MachineInstructionSurface definition remains in authored
scope, preserving relative payload/root paths. The syntax module preserves the
runtime enum's visibility. Tests inspect the hidden definition, reexport,
HasInstructionSet association, raw identifiers, and public/private visibility
in process. An enum named syntax is rejected locally. Multiple sets need
separate authored modules; function-local sets cannot use super reexports from
a generated Rust module. The active contract and Steps 6–9 record this naming
and scope amendment.

After the naming amendment, all eight gated derive unit tests and typed
workspace clippy passed, along with formatting, SPDX headers, and whitespace
checks. Logs: `/tmp/vihaco-step6-syntax-*.log`. No custom compiler runner was
reintroduced and no commit was made.

## Review follow-up: required device codes as Field keys

At the user's request, every typed device declaration now requires an explicit
u8 code, including loadable sections. Field impls, hidden field-name usize
constants, and generic description projections use that code directly as the
key. No declaration-order key is allocated. Codes remain unique within each
parent, including loadable devices; section devices retain code metadata but
have no Field relation. CompositeDevice::code is now u8 rather than Option<u8>.
Capability metadata/descriptions preserve declaration order independently of
numeric key order. Instruction-set target spelling remains the field name and
its lookups/coherence paths now resolve to device codes.

In-process tests assert sparse codes, hexadecimal spelling, 0/255, unchanged
keys after reordering, ordered metadata/child projections, and exclusion of
state/loadable fields from Field relations/constants. They reject missing codes
on eligible/loadable devices and duplicate codes across section boundaries.
Existing generic runtime relationship tests use explicit parent-local codes.
No custom compilation runner was added. The active contract, overview, and
Steps 5–9 reflect the change; earlier optional/index-order results are historical.

After the required-code amendment, default and supported typed workspace
all-target tests, doctests, and clippy passed. Focused composite/runtime tests,
formatting, SPDX headers, and whitespace checks also passed. Existing trybuild
snapshots were unchanged. Logs: `/tmp/vihaco-step6-device-codes-*.log`.
The typed composite emitter remains internal until Step 7. No commit was made.

## Approved commit

The user explicitly approved committing Step 6. Commit `7e33c88` is
`feat: prepare instruction selection by type and field` on
`rob/instruction-sets-and-parsing`. It includes the reviewed parser/emitter
split, syntax-module naming, required device-code Field keys, and test-harness
removal. All configured hooks passed: formatting, default/typed workspace
clippy, and SPDX license headers. All eighteen implementation files are outside
design; no design artifact was staged or committed. Step 7 has not started.
