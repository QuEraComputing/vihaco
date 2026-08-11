# `component!` Macro Design

## Status

Design plan; implementation is intentionally out of scope.

## Purpose

`component!` declares a reusable component and its runtime instruction products.
It gives each instruction its own product type and places those products in a
stable namespace derived from the component name.

The macro is a declaration and association boundary. It is not the machine
instruction-set boundary.

## Responsibilities

The macro should:

- Declare the component state type.
- Declare owned runtime instruction product types.
- Preserve generic parameters, const generics, lifetimes where supported, and
  `where` clauses.
- Support unit, tuple, and named-field instruction products.
- Make runtime product types constructible by composite-generated resolution,
  either through public fields or generated public constructors.
- Provide a stable generated namespace, normally snake case derived from the
  component type name.

The macro must not:

- Define source syntax or parser patterns.
- Resolve labels, strings, types, or other module-wide source information.
- Choose which instructions a machine exposes.
- Generate a component-wide execution dispatch match.
- Require one message, effect, or fault type for every instruction.
- Inspect, generate, or validate `Execute<I>` implementations.
- Assign persistent opcodes or silently derive bytecode codecs.
- Require a component-wide instruction enum as the execution boundary.

## Proposed input

The initial declaration shape is:

```rust
component! {
    component GateBeam {
        measure_sites: HashMap<AtomId, Vec<[f64; 2]>>,
        local_x_tolerance_um: f64,
        local_y_tolerance_um: f64,
        measure_x_tolerance_um: f64,
        measure_y_tolerance_um: f64,
        cz_pair_radius_um: f64,
    }

    instruction {
        TopHatCZ,
        GlobalRZ,
        GlobalR,
        LocalRZ,
        LocalR,
        DefineMeasureSites,
        Measure,
        Reset,
    }
}
```

The declaration contains runtime products only. Surface names and patterns are
declared by a composite or a separate surface-instruction declaration selected
by the composite.

The planned optional component syntax declaration is a sibling block after the
runtime `instruction` block. Its exact input shape is:

```rust
syntax {
    value LabelRef = "'@' ident";

    value Value {
        U32(u32),
        Label(LabelRef),
    }

    type Type {
        I64 = "`i64`";
        U32 = "`u32`";
    }

    instruction {
        Step(value: Value) = "'step $value";
        Branch(target: Value) = "'br $target";
        Add(ty: Type) = "'add $ty";
        Reset = "'reset";
    }
}
```

This block produces a component-local `syntax` module and an
`InstructionSet` implementation. It does not receive an alias, device code,
composite, or runtime route. Declarative parsing/code generation is reserved
for the composite syntax implementation; until then, components can provide
the same product with ordinary parser-derived types and a manual
`InstructionSet` implementation.

The syntax should eventually support named and tuple products as well:

```rust
instruction {
    Push(V),
    Store { slot: SlotId, value: V },
    Reset,
}
```

## Generated shape

For the GateBeam example, the conceptual expansion is:

```rust
pub mod gate_beam {
    pub struct GateBeam {
        measure_sites: HashMap<AtomId, Vec<[f64; 2]>>,
        local_x_tolerance_um: f64,
        local_y_tolerance_um: f64,
        measure_x_tolerance_um: f64,
        measure_y_tolerance_um: f64,
        cz_pair_radius_um: f64,
    }

    pub mod instruction {
        pub struct TopHatCZ;
        pub struct GlobalRZ;
        pub struct GlobalR;
        pub struct LocalRZ;
        pub struct LocalR;
        pub struct DefineMeasureSites;
        pub struct Measure;
        pub struct Reset;
    }
}
```

Component state fields remain private by default. Runtime product fields must
be public when generated composite code constructs products directly:

```rust
pub struct Push<V> {
    pub value: V,
}
```

The namespace module and instruction products should be public when the
component is intended for use by composites in other crates. User-supplied
visibility should be preserved where the declaration permits it.

## Execution association

Execution is always provided explicitly by the component author, per product,
not by `component!` and not through a component-wide enum:

```rust
impl Execute<gate_beam::instruction::Measure> for gate_beam::GateBeam {
    type Message = MeasureMessage;
    type Effect = GateEvent;
    type Fault = GateBeamFault;

    // execute implementation
}
```

Different instructions may therefore have different message, effect, and fault
types. `component!` does not inspect, generate, or validate these implementations.

## Composite boundary

The composite owns the machine-specific instruction set:

```text
component products
    -> selected surface instruction sum and parser
    -> module-wide surface resolution
    -> selected runtime instruction sum
    -> route-specific message resolution
    -> component execution and effect handling
```

For example, the composite may expose only `Measure` and `Reset` from the
GateBeam catalog, assign source patterns such as
`gatebeam::measure`, and lower them into the corresponding runtime products.
The component declaration does not need to know that those products were
selected, renamed, or reached through source-level sugar.

## Design constraints from complex components

The implementation must account for these cases:

1. A component has instructions with different messages, effects, and faults.
2. One runtime product is executed by multiple component types.
3. A composite exposes only a subset of a component's products.
4. Generic and const-generic component/product types are used.
5. Products have unit, tuple, or named payloads.
6. One instruction emits zero, one, or many homogeneous effects.
7. Heterogeneous effects use an explicit effect sum chosen by the author.
8. An instruction parks and later resumes through an owned continuation.
9. A surface instruction expands into several runtime instructions.
10. Large products may make a grouped enum expensive; no implicit boxing should
    be introduced.
11. Generated module names may collide with existing user modules.
12. Product names may collide after normalization or with Rust keywords.

Borrowed runtime products and GAT-based execution are not part of the initial
design. Supporting them would require a runtime-boundary redesign because
parked execution and persistent modules need owned values.

## Grouped enums

The component macro should not require a grouped enum such as:

```rust
pub enum Instruction<V> {
    Push(instruction::Push<V>),
    Pop(instruction::Pop),
}
```

If a grouped enum is useful as an optional catalog or storage representation,
it must not become the `Execute<I>` boundary and must not impose common message,
effect, or fault types. The composite-generated runtime sum is the normal place
for machine-local grouping.

## Naming and collision policy

The default namespace is the snake-case form of the component type, such as
`GateBeam` -> `gate_beam`. The implementation should eventually support an
explicit override, for example:

```rust
component! {
    #[module = gatebeam]
    component GateBeam {
        // ...
    }
}
```

The macro should reject collisions rather than silently overwrite or merge
user modules. It should also reject duplicate instruction names and generated
identifier collisions.

## Implementation phases

1. Parse the component declaration, state fields, instruction products, and
   generic parameters.
2. Validate names, duplicate products, visibility, and supported field forms.
3. Generate the public component namespace and runtime product structs.
4. Add compile-fail coverage for malformed declarations and name collisions.
5. Add generic, const-generic, named-field, tuple, and unit-product tests.
6. Integrate the generated products with composite route selection and runtime
   instruction sums.
7. Add documentation examples for a simple stack, GateBeam-like operations,
   and a component with per-instruction message/effect types.
