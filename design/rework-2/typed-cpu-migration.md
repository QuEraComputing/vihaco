# Typed CPU dialect and direct runtime parsing

Status: initial direct-SST parsing and CPU execution slice implemented, pending
user review. This document records the agreed direction
and the implementation boundary.

## Repository baseline

The discussion began on `rob/update-cpu-for-typed-declarations`, at
`9a09d3ca` (`feat: generate flat composite instruction sets with compile-time
Step coverage`). The declaration framework supports typed dialects, whole-dialect
components, field markers, composite-generated flat instruction enums, and
compile-time Step coverage.

The CPU still uses the legacy `component!` declaration and aggregate dispatch.
It has no local `typed-declarations` feature. Existing rework-2 overview and
Step 8 results describe a CPU migration that is absent from this checkout;
those descriptions are historical evidence, not the implementation baseline.

The composite-generated instruction plan supersedes the earlier authored
`#[instruction_set]` selection model. Current `Step` and `Dispatch` return
`()`. Owned executions, routes, and drivers remain separate proposals.

## Instruction pipeline

The intended pipeline is:

```text
SST source
  → runtime instruction enum
  → Dispatch
  → Step
  → Execute
```

Phase out syntax resolution. Parse instruction syntax directly into executable
runtime instructions, without an intermediate surface instruction enum or a
separate instruction-resolution pass.

Typed dialect leaves receive parsers for their actual runtime structs.
Composite parsing produces the generated flat runtime enum whose payloads are
those leaves. Paired surface structs and enums are unnecessary in this model.

Local literal conversion is part of parsing. For example, a parser may read an
`i64` literal, encode its bits into `Word`, and construct a runtime constant
instruction. That does not require symbol lookup or an unresolved instruction
representation.

## Resolved SST operands

The SST producer supplies resolved operands. For example, if a label's target
address is 10, the producer emits `br 10` rather than `br @later`.

Branch and call targets are concrete addresses. String, function, and heap
references are concrete runtime IDs or encoded reference values. The CPU
instruction parser does not resolve names, intern strings, or build symbol
tables. Module construction/loading still owns function tables, strings,
sections, and other program metadata required at execution time.

The producer calculates addresses against the executable instruction sequence,
excluding readability metadata. The exact address convention should match the
existing program-counter and section behavior; this migration does not invent
a different address unit.

## Metadata instructions

- `label` remains available in SST for readability and produces no runtime
  instruction.
- `function_start` and `function_end` disappear.

Metadata must be consumed outside the runtime instruction enum. Parsing a
label must not introduce an executable slot or require an execution handler.

## CPU declaration and feature boundary

Add a nondefault `vihaco-cpu/typed-declarations` feature forwarding to
`vihaco/typed-declarations`. Select legacy and typed declarations, exports,
execution implementations, and mode-specific tests with mutually exclusive
local feature gates.

The typed CPU declares public instruction structs in complete dialects and
implements `Execute<I>` for every executable leaf. Composite-generated
instruction sets retain complete eligible-device enumeration and full Step
coverage. Metadata removed from runtime representation is excluded from that
coverage.

Preserve feature-off behavior while the experimental gate exists. Feature-on
graphs containing CPU must enable the CPU's local feature directly or through
another dependency; enabling only a framework dependency's feature does not
enable CPU's local cfg in reverse.

## Per-instruction messages

Replace the typed path's `CPUMessage` enum with per-instruction associated
message types.

- Split `CPUMessage::FunctionInfo` into a struct preserving `arity`,
  `start_address`, and `local_count`.
- Split `CPUMessage::Print` into a struct preserving its text payload.
- Instructions requiring no runtime input use `()`.

Preserve the existing message construction behavior and the instruction-specific
use of its fields. Message construction remains the composite's responsibility.
Removing syntax resolution does not remove runtime message construction.

Use the current `Execute<I>` contract: a borrowed instruction, an owned
associated message, and separate associated effect and error types. Do not
copy the benchmark worktree's different trait signature.

## Execution implementation

Inline the bodies of the current operation handlers into the corresponding
typed `Execute::execute` implementations. Copy and adapt the handler body;
do not implement the typed entry point by calling its existing `op_*` handler.

Shared checked stack, frame, heap, and arithmetic helpers may remain where they
provide useful common behavior. Inlining operation handlers does not require
duplicating those underlying helpers. Preserve entry-point behavior such as
clearing pending program-counter state.

The feature-off path may retain its existing operation handlers and aggregate
dispatch during the transition.

Split typed execution implementations into modules by operation family. A
proposed organization is:

```text
execute/
  mod.rs
  control.rs
  constants.rs
  locals.rs
  heap.rs
  arithmetic.rs
  comparisons.rs
  logic.rs
  bitwise.rs
```

The precise filenames are an implementation choice. Keep instruction behavior
easy to locate and avoid returning to one large aggregate instruction match.

## State and behavior preservation

Preserve CPU state and the existing `pub(crate) stack: Vec<Word>` representation.
Retain checked stack/local access, frame and operand boundaries, operand order,
heap behavior, runtime errors, and control-flow outcomes.

Use the sibling `../vihaco-execute-bench` worktree's per-instruction execution
implementations as references where useful. Do not import its stack wrapper,
separate stack pointer, unchecked indexing, or unsafe operations. Current
checkout helpers and operation bodies are the behavioral authority for safe
execution.

## Defaults adopted for the initial implementation

The implementation uses the proposed defaults:

- Keep readable constant literals, such as `const_i64 -42`, `const_f64 1.5`,
  and `const_bool true`. Parse them directly into encoded `Word` operands.
  Reference constants accept numeric IDs or values.
- Use `Effect = StepOutcome` and `Error = eyre::Report`, returning one outcome
  directly. Composite behavior handles observation and control flow.
- Apply direct runtime parsing throughout the typed-declarations path now,
  removing paired surface associations including `HasInstructionSet::Syntax`.
  Preserve the feature-off implementation during the transition.

## Remaining implementation scope

The first slice covers direct SST parsing and checked CPU execution. Generated
hierarchical bytecode codecs and binary loading of those aggregates remain
separate work; do not claim aggregate bytecode support until implemented and
verified.

Wire identity remains the agreed hierarchical device-code path followed by
authored dialect/instruction identities and payload. Inventory slots, generated
variant names, and declaration order are not wire IDs.

## Validation

Verify direct parsing produces actual runtime leaves and aggregate variants,
including resolved targets, constant values, and numeric references. Verify
labels emit no runtime instructions and do not shift addresses.

Exercise execution and error cases through the real typed CPU: arithmetic,
underflow, invalid locals, heap operations, branch/call/return behavior, print,
and per-instruction messages. Cover repeated and nested CPU destinations,
complete Step coverage, outcomes, and observation where applicable.

Run the CPU and affected workspace checks in separate coherent feature-off and
feature-on graphs, including indirect CPU feature activation. Keep the parser
crates within their existing Rust 1.75 requirement. Use ordinary tests,
doctests, and established trybuild fixtures; do not add nested compilation
runners.

If binary support is included, add exact-byte and round-trip fixtures plus
section-boundary checks. Preserve authored codec semantics.

Implementation was explicitly requested after this document was written. No
commit has been requested. Keep design artifacts local and excluded from
implementation commits, following the existing rework-2 workflow.

## Initial implementation checkpoint

The first slice adds 139 executable CPU leaves and explicit per-instruction
execution implementations grouped by operation family. Runtime constants keep
`Word` fields and use typed pattern bindings with `TryInto` conversions. CPU's state remains `Vec<Word>`
with checked access. Per-instruction messages preserve existing function and
print metadata.

Typed dialect/composite parsing now produces runtime instructions directly.
The typed path has no paired surface association or public `Resolve` bridge.
SST consumes `label @name` outside the executable instruction parser. Function
parameters and return types parse directly into the ABI runtime `Type`, including
CPU spellings `undef`, `fn_ref`, and `heap_ref`.

Tests cover all executable leaf parsers, complete inventory counts, nested and
repeated CPU dispatch, metadata omission, messages, checked frames/locals,
arithmetic, heap, reset, and errors. Direct literal parsing also exposed an
existing integer-overflow panic in the primitive parser; numeric conversion now
returns a parse error, with boundary regression coverage.

The supported CPU feature-off, feature-on, direct runtime-root, and independently
activated indirect graphs pass. The deliberately unsupported framework-only
opt-in graph still rejects the legacy CPU macro, confirming the documented
one-way feature forwarding limitation. A compiled public CPU example computes
`-35` from `-42 + 7`. Generated hierarchical codecs and binary aggregate loading
remain outside this slice. No files have been staged or committed.

## Operand parser selection

Pattern bindings without an annotation use the corresponding runtime field's
`Parse` implementation directly. An annotation, such as `$1:u64`, selects a
parser type and uses `TryInto` to produce that field's runtime type. Failed
conversions return parse errors at the operand's span. Binding indices/names
retain their field identity when captures are reordered.

CPU signed and floating-point constants use local literal types with `Parse`
and `From<Literal> for Word`, giving `TryInto` through Rust's blanket impl.
Unsigned constants, booleans, and numeric reference IDs use standard conversions.
The whole-instruction `#[instruction(parser = ...)]` hook and its factories have
been removed. Parsing still constructs runtime instructions directly.
