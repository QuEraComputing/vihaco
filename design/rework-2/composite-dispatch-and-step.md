# Composite instruction sets, field markers, and routes

Status: agreed design direction from the discussion on 2026-10-07; API sketches
below are illustrative and are not implemented contracts.

This document records the revised direction for rework-2. It supersedes the
authored `#[instruction_set]` selection model and the use of device codes as
`Step` keys in earlier plans. Implementation and the remaining plans still need
to be reconciled with these decisions.

## Composite-generated instruction sets

Users will not author an enum with `#[instruction_set]`. Instead,
`#[composite]` generates an instruction enum from all instructions available
from its subdevices. The generated representation preserves which device field
an instruction targets, including when multiple fields contain the same
component type.

The user-facing, in-memory Rust representation is a single flat enum. Each
variant contains a leaf instruction directly, with a distinct variant for each
destination field path and instruction. Child composite or dialect enums are
not nested inside its payloads:

```rust,ignore
#[composite]
struct Composite {
    #[device(0x42)]
    alu: ALU,

    #[device(1)]
    alu_2: ALU,
}

// Illustrative generated representation; naming remains to be decided.
enum CompositeInstruction {
    AluAdd(arith::Add),
    AluSub(arith::Sub),
    AluMul(arith::Mul),
    Alu2Add(arith::Add),
    Alu2Sub(arith::Sub),
    Alu2Mul(arith::Mul),
}
```

The enum exposes the complete available instruction set. Execution behavior
is supplied through `Step` implementations, and `Dispatch` is generated for
the user from that representation and those implementations.

## Hierarchical bytecode encoding

Flattening the Rust enum does not flatten the encoded instruction identity.
Bytecode encodes the chain of device codes from the outermost composite to the
destination component, followed by the dialect ID and the instruction variant
within that dialect, then the instruction payload:

```text
device code → nested device code → … → dialect ID → instruction variant → payload
```

For example, if an outer machine contains `Composite` at device code `0x07`,
the `alu` field has code `0x42`, and the arithmetic dialect and Add instruction
have IDs `0x02` and `0x03`, respectively, its encoded identity is:

```text
0x07 → 0x42 → 0x02 → 0x03
```

The corresponding outer machine Rust enum still has one variant containing
`arith::Add` directly, such as `CompositeAluAdd(arith::Add)`. Variant names here
are illustrative; naming rules remain to be decided.

Encoding maps each flat variant to its full device-code chain, dialect ID,
instruction variant, and payload. Decoding maps that hierarchical identity
back to the corresponding flat variant. The Rust enum's discriminant or
declaration order does not replace the hierarchical bytecode identity.

## Field identity as a type

Each device field gets its own generated marker type. The marker identifies
the field in its owning composite, rather than the component's type or its
numeric device code. Thus `Composite::alu` and `Composite::alu_2` have distinct
identities even though both contain `ALU`.

The `field!` macro exposes this identity in type position:

```rust,ignore
trait Step<F, I> {
    type Result;

    fn step(&mut self, instruction: I) -> Self::Result;
}

impl Step<field!(Composite::alu), arith::Add> for Composite {
    type Result = ();

    fn step(&mut self, instruction: arith::Add) {
        // Behavior for the first ALU.
    }
}

impl Step<field!(Composite::alu_2), arith::Add> for Composite {
    type Result = ();

    fn step(&mut self, instruction: arith::Add) {
        // Independent behavior for the second ALU.
    }
}
```

Users do not need to know the generated marker's name or module path.
Generated dispatch uses the same marker when delegating a leaf instruction:

```rust,ignore
<Self as Step<field!(Composite::alu), arith::Add>>::step(self, instruction)
```

Explicit device codes remain relevant to bytecode encoding. They do not serve
as the identity keys for `Step` implementations. Source aliases likewise do
not create new field identities.

## Route builders inside Step implementations

The route builders from `../vihaco-static-top-experiment` will be integrated
into `Step` implementations. The field marker used to select a Step can also
select the component through the route builder's `on::<F>()` operation.

For a route that returns the component's effect directly, the intended shape
is:

```rust,ignore
impl Step<field!(Composite::alu), arith::Add> for Composite {
    type Result = eyre::Result<AddEffect>;

    fn step(&mut self, instruction: arith::Add) -> Self::Result {
        route::<Self>()
            .on::<field!(Composite::alu)>()
            .build_message_for::<arith::Add>()
            .then_execute()
            .return_result()
            .finish()
            .run(self, instruction)
    }
}
```

The builder expresses field access, component execution, and handling of the
resulting effect. `build_message_for::<I>()` configures the `BuildMessage` input
of `ExecuteAt`, which uses it to supply the message to `Execute`; it is not a
standalone route stage. Other route operations do not use `BuildMessage`.
Deferred routes can use the experiment's
`emit::<Work>()` or `await_on::<Work>()`, followed by
`continue_with::<Continuation>()`, to produce owned work with a continuation.

The responsibilities are:

- **Composite declarations:** define device fields and generate instruction
  enums and field identities.
- **Generated Dispatch:** selects the appropriate Step for the instruction
  and destination field.
- **User Step implementations:** define behavior using route builders.
- **Routes:** execute the static sequence and return a result or owned work.
- **Drivers:** manage queues, waits, timing, completion, and resumption of
  continuations.

## Reference experiment

The experiment provides two complementary references:

- [`src/test.rs`](../../../vihaco-static-top-experiment/src/test.rs) sketches
  aggregate instruction enums and generated `Dispatch` delegating to `Step`.
  It is unfinished Rust and currently uses const device-code Step keys; this
  document replaces those keys with field marker types and its nested runtime
  enum sketch with a flattened Rust representation.
- [`src/topology.rs`](../../../vihaco-static-top-experiment/src/topology.rs)
  implements the route builder and field, message, work, and continuation
  contracts. [`src/common.rs`](../../../vihaco-static-top-experiment/src/common.rs)
  demonstrates routes shared by emulation and realtime drivers.

The experiment's profile-specific dispatch functions are handwritten examples
of route selection. The intended framework API generates dispatch and places
route definitions inside Step implementations.

## Questions for the next design discussion

The following details remain open:

- How macros generate the complete flat enum from declared dialects and nested
  devices, name its variants, and associate each variant with its bytecode path
  and Step implementation.
- How `field!` resolves markers across modules, aliases, generic composites,
  and nested field paths while preserving ordinary Rust type identity.
- How different Step result types become a composite Dispatch result, including
  driver-facing work enums and error propagation.
- Whether generic whole-dialect Step implementations are supported, and how
  their coverage and coherence interact with instruction-specific implementations.
- How complete generated instruction sets affect surface parsing, operand
  resolution, and loadable section boundaries.
- Which checks require Step coverage for every generated instruction and where
  missing coverage should be diagnosed.

These questions do not change the agreed direction: composite-generated flat
Rust instruction enums, hierarchical bytecode identities, generated dispatch,
field marker types hidden behind `field!`, and route builders within Step
implementations.
