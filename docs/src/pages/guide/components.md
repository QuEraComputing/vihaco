---
layout: ../../layouts/Guide.astro
title: '`component!` Language Reference'
slug: components
description: "The complete language reference for the vihaco component! declaration macro."
---

# `component!` Language Reference

This document is the normative reference for the `component!` declaration
macro. It specifies the accepted declaration grammar, generated Rust items,
runtime instruction products, optional component syntax, visibility rules,
generic behavior, and the boundaries that remain ordinary Rust.

`component!` declares a reusable component type and the product types that may
be passed to its runtime implementation. It does not define a machine-wide
instruction set. A `composite!` declaration chooses which products a machine
exposes and how messages and effects are routed.

The macro is re-exported by `vihaco` and by `vihaco-runtime` when its `derive`
feature is enabled.

## 1. Complete grammar

The following notation describes the macro input. `Ident`, `Type`, `Expr`,
`String`, `Generics`, `WhereClause`, `Visibility`, and `Attribute` have their
Rust meanings. `ε` denotes an optional production. Whitespace, comments, and
trailing commas are accepted where the grammar permits them.

```text
Component       ::= OuterAttribute* `#[module = Ident]`? Visibility?
                    `component` Ident Generics? WhereClause?
                    `{` NamedField* `}`
                    RuntimeBlock? SyntaxBlock?

NamedField      ::= Attribute* Visibility? Ident `:` Type `,`?

RuntimeBlock     ::= `runtime` `{` RuntimeAlias* InstructionBlock? `}`
RuntimeAlias    ::= Visibility? (`type` | `value`) Ident `=` Type `;`
InstructionBlock
                ::= `instruction` `{` Product* `}`
Product         ::= OuterAttribute* Visibility? Ident ProductFields? `,`?
ProductFields   ::= `(` TupleField* `)` | `{` NamedProductField* `}`
TupleField      ::= Attribute* Visibility? Type `,`?
NamedProductField
                ::= Attribute* Visibility? Ident `:` Type `,`?

SyntaxBlock     ::= `syntax` `{` TypeBlock ValueBlock InstructionSyntaxBlock `}`
TypeBlock       ::= `type` Ident `{` SyntaxEnumVariant* `}`
ValueBlock      ::= `value` Ident `{` SyntaxEnumVariant* `}`
SyntaxEnumVariant
                ::= Ident `=` String (`;` | `,` | ε)
InstructionSyntaxBlock
                ::= `instruction` `{` SyntaxInstructionVariant* `}`
SyntaxInstructionVariant
                ::= Ident SyntaxPayload? `=` String (`;` | `,` | ε)
SyntaxPayload   ::= `(` TypeList `)`
TypeList        ::= Type (` ,` Type)* `,`?
```

The displayed comma in `TypeList` is the ordinary `,` token; spacing in the
notation is illustrative. The component state block contains named Rust
fields. Runtime products may be unit-like, tuple-like, or named-field structs.
The syntax block requires exactly one `type`, one `value`, and one
`instruction` declaration; their order within `syntax` is not significant.

## 2. Component declaration

The canonical declaration is:

```rust ignore
component! {
    #[derive(Default)]
    pub component Counter<T>
    where
        T: Default,
    {
        value: T,
    }

    runtime {
        instruction {
            Add(T),
            Reset,
        }
    }
}
```

The macro generates a module, and places the component struct and product
types inside that module. With the default naming rules, the declaration above
produces `counter::Counter`, `counter::runtime::instruction::Add<T>`, and
`counter::runtime::instruction::Reset`.

### 2.1 Component visibility

The component's `Visibility` applies to the generated module, the component
struct, the `runtime` and `instruction` modules, and the optional `syntax` module. If no
visibility is written, these generated items are `pub`.

```rust ignore
component! {
    pub(crate) component InternalCounter { value: i64, }
}
```

The generated module is the public API namespace. The macro does not emit a
component type directly at the invocation site.

### 2.2 Attributes

The only supported outer attribute on the component declaration is:

```text
#[module = Ident]
```

It overrides the default generated module name. The value must be a single
Rust identifier:

```rust ignore
#[module = device_cpu]
component Cpu { }
```

This produces `device_cpu::Cpu` instead of the default `cpu::Cpu`. The
attribute is consumed by the macro and is not emitted. Any other outer
attribute on the component declaration is rejected.

Attributes on instruction products are preserved and emitted on the generated
product struct. They may include `#[derive(...)]`, `#[doc = ...]`, and other
attributes accepted by Rust for structs.

Attributes on state fields and product fields are parsed as ordinary Rust
field attributes and are preserved. The macro does not assign special meaning
to them.

### 2.3 State fields

The first braced block is the component's state. It must contain named fields:

```rust ignore
component! {
    component RegisterFile {
        values: Vec<i64>,
        #[allow(dead_code)]
        capacity: usize,
    }
}
```

The generated state fields have a deliberate visibility split. A field with
no explicit visibility is emitted as `pub(super)`, allowing implementations
written in the module containing the macro invocation to access it while not
making it public to all downstream users. An explicit field visibility is
preserved.

The component struct itself receives the declaration's generics and where
clause. State field types are resolved in the lexical scope containing the
macro invocation, so `super::Type`, local aliases, and parent-module names are
valid.

### 2.4 Empty components

The state block may be empty. The `runtime` block may be omitted when the
component has no products:

```rust ignore
component! {
    component Clock {}
}
```

This still generates `clock::Clock`. No `clock::runtime::instruction` module is
generated unless a runtime instruction block is present. An empty runtime
`instruction {}` block generates the instruction module with no product structs.

## 3. Runtime instruction products

The optional `runtime { instruction { ... } }` block is a catalog of independent runtime product
types. It is not an enum and does not impose common message, effect, or fault
types.

```rust ignore
component! {
    component RegisterFile {
        values: Vec<i64>,
    }

    runtime {
        instruction {
            Read { slot: usize },
            Write(usize, i64),
            Reset,
        }
    }
}
```

The generated items are equivalent to:

```rust ignore
pub mod register_file {
    pub struct RegisterFile {
        pub(super) values: Vec<i64>,
    }

    pub mod runtime {
        pub mod instruction {
            pub struct Read {
                pub slot: usize,
            }
            pub struct Write(pub usize, pub i64);
            pub struct Reset;
        }
    }
}
```

The conceptual expansion omits generic parameters and user attributes for
brevity.

### 3.1 Unit products

```text
Reset,
```

generates a unit-like struct. Construct it as
`register_file::runtime::instruction::Reset`.

### 3.2 Tuple products

```text
Write(usize, i64),
```

generates a tuple struct. Tuple fields without explicit visibility are made
`pub`, because composite-generated code and downstream users must be able to
construct products:

```rust ignore
let instruction = register_file::runtime::instruction::Write(3, 42);
```

Tuple field attributes and explicit visibilities are preserved.

### 3.3 Named products

```text
Read { slot: usize },
```

generates a named-field struct. Named fields without explicit visibility are
made `pub` for the same construction boundary:

```rust ignore
let instruction = register_file::runtime::instruction::Read { slot: 3 };
```

Use explicit visibility when a product field needs a different Rust visibility.

### 3.4 Product attributes and visibility

The product declaration accepts outer attributes and an optional visibility:

```rust ignore
runtime {
    instruction {
        #[derive(Clone, Debug, PartialEq)]
        pub Add(i64),
        #[doc = "Stops the device."]
        Reset,
    }
}
```

If a product has no visibility, it is emitted as `pub`. If it has an explicit
visibility, that visibility is preserved. The product's generic parameters
are filtered from the enclosing component generics to retain only parameters
used by the product's fields and its relevant where predicates.

### 3.5 Supported field forms

Products support all three Rust struct forms:

```rust ignore
runtime {
    instruction {
        Unit,
        Tuple(T, const_value_type),
        Named { value: T, index: usize },
    }
}
```

The macro does not generate constructors beyond Rust's normal unit, tuple, and
named struct constructors. It does not box, wrap, or convert product fields.

## 4. Implementing runtime behavior

`component!` declares product types but does not implement their behavior.
Implement `Execute<I>` separately for each product and component type:

```rust ignore
impl Execute<counter::runtime::instruction::Add> for counter::Counter {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &counter::runtime::instruction::Add,
        _message: NoMessage,
    ) -> Result<StepResult<NoEffect>, Self::Fault> {
        self.value += instruction.0;
        Ok(StepResult {
            effects: Effects::none(),
            execution: Execution::Complete,
        })
    }
}
```

The runtime contract is:

```rust ignore
trait Execute<I> {
    type Message;
    type Effect;
    type Fault;

    fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault>;
}
```

Each product may have a different `Message`, `Effect`, and `Fault`. The
component macro does not inspect, generate, or validate these implementations.
Rust trait resolution reports missing or incompatible implementations when a
composite or caller uses the product.

### 4.1 Messages

Messages are owned values supplied to `Execute`. Use `NoMessage` for an
operation with no input, or define a component-specific message type. The
`Message` trait is a marker for message types that need the framework's message
marker semantics; it is not required for every type used as
`Execute::Message`.

Message acquisition is a composite concern. A composite route may pass
`NoMessage`, call `Supply<M>` on another field, or use a composite-owned
resolver method.

### 4.2 Effects

`Execute::Effect` is the homogeneous item type carried by `Effects<E>` in the
returned `StepResult`. `Effects::none()`, `Effects::one(value)`, and a many
effect stream express zero, one, or many outputs. The component chooses the
effect type; the composite chooses how to observe and consume it.

For an operation that must not produce effects, use `NoEffect` and return an
empty effect stream. This lets generated composite routing type-check that no
output is silently discarded.

### 4.3 Execution state

`StepResult` contains both the effect stream and an `Execution` state:

* `Execution::Complete` indicates that the operation completed from the
  component's perspective.
* `Execution::Parked` indicates that the parent runtime must retain or
  otherwise coordinate the operation before continuing.

The component macro does not generate a program counter, event loop, timing
policy, or resume/continuation dispatch. Those remain the responsibility of
the runtime root or composite host.

## 5. Component capabilities

The component struct can implement capabilities independently of its products.
These implementations are ordinary Rust and are not generated:

```text
Supply<M>          produces an owned message
Absorb<E>          consumes an owned effect
Observe<E, R>      borrows an effect and may emit observation effects
Handle<E, R>       consumes an effect for a selected route
```

`component!` therefore remains reusable across machines. A composite can use
one component as a target, message source, observer, or effect destination
without changing the component declaration.

## 6. Component-local surface syntax

The optional `syntax` block declares a component's source-language instruction,
value, and type vocabulary. It is independent of machine routing. A composite
may mount this vocabulary under one or more namespaces with its field
attribute `#[syntax(...)]`.

The canonical form is:

```rust ignore
component! {
    component Arithmetic {}

    runtime {
        instruction {
            Add(ArithmeticType),
        }
    }

    syntax {
        type ArithmeticType {
            Integer = "`integer`";
            Address = "`address`";
        }

        value ArithmeticValue {
            Zero = "`zero`";
        }

        instruction {
            Add(ArithmeticType) = "'add $0";
        }
    }
}
```

The syntax block must contain all three declarations: `type`, `value`, and
`instruction`. Omitting any one is a macro expansion error.

### 6.1 Syntax type and value enums

`type Name { ... }` and `value Name { ... }` each generate a public enum with
the requested name. Each variant has a parser pattern:

```text
type ArithmeticType {
    Integer = "`integer`";
    Address = "`address`";
}
```

is equivalent in shape to:

```rust ignore
#[derive(Clone, Debug, PartialEq, vihaco::Parse)]
pub enum ArithmeticType {
    #[pattern = "`integer`"]
    Integer,
    #[pattern = "`address`"]
    Address,
}
```

Syntax type and value variants are unit variants. Their pattern strings are
passed to the parser derive and are not interpreted as runtime instruction
products.

### 6.2 Syntax instruction enum

The nested syntax `instruction` block generates `syntax::Instruction`. A
variant may be unit-like or carry one or more typed parser fields:

```text
instruction {
    Add(ArithmeticType) = "'add $0";
    Halt = "'halt";
}
```

This generates the shape:

```rust ignore
#[derive(Clone, Debug, PartialEq, vihaco::Parse)]
pub enum Instruction {
    #[pattern = "'add $0"]
    Add(ArithmeticType),
    #[pattern = "'halt"]
    Halt,
}
```

The types in the syntax instruction payload are parser-side types. They are
not automatically converted into runtime product fields and are not required
to be the same type as the component's runtime product payload. A composite or
other resolver performs that lowering.

The pattern string is attached to the generated variant as `#[pattern =
...]`. `$0`, `$1`, and subsequent placeholders refer to the corresponding
payload fields according to the parser derive's pattern rules.

### 6.3 Syntax module and `InstructionSet`

When `syntax` is present, the generated component module contains:

* `syntax::<TypeName>` — the declared type enum;
* `syntax::<ValueName>` — the declared value enum; and
* `syntax::Instruction` — the generated instruction enum.

The component type implements:

```rust ignore
impl InstructionSet for arithmetic::Arithmetic {
    type Instruction = arithmetic::syntax::Instruction;
    type Value = arithmetic::syntax::ArithmeticValue;
    type Type = arithmetic::syntax::ArithmeticType;
}
```

The `InstructionSet` implementation is what allows a composite to mount the
component with `#[syntax]`. It does not assign a namespace; namespaces belong
to the composite's mount.

All generated syntax enums derive `Clone`, `Debug`, and `PartialEq`, and use
the framework's parser derive. Their visibility follows the component's
visibility.

## 7. Generated API summary

For:

```rust ignore
component! {
    pub component Sensor<T> where T: Clone {
        state: T,
    }

    runtime {
        instruction {
            Measure(T),
            Reset,
        }
    }
}
```

the generated public shape is:

```text
sensor::Sensor<T>
sensor::runtime::instruction::Measure<T>
sensor::runtime::instruction::Reset
```

More precisely:

* the module is `sensor` by default, or the identifier supplied by
  `#[module = ...]`;
* the component struct is `<module>::Sensor`;
* products are `<module>::runtime::instruction::<Product>`;
* state fields without explicit visibility are `pub(super)`;
* product fields without explicit visibility are `pub`;
* a syntax block adds `<module>::syntax`, the declared enums, and
  `InstructionSet` for the component; and
* the macro emits no runtime execution method or component-wide instruction
  enum.

The generated module uses `use super::*`, so types in the parent module are
available to generated state, product, and syntax declarations. Product and
component generic parameters are retained according to where they are used;
unused enclosing generics are not copied onto individual products or syntax
types.

## 8. Generic and const-generic components

The component declaration accepts Rust generics and an optional where-clause:

```rust ignore
component! {
    component GenericComponent<T, const N: usize>
    where
        T: Clone,
    {
        value: T,
    }

    runtime {
        instruction {
            Unit,
            Tuple(T),
            Array([T; N]),
        }
    }
}
```

The component is `generic_component::GenericComponent<T, N>`. The products
are:

```text
generic_component::runtime::instruction::Unit
generic_component::runtime::instruction::Tuple<T>
generic_component::runtime::instruction::Array<T, N>
```

`Unit` does not retain `T` or `N`, `Tuple` retains `T`, and `Array` retains
both. Relevant where predicates are retained with the parameters they
constrain. This allows a product to be used independently of unrelated state
or component parameters.

Generic types used only in state remain on the component struct but do not
appear on products that do not reference them.

## 9. Naming and validation

The macro validates generated names during expansion. It rejects:

* unsupported component-level attributes;
* invalid generated module names;
* duplicate `#[module]` attributes;
* duplicate product names after snake-case normalization;
* syntax blocks missing `type`, `value`, or `instruction`;
* malformed state or product field declarations; and
* trailing tokens after the final declaration block.

Product name collision checking is performed after removing a raw-identifier
prefix and converting the name to snake case. For example, two product names
that normalize to the same generated name are rejected. Rust then performs the
remaining semantic checks, including duplicate fields, invalid visibility,
generic bounds, and derive errors from product attributes.

The macro does not validate whether a product has an `Execute` implementation,
whether a message/effect type is suitable, or whether a syntax pattern lowers
to a runtime product. Those are intentionally separate component and composite
contracts.

## 10. What `component!` does not generate

The following remain author- or composite-defined Rust:

* `Execute<I>` implementations;
* a component-wide instruction enum;
* source namespaces and machine-wide syntax selection;
* runtime route selection;
* message supply and message resolution;
* effect observation and consumption policy;
* opcodes, widths, bytecode encoders, and decoders;
* program counters and instruction fetching;
* scheduling, timing, parking, and resume policy; and
* a universal machine execution trait.

The machine-local encoded instruction enum is normally generated by
`composite!`. If a custom machine representation is needed, it may be defined
separately, but it should contain the component products as payloads. The
independent product types generated by `component!` are the normal inputs to
`Execute<I>` and to composite route declarations.

## 11. Integration with `composite!`

A composite selects component products explicitly:

```rust ignore
composite! {
    composite Machine {
        error = MachineError;

        #[syntax("arithmetic")]
        arithmetic: arithmetic::Arithmetic,
    }

    runtime {
        Add(arithmetic::runtime::instruction::Add) => arithmetic {
            message none;
        }
    }
}
```

The component owns `arithmetic::runtime::instruction::Add`, its state, its
`Execute<Add>` implementation, and optionally its local parser vocabulary.
The composite owns the public machine route, source namespace, message policy,
and effect policy. A component product may be selected by multiple composites
or by multiple routes, and one product type may be executed by multiple
component state types when their `Execute<I>` implementations permit it.

For route grammar and generated machine behavior, see the
[`composite!` Language Reference](/guide/composites). For machine-level
encoding and widths, see [Defining Instructions](/guide/instructions).
