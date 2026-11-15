# Load-time verification

Status: implemented through step 6 of the
[Implementation order](#implementation-order) by the stacked PRs into
`feature/typed-heap-verifier`; step 7 lists follow-ups. Sections end with what
was settled in implementation where it refines the design.

## Purpose

Reject ill-typed programs before they run. A program that passes verification
cannot hand an instruction an operand of the wrong type, underflow the operand
stack, call a function with the wrong arguments, return the wrong values, or
reach a join point with an inconsistent stack.

The verifier is a **framework**, not a CPU feature. vihaco machines are
composites of components (a CPU, a program/execution device, sequencers, AWGs,
user-defined devices), and each component brings its own instructions and its
own notion of what "well-formed" means. The framework owns the generic parts:
finding basic blocks, walking control flow to a fixpoint, merging abstract
state at join points, structural checks, and reporting. Components own their
**typing rules**, and may bring their own abstract state. A machine is
verifiable exactly when every instruction it contains has a rule, and Rust
checks that at compile time.

Runtime checks (heap object tags, bounds, freed objects) remain. The verifier
removes the failures that are visible in the program text; the runtime guards
what cannot be known before execution.

This design adopts the stack-validation RFC in #120, which is based on a
validator running in production in bloqade-lanes: an intraprocedural CFG walk to
a fixpoint, per-component stack effects, two-phase reporting, and structural
checks. It departs from the RFC in three ways: rules type values with vihaco's
own types (structs, typed arrays, and named types), replacing the RFC's
embedder-defined value lattice; checks on particular constant values move to
runtime device checks; and components can contribute abstract state beyond the
operand stack. bloqade-lanes is expected to move its validation onto this
framework rather than keep its own.

## Use cases

The open issues that motivate the verifier, and what each needs from it:

| Issue | Use case | What the design provides |
|---|---|---|
| #120 | bloqade-lanes composes `vihaco-cpu` with a lanes device whose operands are `LOCATION`, `LANE`, `ZONE` values, all `u64` to vihaco. It checks that device operands carry the right tag, and checks groups of constant addresses (no duplicate lanes in one `move`). | Device operand kinds become [named types](#named-types) backed by `u64`, so a `LANE` passed as a `LOCATION` is a type error; groups of constant addresses are checked at runtime by the device ([Slots](#slots-and-types)). |
| #120 | A frame-model hazard: a crafted program grew the stack to 68 GB through an out-of-range local index. | Declared local types bound every local index; a metadata cap bounds frame sizes ([Metadata](#required-metadata-changes)). |
| #175 | Records and lists that components and hosts can rely on. | Rules type `struct_new`, `get_field`, and the `array_*` instructions against the struct table and element types. |
| #175, #176 | Observers, debuggers and IDEs need to know what a stack word means at a breakpoint. | The verifier's converged per-block states, exported as **type maps** ([Outputs](#outputs)). |
| #173 | Asynchronous components: `submit` pushes a handle, `wait` pops it and pushes the result later. Lanes' `measure n` pushes measurement futures. | Handles are values with a type; a component-defined slot tag today, a standard `future T` type once #173 lands ([Open questions](#open-questions)). |
| #174 | Execution contexts: `spawn f(args)` and `join`. | Typed like calls: arguments checked against `f`'s signature, `join` pushes `f`'s results. Intraprocedural, so contexts add no new analysis. |
| #176 | A module builder for external compilers. | The verifier is the check a builder's output goes through; builders emit declared local types. |
| #5 | Components define their own runtime types. | Word-backed [named types](#named-types) declared in the module, which component instructions name in their operands and rules. |
| #169 | Instruction-set crates outside the composite's crate, with their own messages. | Rules live with the instruction type, and every capability trait is public and semver-covered, so no `#[doc(hidden)]` plumbing is needed to write a rule. |
| #171 | Cast instructions. | Plain fixed-signature rules: `[i64] → [f64]`. |
| #121 | Timing and latency annotations. | An example of component-defined abstract state (elapsed time per sequencer), with the widening guidance under [Merging](#merging). |

## Design principle: mirror capability-based lowering

Lowering already solved "per-instruction behavior, supplied by each component,
aggregated by the composite, with requirements checked by Rust"
(`instruction-phases-and-capability-lowering.md`). Verification follows the
same shape:

| Lowering | Verification |
|---|---|
| `Lower<C>` per instruction | `Verify<C>` per instruction |
| Loader capabilities as bounds on `C` (`FunctionLookup`, `StructLookup`) | Verifier capabilities as bounds on `C` (`OperandStack`, `Locals`, `Signatures`, …) |
| Composite macro generates the aggregate `Lower` | Composite macro generates the aggregate `Verify` |
| `MetadataLoweringContext` provides standard capabilities; loaders wrap it to add more | `ModuleVerifier` provides standard capabilities; machines wrap it to add more |

Component authors already know this pattern, the generated-aggregate machinery
already exists (`inventory.rs`), and a missing rule is a compile error naming
the instruction or the missing capability.

## The instruction rule

```rust,ignore
// vihaco-abi, next to `Lower`
pub trait Verify<C: ?Sized> {
    /// Where control goes after this instruction. Stateless: it depends only
    /// on the instruction's operands, so the engine can find basic blocks and
    /// reachable code before any state exists.
    fn flow(&self) -> Flow {
        Flow::Next
    }

    /// Check this instruction's inputs against the abstract state in `cx` and
    /// update the state to its post-state.
    fn verify(&self, cx: &mut C) -> eyre::Result<()>;
}

pub enum Flow {
    /// Continue at the next instruction (`call` included: callees are
    /// verified separately).
    Next,
    /// Continue at each listed address, with no fallthrough (`br`, `cond_br`).
    /// At least one address: an empty jump is a structural error (`EmptyJump`),
    /// not a terminator.
    Jump(SmallVec<[u32; 2]>),
    /// Leave the function (`ret`).
    Return,
    /// Stop the machine (`halt`).
    Halt,
}
```

Control flow is a separate, stateless method so that block boundaries and
reachability never depend on whether a rule succeeded (the RFC's "one walker
for reachability and stack checks"). Most instructions keep the default
`Flow::Next`; only control-flow instructions override it.

Rules are written against capabilities, never against a concrete verifier:

```rust,ignore
// vihaco-cpu
impl<C: OperandStack + ?Sized> Verify<C> for cpu::AddI64
where
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        stack_effect(cx, [Type::I64, Type::I64], [Type::I64])
    }
}

impl<C: OperandStack + StructTypes + ?Sized> Verify<C> for cpu::GetField
where
    C::Slot: TypedSlot,
{
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        let FieldSlot { struct_id, slot } = self.0;
        cx.pop_expect(&Type::Struct(struct_id))?;
        let ty = cx.field_type(struct_id, slot)?;
        cx.push(C::Slot::of_type(ty));
        Ok(())
    }
}

// vihaco-stdlib (program device)
impl<C: OperandStack + ?Sized> Verify<C> for program::Branch {
    fn flow(&self) -> Flow {
        Flow::jump(self.0)
    }
    fn verify(&self, _: &mut C) -> eyre::Result<()> {
        Ok(())
    }
}
```

A rule that fails ends the analysis of its basic block; see
[Diagnostics](#diagnostics).

## Slots and types

The abstract stack holds one **slot** per operand and per local. A slot is a
vihaco type, or a conflict where paths disagreed:

```rust,ignore
pub enum TypeSlot {
    Known(Type),
    /// Paths disagreed at a join. Using it where a type is required is a
    /// `TypeMismatch` that names both types.
    Conflict(Type, Type),
}
```

Every standard context and rule uses `TypeSlot`. Facts that components need
beyond vihaco's types are expressed in one of two ways, so there is no
embedder-defined slot type:

- **Distinct kinds of word-sized operand** become [named types](#named-types):
  bloqade-lanes' `LOCATION`, `LANE`, and `ZONE` are distinct types backed by
  `u64`, so mixing them up is an ordinary `TypeMismatch` (#120, #5).
- **Facts about particular values**, such as "no duplicate lanes in one
  `move`", are checked at runtime by the device. Tracking known constants
  through the verifier would catch them only when every value is a literal,
  and is what would force a pluggable slot type.

The engine's traits stay generic (`Lattice` for joins, `TypedSlot` for slots
that can represent vihaco types, with `TypeSlot` implementing both), which
keeps it testable with small fixture slots; that generality is not an
extension point the standard contexts promise.

The standard rules are **sound** for type safety: a conflicted slot never
satisfies a typed pop. (The RFC lets an unknown tag pass a typed pop.)

Compatibility for `vihaco_abi::Type`: equality, plus `Struct(n)` and `Array(_)`
satisfy `HeapRef`, so instructions that accept any heap object (`heap_dealloc`)
accept both. `HeapRef` satisfies neither, and a named type satisfies only
itself.

## Named types

A module can declare **named types**: nominal types represented by a word-sized
scalar. They exist only for verification; at runtime a value of a named type is
the same word as its representation, with no tag and no cost.

```text
metadata {
    strings: ["main", "lanes.sst", "location", "lane"],
    types: [
        { name: 2, repr: u64 },   // type 0: location
        { name: 3, repr: u64 },   // type 1: lane
    ],
    // ...
}
```

- `Type::Named(u32)`, written `type N`, an index into the `types` table, like
  `struct N`. Instruction operands may name it, `@location`, resolved during
  lowering.
- `repr` is a scalar word type (`i32`, `i64`, `u32`, `u64`, `f32`, `f64`,
  `bool`). Names are unique; module assembly checks every `type N` reference,
  including inside arrays and struct fields.
- **Nominal:** `type 0` satisfies only `type 0`, not its representation and not
  another named type with the same representation. Two instructions convert
  explicitly, and are no-ops at runtime:

  | Instruction | Stack effect |
  |---|---|
  | `wrap @location` | `[u64] → [location]` |
  | `unwrap @location` | `[location] → [u64]` |

- Devices state their operand types by name: a lanes rule pops
  `@location`, resolved against the module's table through a `NamedTypes`
  capability, so a device works in any module that declares its types.
- Function signatures, locals, struct fields, and array elements may all use
  named types (`array location` for a group of addresses).

Settled in implementation:

- `type N` is encoded as byte `0x0D` followed by `N` as a little-endian
  `u32`, like `struct N`.
- `wrap` and `unwrap` take a named type by name or index, `wrap @location` or
  `wrap 0`, like `struct_new`. Type operands of the array instructions accept
  `type @location`. Lowering resolves both through a `NamedTypeLookup`
  capability, parallel to `StructLookup`; the legacy enum build takes numeric
  `wrap 0` only.
- At runtime `wrap` and `unwrap` still require an operand, so they fail on an
  empty stack like any other one-operand instruction.
- Module assembly checks representations through `StructTypeRef`, which
  gains `visit_named_refs` and `is_named_type_repr` with defaults that declare
  no named types, so custom type models keep compiling and must keep `types`
  empty.

## Standard capabilities

Defined by the framework, provided by the standard `ModuleVerifier`. Each is a
small public trait, so rules depend only on what they use (and rules in
external crates need nothing hidden, #169).

| Capability | Provides | Used by |
|---|---|---|
| `OperandStack` | `type Slot`; `pop`, `pop_expect`, `pop_n`, `push`, `peek`, `depth` | almost every rule |
| `Locals` | `local(i)`, `set_local(i, slot)`, `local_count` | `load_*`, `store_*` |
| `Signatures<Ty>` | signature by function index or address; the current function's signature | `call`, `ret`, `const_fn_ref`, `spawn` |
| `StructTypes<Ty>` | field count and field type by struct index | `struct_new`, `get_field`, `set_field` |
| `Constants<V, Ty>` | constant and string table lookups | `const_string`, constant loads |

Pops are bounded: `pop_n(n)` costs at most `O(depth)` and yields one underflow,
so a huge count in a crafted program cannot blow up verification.

Settled in implementation:

- `Locals` extends `OperandStack`, so locals and operands share one `Slot`
  type and a rule bounding `C: Locals` names `C::Slot` unambiguously.
- `Signatures::function_at(address)` resolves a direct `call` by address the
  way the runtime does: the first function in table order starting there.
- `Constants<V, Ty>` also answers `constant_type(i)`, the type a constant load
  pushes (`Value::type_of` for the standard model).
- `call_effect(cx, callee, n)` and `return_effect(cx, k)` implement the call
  and return rules below over `OperandStack + Signatures<Type>`, so program
  and CPU rules (and external dialects) share one implementation.

## Component-specific capabilities and state

A component whose correctness depends on more than operand types defines its
own capability and state. Example: a sequencer that must open a channel before
playing on it.

```rust,ignore
// in the sequencer crate
pub trait ChannelState {
    fn require_open(&self, channel: u32) -> Result<(), VerifyError>;
    fn open(&mut self, channel: u32);
}

impl<C: ChannelState + ?Sized> Verify<C> for seq::Play {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        Ok(cx.require_open(self.channel)?)
    }
}
```

The machine's verifier context wraps `ModuleVerifier`, delegates the standard
capabilities, and implements `ChannelState` with its own state. The framework
provides this wrapper as `WithState<'m, S>`: `S: ComponentState` (a `join`
with the same contract as `AbstractState::join`; tuples combine several
components' states) is snapshotted and joined alongside the frame, and the
component crate implements its capability trait for `WithState<'_, S>`. A
hand-written context can instead delegate to `ModuleVerifier` directly and
build its entry states with `function_spans()` and `FunctionSpan::map_entry`. That state
takes part in snapshots and joins (below) like the operand stack, so a channel
opened on only one branch is not open after the join (the join intersects the
open sets).

Component state and calls. Every function is verified from the same entry
component state, so that state must assume nothing a caller might not have
established (for the sequencer: no channels open). A call's operand effect
comes from the callee's signature, but its effect on component state is
unknown to the caller, so after the operand effect the call helpers apply
`Signatures::after_call(callee)`. `WithState` implements it with
`ComponentState::after_call(&mut self, callee: Option<u32>, entry: &Self)`,
whose default resets the caller's state to `entry`: nothing the caller
established is assumed to survive the call. `ModuleVerifier` has no state
beyond the frame and does nothing. This is sound and strict (a helper cannot
play on a channel its caller opened). A component may override `after_call`
with any state that assumes no more than a callee could leave behind, such as
"unknown" for an elapsed-time budget. Per-callee summaries (the callee's exit
state, computed in call-graph order) are a possible future extension.

## The engine

### Functions and blocks

Verification is **intraprocedural**: each function in the metadata table is
verified on its own, from its own entry state. Because a callee can never pop
below its own operands (vihaco's frame model), a caller's depth after `call` is
fixed by the callee's declared signature, so no call-graph order or summaries
are needed and recursion is not a special case (#120).

For each function `start..end`:

1. **Leaders.** Using only `flow()`: the entry, every jump target, and every
   address after a jump. Joins happen only at leaders, and only leaders store
   a state, so a straight-line function stores one state.
2. **Reachability.** A walk over `flow()` alone marks reachable addresses.
3. **Fixpoint.** A worklist over leaders. Each block is transferred from its
   leader's state by calling `verify` on each instruction. Successor leaders
   receive the post-state; a leader is requeued only when its stored state
   **strictly** changes under `join`. Finite-height lattices guarantee
   termination.
4. **Replay.** Each reached block is replayed once, in address order, from its
   leader's converged state, with reporting on.

Errors found while converging are discarded: every block but the last
transfer sees an unconverged state. The replay yields at most one report per
defect, in address order, even inside loops (#120).

### Entry state and frame rules

- Operands: empty.
- Locals: parameters typed from the signature, then the remaining declared
  locals. (At runtime locals start zeroed; a zero heap reference is invalid and
  is caught by the runtime if used before being stored.)
- Popping an empty operand stack is `StackUnderflow`: a function may not reach
  into its locals or its caller's frame.
- `call f` pops the callee's parameter count and checks each argument against
  the declared parameter type, then pushes the declared results and applies
  `after_call` to component state (see above).
- `ret k`: `k` must equal the declared return count and the top `k` operands
  must match the declared return types; anything below is dropped, as at
  runtime.

### The engine's contract with the context

```rust,ignore
pub trait AbstractState {
    type Snapshot: Clone;
    fn snapshot(&self) -> Self::Snapshot;
    fn restore(&mut self, snapshot: &Self::Snapshot);
    /// Join `incoming` into the state saved at a leader. Returns whether the
    /// saved state strictly changed, or a structural mismatch.
    fn join(saved: &mut Self::Snapshot, incoming: &Self::Snapshot) -> Result<bool, VerifyError>;
    /// Called before verifying each instruction, so errors carry addresses.
    fn set_address(&mut self, function: u32, address: u32);
}
```

`ModuleVerifier` implements this for the operand stack and locals; a wrapping
context joins its own state alongside.

## Merging

At a leader:

| Saved | Incoming | Result |
|---|---|---|
| depth `d` | depth ≠ `d` | `StackDepthMismatch { expected: d, found }`; the saved state stands, not requeued |
| depth `d` | depth `d` | slot-wise `join` of locals, then operands; requeue only if some slot changed |

Depth must not depend on which edge was taken (the Wasm and JVM rule). This
catches unbalanced diamonds at the join and loops with a nonzero depth delta at
the loop header, while a zero-delta loop converges.

Component state chooses its own join, which must have finite height. Unbounded
abstract values, such as elapsed time on a sequencer (#121), must widen: for
example, to "unknown" once a loop header has been joined more than a fixed
number of times.

## Diagnostics

Verification reports every problem at once, compiler-style:

```rust,ignore
pub struct Diagnostic {
    pub function: u32,
    pub address: u32,
    pub severity: Severity,   // Error or Warning
    pub kind: DiagnosticKind,
}

pub enum DiagnosticKind {
    StackUnderflow,
    StackDepthMismatch { expected: usize, found: usize },
    TypeMismatch { expected: String, found: String },
    CallArityMismatch { callee: u32, declared: usize, found: usize },
    ReturnCountMismatch { declared: usize, found: usize },
    LocalIndexOutOfRange { index: u32, count: u32 },
    LocalCountMismatch { expected: usize, found: usize },
    InvalidControlFlowTarget { target: u32 },
    /// A `Flow::Jump` with no targets: a jump must name an address.
    EmptyJump,
    /// A function's `start..end` is reversed or extends past the code.
    InvalidFunctionSpan { start: u32, end: u32, code_len: usize },
    MissingTerminator,
    Unreachable,
    /// The operand count or shape comes from a runtime value.
    Unverifiable { reason: String },
    /// A component-defined error, from a rule or capability.
    Rule(String),
}
```

- Standard capabilities return a `VerifyError` that maps to a structured kind;
  any other error a rule returns becomes `Rule`.
- A rule error ends its block, so a block reports at most one rule error, and
  the block passes no state to its successors. Blocks with states from other
  paths still report; code reachable only through the failed block is not
  analysed and has no type-map entry, which is safe because verification has
  already failed. Because a block may pass states on in early fixpoint rounds
  and fail only at convergence, the engine then reruns the fixpoint with the
  out-edges of failing blocks cut. It settles on a cut set `C` where every
  uncut block passes in `fixpoint(C)` and every cut block `L` fails in
  `fixpoint(C \ {L})`, alternately cutting uncut blocks that fail and
  restoring cut blocks whose failure was a follow-on effect (they pass once
  the other cut blocks pass nothing on). Each block is restored at most once,
  so this terminates, and the cut set never empties while a block fails. Only
  a cut block that has a state in `fixpoint(C)` and that another cut block
  reaches needs a fixpoint of its own, and only if that other block passed a
  state on in some round of the uncut fixpoint (cutting a block that never did
  changes nothing). A failing block usually fails in every round, so
  independent errors that fail on first entry cost a few fixpoints, not one
  each, even when each failing branch rejoins the code after it. Others, such
  as loop headers that pass on first entry and fail after the back edge,
  cost a fixpoint each, so these reruns are capped at a fixed budget (64
  whole-function fixpoints per function): rejecting a function costs at most
  the budget plus two fixpoints. Past the budget the engine stops refining
  and keeps the current cut set, so the function is still rejected with
  every real error found, but a follow-on diagnostic may appear; the
  guarantees below hold within the budget.
- A failing block `L` is replayed, and mapped, from its state in
  `fixpoint(C \ {L})`: fed by analysed blocks and by `L`'s own loops (which is
  what makes a loop header fail), but not by another failing block. Joins are
  checked only between edges from analysed blocks. So no diagnostic stems
  from a state another failing block passed on, and code reachable only
  through failing blocks has no type-map entry. (A block cut again after its
  one restore keeps the state it was cut at, as a fallback; past the budget,
  a cut block is replayed from its state in `fixpoint(C)` if it fails there,
  else from the state it was cut at.)
- Consecutive underflows at one address collapse into one: a rule returns one
  result per address, and the replay visits each address once.
- `Unreachable` is a **warning**: unreachable code never runs, and code
  generators emit it routinely. One warning is reported per contiguous run of
  unreachable code, at its first address; unreachable code is not verified and
  its jump targets are not checked. Every other kind is an error.
- Diagnostics are sorted by function and address. `DiagnosticKind`
  is `#[non_exhaustive]`, so new structural checks are not breaking changes.
- `Unverifiable` is the standard error for an instruction whose stack effect
  depends on a runtime value, such as a count popped from the stack. Device
  instructions must carry counts and shapes as immediates (`sum_f64 4`, a
  lanes `move` with its lane count in the instruction) so that rules can check
  them; a rule for an instruction that cannot follow this reports
  `VerifyError::unverifiable(reason)` instead of an ad-hoc `Rule` message.

## Outputs

A successful verification produces:

- **`Verified<M>`**, the only way to build a stdlib image (see
  [Enforcement](#enforcement)).
- Warnings (`Unreachable`).

Both a successful verification (`Verification<S>`) and a rejection
(`Rejection<S>`) optionally carry **type maps**: each reached leader's converged
state. Any address's state is one block replay away, so debuggers and observers
can show what each local and operand slot holds at a breakpoint (#175, #176),
and an editor can show types next to errors. They are off by default
(`VerifyOptions::with_type_maps`). In a rejection, a leader has an entry only if
some block transfer reached it without a rule error, so leaders reachable only
through failing blocks, and functions with an invalid span, have none.

Settled in implementation (step 6): `Rejection` is generic over the snapshot
type, like `Verification`, and converts into an `eyre::Report` when the
snapshot is `Send + Sync + 'static`; `Rejection::without_type_maps` drops the
maps otherwise.

## Required metadata changes

- **Local types.** `FunctionInfo` records `local_count` but not local types,
  and locals are written by `store_*`. Replace `local_count` with
  `locals: [Ty, ...]` (parameters first, matching `signature.params`), the way
  Wasm declares locals. Breaking change to the metadata header. Inferring local
  types instead makes merge points ambiguous and error messages worse.
- **Frame size cap.** `ExecutionState` sizes each frame from the metadata, so a
  header claiming four billion locals makes function entry try to allocate
  about 32 GB. With declared local types a frame is bounded by the header's own
  size; metadata validation also enforces an explicit maximum,
  `MAX_LOCALS = 65_536` slots. The stdlib `ExecutionState` and the CPU's
  frame entry re-check it before allocating, and `ModuleVerifier` reports a
  function whose frame metadata breaks these rules (too many locals, fewer
  locals than parameters, or locals disagreeing with parameter types), since
  modules can be built without metadata validation.
- **Array types.** Arrays are first-class and typed by element; see
  [Arrays](#arrays).

## Arrays

Arrays are a first-class type, typed by their element, and checked both by the
verifier and at runtime, on the same terms as structs.

### The type

`Type::Array(Box<Type>)`, written `array <elem>`:

```text
array f64              ; a waveform
array struct 0         ; an array of Points
array array u64        ; nested: one u64 array per channel
```

- Structural, not nominal: two `array f64` types are the same type, so arrays
  need no table, unlike structs.
- Encoded as byte `0x0C` followed by the element type's encoding (recursive).
- Nesting is limited to `Type::MAX_ARRAY_DEPTH` (32) levels. Decoding counts
  the `0x0C` prefixes and parsing counts the `array` prefixes, both without
  recursion, so hostile input cannot overflow the stack; the parse error names
  the depth. Lowering rejects an array instruction whose element type is
  already at the limit, and so do the `array_new`/`array_reserve` rules, so no
  runtime array type is deeper either.
- `Type` loses `Copy`. Measured impact: one test site in the workspace.
- Element types inside `struct N` are checked against the struct table at
  module assembly, like any other `struct N`.

### Runtime tags

The heap object kind carries the element type, set at allocation. Array
instructions carry their element type, as `get_field` carries its struct, and
the CPU checks the tag before touching elements:
`type error: heap object … is array u64, expected array f64`. This keeps the
runtime guarantee independent of the verifier: a reference that reaches an
array instruction from outside the verified instruction stream (a host value,
or a `const_heap_ref` literal in the legacy enum build) is still checked.

### Instructions

The CPU ISA already names operand types in its mnemonics (`add_i64`,
`store_f64`). Array instructions follow suit with an element-type operand, and
are renamed from `heap_*` to `array_*` to distinguish them from struct
allocation:

| Before | After | Stack effect (verifier) |
|---|---|---|
| `heap_alloc N` | `array_new T, N` | `[T × N] → [array T]` |
| `heap_reserve` | `array_reserve T` | `[u64] → [array T]` (capacity) |
| `heap_push` | `array_push T` | `[array T, T] → [array T]` |
| `get_item` | `array_get T` | `[array T, u64] → [T]` |
| (none) | `array_set T` | `[array T, u64, T] → []` (index < len) |
| `heap_len` | `array_len` | `[array _] → [u64]` |
| `heap_dealloc` | `heap_dealloc` | `[heap] → []` (any object) |

- `array_set` makes arrays mutable in place within their current length, like
  struct fields; appends stay bounded by the reserved capacity.
- Indices are `u64` in every build.
- `array_len` accepts any element type, so it needs no operand.
- Struct elements can be named, `array_new struct @Point, 4`, resolved through
  `StructLookup` during lowering. The legacy enum build takes numeric
  `struct 0` only.

## Heap references

Heap references are generational keys from the `slotmap` crate, packed into one
64-bit word (32-bit slot index, 32-bit version). Freeing an object bumps its
slot's version, so stale references fail with "invalid or has been
deallocated" instead of silently reaching whichever object reuses the slot.
The version is 32 bits and wraps, so this holds for up to 2^31 reuses of one
slot.

References are predictable, not secret: a fresh heap always issues the same
sequence (the first object is `0x1_0000_0001`). What holds is narrower:

- stale references are rejected (within the wrap bound above);
- every access is checked against the object's tag, so a guessed word still
  has to name the right kind of object;
- typed dialects have no instruction that writes a reference literal, so a
  typed program can only use references it allocated or was given (entry
  arguments, host-supplied values). The legacy enum build keeps
  `const_heap_ref N`, and there a correctly guessed word reaches the object.

The heap rejects words whose
version half is even (never issued) before decoding them, because slotmap's
`KeyData::from_ffi` would otherwise force the version odd and let a bare slot
index such as `1` reach the first object allocated in that slot.

A standalone benchmark against the previous slot + free-list heap showed no
measurable difference: field reads 1.07 vs 0.99 ns (1,000 live objects) and
19.25 vs 19.39 ns (1,000,000), allocate/free/read churn 14.79 vs 14.81 ns. The
object payload (kind, capacity, element storage) stays vihaco's own.

## Dynamic cases

| Case | Policy |
|---|---|
| `call_indirect` | The instruction carries its expected signature inline (`call_indirect (u64, f64) -> u64`). The verifier types it like a direct call; the runtime checks that the target function's signature matches before calling, as Wasm does. No signature table is needed. |
| Reference literals | Typed dialects have no `const_heap_ref`: a typed program cannot write a reference, so the verifier never has to type one. (The legacy enum build keeps `const_heap_ref N`; it is unverified and relies on runtime tags.) |
| Entry arguments from the host | Arity is checked at entry today; values are words. The host is trusted to pass the declared types, as with any foreign call. |
| Host-supplied values (messages, completions of pending instructions) | Outside the instruction stream. The component's rule declares the type it pushes and the component guarantees it at runtime. |
| Operand counts from runtime values | Not verifiable. Instructions carry counts as immediates; otherwise the rule reports `Unverifiable` ([Diagnostics](#diagnostics)). |

Settled in implementation (step 5):

- The `call_indirect` operand is a `vihaco_abi::FunctionType`, written like a
  function header's types: `(u64, f64) -> u64`, `() -> ()`, or
  `(u64) -> (u64, f64)`. The function reference is on top, above the
  arguments. The stdlib program device compares the operand with the target's
  metadata signature; the `typed-declarations` CPU receives the target's
  signature in a new `message::IndirectCallee` and compares it.
- In the typed dialects the signature's types may name structs and named
  types, alongside the numeric forms:
  `call_indirect (struct @Point, type @location) -> u64`. The syntax operand
  is `vihaco_cpu::field::FunctionTypeOperand` (a list of `TypeOperand`s), and
  lowering resolves it through `StructLookup` and `NamedTypeLookup`, the same
  mechanism as `array_new struct @Point, 4`, checking every reference against
  the tables. Unknown names are load-time errors. The runtime instruction and
  the binary encoding keep the resolved `FunctionType`, so bytecode carries
  indices only. (Step 6.)
- Decided: the legacy enum build keeps an operand-free, unsigned
  `call_indirect` with no check. It has no verifier and no lowering phase, and
  the typed builds are where checked indirect calls live.
- Decided: the `typed-declarations` CPU's `print` pops a `str` (a string-table
  reference), `[str] -> []`; printing other values is a host or observer concern.
- Untyped locals are `load i` and `store i`. A typed `load_<t>` / `store_<t>`
  requires the local to be declared exactly `<t>`. Every store checks the
  operand against the declared type and leaves the local's slot unchanged, so
  locals keep their declared types for the whole function.
- `TypedSlot::known_type` exposes a slot's type, so rules can accept a family
  of types (`array_len` takes any `array _`). Shared helpers in
  vihaco-verifier (`load_local`, `store_local`, `direct_call_effect`,
  `indirect_call_effect`, `string_constant`, `function_constant`, `pop_array`)
  keep the CPU and program-device rules identical.
- `struct_new` also checks that its lowered field count matches the struct
  table, because the runtime pops the lowered count.

## Enforcement

Verification should not be optional for execution: a `Verified<M>` wrapper
produced only by the verifier, and the stdlib image constructors (`Image::new`,
`Loadable`) accept `Verified<LocalModule<…>>`. Constructing an image from an
unverified module requires an explicit, greppable escape hatch
(`Verified::assume_verified`) for tests and trusted toolchains.

Settled in implementation (step 6):

- `Verified<M>` lives in vihaco-verifier, as placed. vihaco-stdlib already
  depends on it (for the program-instruction rules) under
  `program-image-execution`, and vihaco-verifier depends only on vihaco-abi and
  vihaco-module, so there is no cycle. vihaco-stdlib re-exports `Verified`.
- Constructors: `Verified::new(module)` (standard `ModuleVerifier`, default
  options, warnings dropped), `Verified::verify(module, options)` (also
  returns the `Verification`), `Verified::verify_with_state(module, entry,
  options)` (a `WithState` context), and `Verified::assume_verified(module)`.
  The first three exist for `LocalModule<I, Value, Type, Info>` with
  `I: for<'m> Verify<…>`; other module models and hand-written contexts use
  `assume_verified` after verifying. A rejected module is consumed; callers who
  need it back verify a borrow with `verify_module` first.
- `Verified` gives shared access only (`Deref`, `get`), and `into_inner`
  returns the module unverified, so a verified module cannot be mutated in
  place.
- `Image::new(Verified<LocalModule<…>>)`; the image's module is private, read
  through `Image::module()`, and `Image::into_module()` returns the
  `Verified` module. `Loadable::new` and `replace_image` take an `Image`, so
  they need no change.
- The free functions `execution_state::execute` and `resume` keep taking a
  plain `&LocalModule`: they are the low-level runner that `Loadable` and
  custom owners build on, and the module type is also what `Dispatch` and
  `Step` receive. The runtime's own checks (heap tags, bounds, call
  signatures) remain in force for code run through them.

## Placement

- `Verify`, `Flow` in **vihaco-abi** next to `Lower`, so dialect crates depend
  on nothing new and the derive can emit `::vihaco_runtime::Verify` the same
  way it emits `Lower`.
- Aggregate generation in **vihaco-runtime-derive** `inventory.rs`, alongside
  the aggregate `Lower`.
- Engine, `Lattice`, `TypedSlot`, `TypeSlot`, standard capabilities,
  diagnostics, `ModuleVerifier`, `Verified<M>` in a new **vihaco-verifier**
  crate, depending on vihaco-abi and vihaco-module.
- CPU rules in **vihaco-cpu**; program-instruction rules in **vihaco-stdlib**.
- The `vihaco` facade re-exports the crate as `vihaco::verifier` (plus
  `Verified` and `verify_module` at the root) behind a `verifier` feature,
  which implies `typed-declarations` and forwards vihaco-verifier's own
  `typed-declarations`; `program-image-execution` implies `verifier`.
- Typed builds only. The legacy enum build has no lowering phase to hook into.

## Implementation order

0. Generational heap references (`slotmap`).
1. Typed arrays: `Type::Array`, element-tagged heap objects, the `array_*`
   instructions with runtime checks, in all three CPU builds.
2. `Verify` (`flow` and `verify`), `Flow`, and the generated aggregate, with
   trybuild cases for a missing rule and a missing capability.
3. vihaco-verifier engine: leaders and reachability from `flow`, the worklist
   fixpoint with strict-change joins, two-phase reporting, structural checks,
   structured diagnostics, `Lattice`/`TypedSlot`/`TypeSlot`, and
   `OperandStack`. Tested against fixture dialects: balanced and unbalanced
   diamonds, zero- and nonzero-delta loops, unreachable code, missing
   terminators, invalid targets, type conflicts at joins, and component state.
4. Metadata: `locals: [Ty]` replaces `local_count`, plus the frame-size cap;
   `Locals`, `Signatures`, `StructTypes`, `Constants`; `ModuleVerifier` built
   from module metadata.
5. Named types (`types` table, `Type::Named`, `wrap`/`unwrap`, a `NamedTypes`
   capability). Rules for program instructions (control flow, calls, returns,
   locals, `call_indirect` with an inline signature) and CPU instructions
   (arithmetic, comparisons, casts, structs, arrays).
6. `Verified<M>` enforcement in stdlib images, type maps (also with
   rejections), the `Unverifiable` diagnostic, names in `call_indirect`
   signatures, the facade `verifier` feature, docs and guide examples
   (including a verification guide). Closes #120. **Done.**
7. Follow-ups: array constants; `future T` with #173 and context handles with
   #174; deriving rules from stack messages.

## Open questions

- **Futures and handles.** #173 and #174 need typed handles. A standard
  `future T` type (like `array T`) would let handles cross components and
  hosts; until then, a component can declare a named type per handle kind.
  Which comes first depends on #173's chosen shape.
- **Named type representations.** Whether a named type may wrap a heap type
  (a named `array f64`, for example a `waveform`), which would need the runtime
  heap tag to carry the name too, or only word-sized scalars as proposed.
- **Deriving rules.** Program-image instructions already declare stack shapes
  as Rust types (`PopFromStack<(u64, u64)>`). Those are representation types
  (`u64` is also a heap reference, `Word` is anything), so they cannot express
  `struct 0` or `array f64`. Explicit rules are clearer; a derive could
  generate rules for purely scalar instructions and check the two agree.
- **Array constants**: their text and binary encoding, and whether each load
  allocates a fresh heap copy (simple, mutable-safe) or shares one read-only
  object (cheaper, needs an immutability flag).
- **Sections.** Whether verification should also run per section, so a parent
  section can verify cross-section calls, once multi-section programs call into
  each other (#125).
- **Ownership.** #175 asks for a defined lifetime for heap values. The verifier
  could later track ownership (each reference freed at most once on every
  path), but that needs a linear or affine discipline the type system does not
  have yet.
