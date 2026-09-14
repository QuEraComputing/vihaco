# RFC: Dialect-Based Instruction Dispatch

> **Status:** Draft
> **Owner:** robpatterson13
> **Last revised:** 2026-09-14

## 1. Summary

This RFC proposes reworking instruction dispatch around composable **dialects**
and a single, flattened instruction enum at the composite level.

The proposal separates instruction vocabulary from component definitions. A
`dialect!` declaration owns the instruction types and the runtime and syntax
representation associated with those instructions. Components declare which
dialects they use and implement execution for the instructions they support.
When components are assembled into a composite, the composite builds one
instruction enum containing the instructions exposed by all of its component
fields. Dispatch then executes against that flattened enum rather than adding
another dispatch layer for each component.

## 2. Motivation

The current component and instruction-set model couples a component to the
definition of its instruction set. This makes it difficult to reuse one
instruction vocabulary across multiple components and requires composites to
represent nested instruction sets.

Nested instruction sets also add dispatch work at runtime. A nested design
requires the machine to first dispatch on the composite instruction to select
the component, then dispatch again on that component's instruction enum to
select the operation. Flattening all component instructions into one
composite-level enum reduces this to a single instruction dispatch. The
generated variant already identifies both the target component field and the
operation, giving the runtime a shorter dispatch path and avoiding an
additional layer of enum matching.

The proposed model provides:

- reusable instruction vocabularies shared by multiple components;
- consistent SST syntax and runtime type layout for components using the same
  dialects;
- a composite-level instruction enum that can be dispatched efficiently; and
- explicit compile-time guarantees that a component implements every
  instruction in each dialect it declares.

## 3. Terminology

### Dialect

A dialect is a named collection of instruction types. It owns the instruction
definitions and the generated runtime and syntax representation logic for those
instructions.

The term *dialect* replaces the current formal term *instruction set* for this
concept.

### Component

A component is a stateful executor that declares one or more dialects and
implements `Execute` for each instruction in those dialects.

### Composite

A composite contains component fields. It creates one instruction enum by
flattening the instructions contributed by each field's declared dialects.

## 4. Goals and non-goals

### Goals

1. Decouple dialect declarations from component declarations.
2. Allow multiple components to reuse the same dialect.
3. Preserve the same surface SST syntax and runtime type layout for components
   using the same dialects.
4. Flatten component instructions into one composite-level instruction enum.
5. Define each component's message and effect requirements per instruction.
6. Validate dialect coverage at compile time.
7. Leave instruction bytecode encoding changes for a later phase.

### Non-goals

- This RFC does not finalize the bytecode encoding changes needed to represent
  field-qualified or composite-level instructions.
- This RFC does not define the complete implementation of the dialect callback
  macro; it identifies the metadata that callback must receive.
- This RFC does not introduce dynamic dispatch or trait objects for
  instruction execution.

## 5. Dialect declarations

A dialect is declared independently from any component:

```rust
dialect! {
    arith [
        #[pattern = "'add $0"]
        Add(u32),
        Sub,
        Mul,
    ]
}
```

The macro expands to a module named after the dialect. Each listed instruction
becomes a struct within that module:

```rust
mod arith {
    pub struct Add(pub u32);
    pub struct Sub;
    pub struct Mul;
}
```

The generated instruction types are the shared vocabulary. A component using
`arith` refers to `arith::Add`, `arith::Sub`, and `arith::Mul`, regardless of
which component implements them. Instructions may carry payloads, and their
surface syntax can be described with the same `#[pattern]` attribute currently
supported by `component!`:

```rust
dialect! {
    arith [
        #[pattern = "'add $0"]
        Add(u32),
        Sub,
        Mul,
    ]
}
```

Here, `arith::Add` carries a `u32` payload and parses from an `add` mnemonic
followed by one operand. The dialect owns this pattern and payload shape, so
every component using `arith` sees the same instruction syntax and runtime
layout.

The dialect also owns the generated runtime and syntax representation logic.
Consequently, two components that use the same dialect expose the same surface
syntax and runtime instruction layout. Bytecode encoding is intended to follow
the same ownership model, but the encoding changes are deferred.

## 6. Dialect callback macros

In addition to instruction types, `dialect!` generates a helper macro. For the
example above, the helper is conceptually named:

```rust
__vihaco_arith_instructions!
```

The helper is a callback macro. A composite supplies a callback and a list of
dialects; the dialect helper invokes the callback with the instruction variants
it contributes. This allows a composite to construct its own instruction enum
without requiring a central registry of all dialects.

Conceptually, the generated helper has a shape like this:

```rust
macro_rules! __vihaco_arith_instructions {
    ($callback:ident, [$($variants:tt)*]; $($dialect:ident),*) => {
        $callback! {
            [
                $($variants)*
                <field-name>ArithAdd(/* dialect instruction payload */),
                <field-name>ArithSub(/* dialect instruction payload */),
                <field-name>ArithMul(/* dialect instruction payload */),
            ]
            ; $($dialect),*
        }
    };
}
```

### Component-field qualification

The same dialect may occur on multiple fields of one composite. Those fields
must produce distinct instruction variants even though they use the same
dialect. For example:

```rust
struct Comp {
    a: ArithComponent,
    b: ArithComponent,
}
```

Both fields use the arithmetic dialect, so the composite must distinguish an
instruction targeting `a` from one targeting `b`.

The dialect callback therefore needs component-field metadata in addition to
the dialect name. The field name is used to qualify the generated enum variant
name. At minimum, that metadata must identify:

- the composite field name;
- the component type associated with the field; and
- the generated instruction variant name for that field.

The instruction payload itself remains the dialect instruction payload; field
qualification is expressed by the variant name. For example, the `a` field in
the example above could contribute `AArithAdd`, `AArithSub`, and `AArithMul`,
while the `b` field contributes `BArithAdd`, `BArithSub`, and `BArithMul`.

The exact callback syntax remains to be designed. The important invariant is
that each `(component field, dialect instruction)` pair produces a unique
composite-level instruction variant name.

## 7. Flattened composite instruction sets

A composite uses the dialect helpers to collect variants into one instruction
enum. The conceptual expansion shown in `crates/vihaco-cpu/src/test.rs` is:

```rust
macro_rules! __collect_instruction_variants {
    ([$($variants:tt)*];) => {
        pub enum Instruction {
            $($variants)*
        }
    };

    ([$($variants:tt)*]; $dialect:ident $(, $rest:ident)*) => {
        $dialect! {
            __collect_instruction_variants,
            [$($variants)*];
            $($rest),*
        }
    };
}

make_instruction_enum!(
    __vihaco_arith_instructions,
    __vihaco_logic_instructions,
);
```

The production form must additionally pass the component field metadata needed
to qualify variants. The resulting enum is the composite's instruction stream
type and is the only instruction enum used by execution.

## 8. Nested composite composability

Every composite generates two related artifacts:

1. its own flattened instruction enum, used when that composite executes as a
   machine; and
2. a hidden callback macro that exposes the composite's flattened instruction
   entries to an enclosing composite.

This allows composites to be used at either level. A top-level composite uses
its generated instruction enum directly. A nested composite can instead
contribute its instruction set to the enclosing composite's flattened enum,
where the enclosing composite qualifies the nested instructions with the
nested composite's field name.

For example:

```rust
#[composite]
struct ArithmeticUnit {
    core: Foo,
}

#[composite]
struct System {
    left: ArithmeticUnit,
    right: ArithmeticUnit,
}
```

`ArithmeticUnit` has its own flattened instruction set for direct execution.
It also generates a callback describing that set. `System` can consume that
callback for both `left` and `right`, producing one flattened instruction enum
whose variants identify the complete path to the target operation, such as
`LeftCoreArithAdd` and `RightCoreArithAdd`.

The nested composite does not need a separate runtime dispatch layer when it
is flattened into its parent. Its callback contributes instruction metadata to
the parent's enum-generation step, and the parent generates the final direct
dispatch path. This preserves the single-dispatch performance goal at every
composite that is intended to execute.

Whether a nested composite is flattened or treated as an independently
executing component is determined by how the author declares or composes it;
the same composite type can support both uses.

## 9. Component dialect declarations

Components declare their dialects using the component attribute:

```rust
#[component(dialect = arith)]
struct ArithComponent {
    x: u32,
}
```

Multiple dialects are declared as a set:

```rust
#[component(dialect = { arith, logic })]
struct ControlComponent {
    x: u32,
}
```

The declaration is the component's instruction contract. A component's
behavioral identity is determined by the combination of its state, its
dialects, and its implementations of `Execute` for those dialects.

If two structs share the same state but differ in execution behavior, message
types, or effect types, they are different components. Component authors should
use Rust's newtype pattern when they need such distinct implementations.

## 10. Execution model

Execution is expressed through an `Execute` implementation for each dialect
instruction:

```rust
trait Execute<T>
where
    Self: vihaco::Component,
    T: Instruction,
{
    type Message;
    type Effect;

    fn execute(&mut self, instruction: &T, message: Self::Message) -> Self::Effect;
}
```

A component chooses the message and effect types independently for each
instruction:

```rust
impl Execute<arith::Add> for ArithComponent {
    type Message = (u32, u32);
    type Effect = ();

    fn execute(
        &mut self,
        _instruction: &arith::Add,
        _message: Self::Message,
    ) -> Self::Effect {
        ()
    }
}
```

The composite-level instruction enum carries enough information to select the
target component field and the dialect instruction. Dispatch can therefore
route directly from that enum to the corresponding component execution path.
There is no second instruction-enum dispatch at the component boundary.

## 11. Compile-time dialect coverage checks

The component macro generates generic assertion functions for every
instruction in every declared dialect. Conceptually:

```rust
mod __v_isa_check_component {
    fn __v_isa_assert_implements_add<T: Execute<arith::Add>>() {}
    fn __v_isa_assert_implements_sub<T: Execute<arith::Sub>>() {}
    fn __v_isa_assert_implements_mul<T: Execute<arith::Mul>>() {}

    fn __v_isa_check_component() {
        __v_isa_assert_implements_add::<ArithComponent>();
        __v_isa_assert_implements_sub::<ArithComponent>();
        __v_isa_assert_implements_mul::<ArithComponent>();
    }
}
```

If a component declares a dialect but omits an `Execute` implementation, the
generated assertion fails to type-check. The component macro should surface a
clear diagnostic that identifies the component, dialect, and missing
instruction implementation.

This check enforces the central invariant of the design: declaring a dialect
means implementing the complete execution contract of that dialect.

## 12. End-user ergonomics

The callback macros and generated instruction variants are implementation
details. End users should only need to declare dialects, components, and
composites:

```rust
dialect! {
    arith [Add, Sub, Mul]
}

#[component(dialect = arith)]
struct Foo {
    state: u32,
}

#[composite]
struct Composite {
    a: Foo,
    b: Foo,
}
```

The macros generate the remaining machinery automatically:

1. `dialect!` generates the instruction types, syntax metadata, and a hidden
   callback that forwards the dialect instruction names and types.
2. `#[component]` records the component's dialect declarations and generates
   the compile-time `Execute` coverage checks.
3. `#[composite]` supplies each component field name to the relevant dialect
   callbacks and generates the flattened instruction enum.
4. The generated composite dispatches each qualified variant directly to the
   corresponding component field.

The generated enum is conceptually equivalent to:

```rust
enum Instruction {
    AArithAdd(arith::Add),
    AArithSub(arith::Sub),
    AArithMul(arith::Mul),
    BArithAdd(arith::Add),
    BArithSub(arith::Sub),
    BArithMul(arith::Mul),
}
```

Users do not provide callback invocations, field prefixes, or generated
variant names. Identifier qualification is handled internally by the macro
implementation, either through a private identifier-concatenation helper or
through procedural-macro identifier construction.

## 13. Open design questions

1. What exact token shape should the dialect callback accept for component-field
   metadata?
2. What naming convention should be used when combining a component field name
   with a dialect instruction name for the generated variant?
3. How should an author select between flattening a nested composite and
   treating it as an independently executing component?
4. How should `Message` and `Effect` types be represented when the flattened
   enum contains instructions whose components use different types?
5. Which generated syntax descriptors belong to a dialect, and which require
   composite field qualification?
6. What bytecode encoding changes are needed to preserve field identity after
   flattening?
7. What diagnostics should be emitted for duplicate dialect declarations or
   unsupported component/dialect combinations?

## 14. Initial implementation sequence

1. Introduce the standalone `dialect!` declaration and generated instruction
   modules.
2. Generate dialect callback macros for instruction collection.
3. Define the callback metadata shape for component fields and implement
   field-qualified variant generation.
4. Add component dialect attributes and generated `Execute` coverage checks.
5. Generate a flattened instruction enum for composites.
6. Generate a callback macro for each composite's flattened instruction set.
7. Add nested-composite flattening and field-path qualification.
8. Rework runtime dispatch to execute through that enum only.
9. Add compile-fail and runtime tests for missing implementations, repeated
   dialects, multiple dialects, and distinct per-component message/effect
   types.
10. Design and implement the corresponding bytecode encoding changes.
