# Load-time verification

Status: proposal. Nothing here is implemented yet.

## Purpose

Reject ill-typed programs before they run. A program that passes verification
cannot hand an instruction an operand of the wrong type, underflow the operand
stack, return the wrong types, or reach a join point with inconsistent state.

The verifier is a **framework**, not a CPU feature. vihaco machines are
composites of components (a CPU, a program/execution device, sequencers, AWGs,
user-defined devices), and each component brings its own instructions and its
own notion of what "well-formed" means. The framework owns the generic parts:
walking control flow, saving and merging abstract state at join points,
reporting errors with function/instruction context. Components own their
**typing rules**. A machine is verifiable exactly when every instruction it
contains has a rule, and Rust checks that at compile time.

Runtime checks (heap object tags, bounds, freed objects) remain. The verifier
removes the failures that are visible in the program text; the runtime guards
what cannot be known before execution.

## Design principle: mirror capability-based lowering

Lowering already solved "per-instruction behavior, supplied by each component,
aggregated by the composite, with requirements checked by Rust"
(`instruction-phases-and-capability-lowering.md`). Verification follows the
same shape:

| Lowering | Verification |
|---|---|
| `Lower<C>` per instruction | `Verify<C>` per instruction |
| Loader capabilities as bounds on `C` (`FunctionLookup`, `StructLookup`) | Verifier capabilities as bounds on `C` (`OperandTypes`, `LocalTypes`, `Signatures`, …) |
| Composite macro generates the aggregate `Lower` | Composite macro generates the aggregate `Verify` |
| `MetadataLoweringContext` provides standard capabilities; loaders wrap it to add more | `ModuleVerifier` provides standard capabilities; machines wrap it to add more |

Component authors already know this pattern, the generated-aggregate machinery
already exists (`inventory.rs`), and a missing rule is a compile error naming
the instruction and the missing capability.

## The instruction rule

```rust,ignore
// vihaco-abi, next to `Lower`
pub trait Verify<C: ?Sized> {
    /// Check this instruction against the abstract state in `cx`, update the
    /// state to its post-state, and report where control goes next.
    fn verify(&self, cx: &mut C) -> eyre::Result<Flow>;
}

pub enum Flow {
    /// Continue at the next instruction.
    Next,
    /// Continue at each listed address; no fallthrough (`br`, `cond_br`).
    Jump(SmallVec<[u32; 2]>),
    /// Leave the function (`ret`); the rule has checked the return types.
    Return,
    /// Stop the machine (`halt`).
    Halt,
}
```

Rules are written against capabilities, never against a concrete verifier:

```rust,ignore
// vihaco-cpu
impl<C: OperandTypes<Type> + ?Sized> Verify<C> for cpu::AddI64 {
    fn verify(&self, cx: &mut C) -> eyre::Result<Flow> {
        cx.pop_expect(&[Type::I64, Type::I64])?;
        cx.push(Type::I64);
        Ok(Flow::Next)
    }
}

impl<C: OperandTypes<Type> + StructTypes<Type> + ?Sized> Verify<C> for cpu::GetField {
    fn verify(&self, cx: &mut C) -> eyre::Result<Flow> {
        let FieldSlot { struct_id, slot } = self.0;
        cx.pop_expect(&[Type::Struct(struct_id)])?;
        let ty = cx.field_type(struct_id, slot)?;
        cx.push(ty);
        Ok(Flow::Next)
    }
}

// vihaco-stdlib (program device)
impl<C: OperandTypes<Type> + Signatures<Type> + ?Sized> Verify<C> for program::Call { … }
impl<C: OperandTypes<Type> + ?Sized> Verify<C> for program::Branch {
    fn verify(&self, _: &mut C) -> eyre::Result<Flow> { Ok(Flow::Jump([self.0].into())) }
}
```

Most rules are a fixed signature. A helper keeps them one line each:

```rust,ignore
impl<C: OperandTypes<Type> + ?Sized> Verify<C> for cpu::AddI64 {
    fn verify(&self, cx: &mut C) -> eyre::Result<Flow> {
        stack_effect(cx, [Type::I64, Type::I64], [Type::I64])
    }
}
```

## Standard capabilities

Defined by the framework, provided by the standard `ModuleVerifier`. Each is a
small trait so rules depend only on what they use.

| Capability | Provides | Used by |
|---|---|---|
| `OperandTypes<Ty>` | `pop_expect`, `pop_any`, `push`, `peek`, `depth` | almost every rule |
| `LocalTypes<Ty>` | `local_type(i)`, `set_local(i, ty)` checks | `load_*`, `store_*` |
| `Signatures<Ty>` | function signature by index/address; current function's return types | `call`, `ret`, `const_fn_ref` |
| `StructTypes<Ty>` | struct field count/type by index | `struct_new`, `get_field`, `set_field` |
| `Constants<V, Ty>` | constant/string table lookups | `const_string`, constant loads |

`Ty` is the module's type vocabulary (`vihaco_abi::Type` for the standard
model). It is generic, as in `ProgramMetadata<V, Ty, Info>`.

## Component-specific capabilities

A component whose correctness depends on more than operand types defines its
own capability and state. Example: a sequencer that must open a channel before
playing on it.

```rust,ignore
// in the sequencer crate
pub trait ChannelState {
    fn require_open(&self, channel: u32) -> eyre::Result<()>;
    fn open(&mut self, channel: u32);
}

impl<C: ChannelState + ?Sized> Verify<C> for seq::Play {
    fn verify(&self, cx: &mut C) -> eyre::Result<Flow> {
        cx.require_open(self.channel)?;
        Ok(Flow::Next)
    }
}
```

The machine's verifier context wraps `ModuleVerifier`, delegates the standard
capabilities, and implements `ChannelState` with its own state. That state
participates in snapshots and merges (below) like the operand stack, so a
channel opened on only one branch is not open after the join.

## The engine

For each function in the module:

1. Start from the entry state: empty operand stack, parameters in their local
   slots with declared types, remaining locals typed from metadata.
2. Run a worklist over instruction addresses within `start..end`. At each
   address, call the aggregate `verify` and follow the returned `Flow`.
3. At every address reached from more than one predecessor (branch targets and
   their fallthrough neighbors), the incoming state is **merged** with the state
   saved for that address.
4. Fail on any rule error, on control leaving the function's range other than
   via `ret`/`halt`, or on falling off the end.

Errors carry context in the style of lowering errors:
`verifying function @main, instruction 5 (cpu::set_field): expected u64, found f64`.

The engine needs one thing from the context beyond the instruction rules:

```rust,ignore
pub trait AbstractState {
    type Snapshot: Clone;
    fn snapshot(&self) -> Self::Snapshot;
    fn restore(&mut self, snapshot: &Self::Snapshot);
    /// Combine `incoming` into the state saved at a join point. Returns whether
    /// it changed; errors when the two cannot be reconciled.
    fn merge(saved: &mut Self::Snapshot, incoming: &Self::Snapshot) -> eyre::Result<bool>;
}
```

### Merging policy

The standard policy is **equality**: at a join point, operand stacks must have
the same depth and identical types, and so must locals. This is what Wasm and
the JVM's stack-map rules effectively require. It is simple, every block is
visited a bounded number of times, and errors point at the exact join that
disagrees. Components with richer state (such as `ChannelState`) choose their
own merge, for example intersecting the set of open channels. Merges must move
up a finite lattice so the worklist terminates.

## Type compatibility

`OperandTypes::pop_expect` asks the type vocabulary whether an actual type
satisfies an expected one:

```rust,ignore
pub trait TypeCompat {
    fn satisfies(actual: &Self, expected: &Self) -> bool;
}
```

For `vihaco_abi::Type`, equality, plus `Struct(n)` satisfies `HeapRef` (a struct
is a heap object, so instructions that accept any object, such as
`heap_dealloc`, accept structs). `HeapRef` does **not** satisfy `Struct(n)`.

## Required metadata changes

- **Local types.** `FunctionInfo` records `local_count` but not local types, and
  locals are written by `store_*`. Replace `local_count` with
  `locals: [Ty, ...]` (parameters first, matching `signature.params`), the way
  Wasm declares locals. Breaking change to the metadata header. The
  alternative, inferring local types, makes merge points ambiguous and error
  messages worse.
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
- `Type` loses `Copy`. Measured impact: one test site in the workspace.
- `Array(_)` and `Struct(_)` both satisfy `HeapRef`, so instructions that accept
  any heap object (`heap_dealloc`) accept both.
- Element types inside `struct N` are checked against the struct table at
  module assembly, like any other `struct N` (`validate_struct_types` recurses).

### Runtime tags

The heap object kind gains the element type: `ObjectKind::Array(Type)`, set at
allocation. Array instructions carry their element type, as `get_field` carries
its struct, and the CPU checks the tag before touching elements:
`type error: heap object 4 is array u64, expected array f64`. This keeps the
runtime guarantee independent of the verifier: a reference that reaches an
array instruction from outside the verified instruction stream (a host value, a
`const_heap_ref`) is still checked.

### Instructions

The CPU ISA already names operand types in its mnemonics (`add_i64`,
`store_f64`). Array instructions follow suit with an element-type operand, and
are renamed from `heap_*` to `array_*` now that the heap holds two kinds of
object:

| Today | Proposed | Stack effect (verifier) |
|---|---|---|
| `heap_alloc N` | `array_new T, N` | `[T × N] → [array T]` |
| `heap_reserve` | `array_reserve T` | `[u64] → [array T]` (capacity) |
| `heap_push` | `array_push T` | `[array T, T] → [array T]` |
| `get_item` | `array_get T` | `[array T, u64] → [T]` |
| (none) | `array_set T` | `[array T, u64, T] → []` (index < len) |
| `heap_len` | `array_len` | `[array _] → [u64]` |
| `heap_dealloc` | `heap_dealloc` | `[heap] → []` (any object) |

- `array_set` is new: arrays are mutable in place within their current length,
  like struct fields; appends stay bounded by the reserved capacity.
- Indices are `u64` everywhere. Today the default build decodes `get_item`'s
  index as a non-negative `i64` while the program-image build pops `u64`.
- `array_len` accepts any element type (`array _`), so it needs no operand.
- Element types are written like any type: `array_new f64, 3`,
  `array_get array u64`. Struct elements can be named,
  `array_new struct @Point, 4`, resolved through `StructLookup` during lowering
  (a `TypeOperand` syntax operand, like `FieldOperand`). The legacy enum build
  takes numeric `struct 0` only.

## Heap references

Heap references are generational keys from the `slotmap` crate, packed into one
64-bit word (32-bit slot index, 32-bit version). Freeing an object bumps its
slot's version, so stale references fail with "invalid or has been
deallocated" instead of silently reaching whichever object reuses the slot. A
reference written as a literal (`const_heap_ref N`) is almost never valid,
because it would also have to match the version. The heap rejects words whose
version half is even (never issued) before decoding them, because slotmap's
`KeyData::from_ffi` would otherwise force the version odd and let a bare slot
index such as `1` reach the first object allocated in that slot.

A standalone benchmark against the previous slot + free-list heap showed no
measurable difference: field reads 1.07 vs 0.99 ns (1,000 live objects) and
19.25 vs 19.39 ns (1,000,000), allocate/free/read churn 14.79 vs 14.81 ns. The
object payload (kind, capacity, element storage) stays vihaco's own.

## Constants

Quantum control programs carry fixed data such as waveforms and calibration
tables. Arrays being first-class suggests array constants in the metadata
constant table, materialized onto the heap at load time:

```text
constants: [array f64 [0.0, 0.25, 0.5]]
```

This needs a value model beyond the `Copy` scalar `Value`. It is listed under
open questions rather than in the first implementation.

## Dynamic cases

| Case | Policy |
|---|---|
| `call_indirect` | The instruction carries the expected signature (index into a signature table). The verifier types the call statically; the runtime checks the target function's signature when calling, as Wasm does. |
| `const_heap_ref N` | Produces `heap`, never `struct N` or `array T`. A forged reference therefore cannot be passed where a struct or typed array is expected, and runtime tags still check it if it reaches `heap`-accepting instructions. |
| Host-supplied values (messages, observer inputs) | Outside the instruction stream. The component's rule declares the type it pushes and the component guarantees it at runtime. |

## Enforcement

Verification should not be optional for execution. Proposal: a `Verified<M>`
wrapper produced only by the verifier, and the stdlib image constructors
(`Image::new`, `Loadable`) accept `Verified<LocalModule<…>>`. Constructing an
image from an unverified module requires an explicit, greppable escape hatch
(`Verified::assume_verified`) for tests and trusted toolchains.

## Placement

- `Verify`, `Flow` in **vihaco-abi** next to `Lower`, so dialect crates depend
  on nothing new and the derive can emit `::vihaco_runtime::Verify` the same
  way it emits `Lower`.
- Aggregate generation in **vihaco-runtime-derive** `inventory.rs`, alongside
  the aggregate `Lower`.
- Engine, standard capabilities, `ModuleVerifier`, `Verified<M>` in a new
  **vihaco-verifier** crate, depending on vihaco-abi and vihaco-module.
- CPU rules in **vihaco-cpu**; program-instruction rules in **vihaco-stdlib**.
- Typed builds only. The legacy enum build has no lowering phase to hook into.

## Implementation order

0. Generational heap references (`slotmap`), so a reference to a freed object
   can never reach the object that reuses its slot. References become full
   64-bit words. See [Heap references](#heap-references).
1. Typed arrays without the verifier: `Type::Array`, element-tagged heap
   objects, the `array_*` instructions with runtime checks, in all three CPU
   builds. Useful on its own, like the struct work, and it fixes the type
   vocabulary before rules are written against it.
2. `Verify`/`Flow` and the generated aggregate, with a trybuild case for a
   missing rule.
3. vihaco-verifier engine with `OperandTypes` and the equality merge policy,
   tested against a small fixture dialect with branches.
4. `locals` metadata change; `LocalTypes`, `Signatures`, `StructTypes`.
5. Rules for program instructions (control flow, calls, locals) and CPU
   instructions (arithmetic, comparisons, structs, arrays).
6. `Verified<M>` enforcement in stdlib images, then docs and guide examples.
7. Signature table for `call_indirect`; array constants (follow-up).

## Open questions

- Array constants: their text and binary encoding, and whether each load
  allocates a fresh heap copy (simple, mutable-safe) or shares one read-only
  object (cheaper, needs an immutability flag on the object).

- Should the CPU's per-instruction rules be generated from the program-image
  build's existing stack types (`PopFromStack<(u64, u64)>`)? Those are
  representation types: `u32` is used for heap ids and `Word` for anything, so
  they cannot express `struct 0` or `f64` vs raw bits. Explicit rules are
  clearer; derivation could be an optional shortcut for purely scalar
  instructions.
- Whether verification should also run per *section*, so a parent section can
  verify cross-section calls, once multi-section programs call into each other.
