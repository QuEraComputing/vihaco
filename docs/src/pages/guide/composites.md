---
layout: ../../layouts/Guide.astro
title: '`composite!` Language Reference'
slug: composites
description: "The complete language reference for the vihaco composite! declaration macro."
---

# `composite!` Language Reference

This document is the normative reference for the `composite!` declaration
macro. It describes the accepted token grammar, the meaning of each clause,
the generated Rust items, and the trait contracts required of the types named
by a declaration. It assumes familiarity with Rust, procedural macros, and the
runtime traits in `vihaco`.

The macro is re-exported by both `vihaco` and `vihaco-runtime` (when the
runtime crate's `derive` feature is enabled). It is invoked as a function-like
macro:

```text
composite! { composite-declaration }
```

The declaration owns a machine's composition policy. Components remain
ordinary Rust values. A composite chooses which component receives each
runtime instruction, where its message comes from, which observers see its
effects, and which handler consumes those effects.

## 1. Complete grammar

The following is a notation for the macro grammar. `Ident`, `Type`, `Expr`,
`String`, `Integer`, and `Attribute` have their Rust meanings. `ε` means that
the production is optional. Whitespace and Rust comments may occur wherever
Rust permits them.

```text
Composite      ::= OuterAttribute* Visibility? `composite` Ident Generics?
                   WhereClause? `{` ErrorClause? Field* `}`
                   SyntaxBlock? RuntimeBlock?

ErrorClause    ::= `error` `=` Type `;`

Field          ::= Attribute* Visibility? Ident `:` Type

SyntaxBlock    ::= `syntax` `{` HeaderClause? SyntaxEntry* `}`
HeaderClause   ::= `header` Type `=>` Ident `;`
SyntaxEntry    ::= `#[pattern = String]` Ident Payload? `=>`
                   (`runtime` Ident | Ident) `;`
Payload        ::= `(` Type `)`

RuntimeBlock   ::= `runtime` `{` Route* `}`
Route          ::= Ident `(` Type `)` `=>` Ident `{` MessageClause EffectsBlock? `}`
MessageClause  ::= `message` (`none` | `from` Ident | `with` Ident) `;`
EffectsBlock   ::= `effects` `{` EffectDeclaration* `}`
EffectDeclaration
               ::= ObserveDeclaration
                |  `absorb` `with` Ident `;`
                |  `handle` `with` Ident `;`
ObserveDeclaration
               ::= `observe` Observer (` ,` Observer)* (`;` | ε)
Observer       ::= Ident (`{` EffectDeclaration* `}`)?
```

The displayed grammar uses a comma with optional surrounding whitespace in
`Observer`; the actual token is simply `,`. Fields are parsed using Rust's
named-field grammar and are comma-terminated, so a trailing comma is allowed.
The `runtime` block may contain routes separated by whitespace or commas; a
trailing comma is allowed. An `effects` block may contain multiple observer
declarations and at most one handler.

The grammar is deliberately narrower than Rust in a few places. In
particular, a field must be named, `error` must be the first item in the
struct body when present, every route must have exactly one message clause,
and `#[pattern = ...]` is the only accepted attribute on a syntax entry.

## 2. Declaration and generated items

The canonical shape is:

```rust ignore
composite! {
    #[derive(Default)]
    pub composite Machine<T>
    where
        T: Default,
    {
        error = MachineError;

        #[device(0x01, alias = "cpu")]
        cpu: Cpu<T>,
        stack: Stack,
    }

    runtime {
        Run(RunInstruction<T>) => cpu {
            message from stack;
            effects {
                absorb with stack;
            }
        }
    }
}
```

The macro emits, in the scope of the invocation:

* the declared struct, with the supplied visibility, outer attributes, fields,
  generics, and where-clause;
* `<Name>Instruction`, if the runtime block is non-empty;
* a private route-marker module and one private marker type per route;
* an optional snake-case module named after the composite, for example
  `machine` for `Machine`;
* `GeneratedMachine` metadata implementation;
* generated SST child-loading support for `#[loadable]` fields; and
* inherent methods for dispatch, surface lowering, and program loading when
  the corresponding declarations are present.

The macro removes the declaration-only field attributes `#[device]`,
`#[loadable]`, `#[program]`, and `#[syntax]` before emitting the struct. Other
field attributes are preserved and are interpreted by Rust normally. The
macro also removes a top-level `#[vihaco(...)]` crate override after using it
to resolve generated paths.

### 2.1 Composite visibility, attributes, and generics

`Visibility` applies to the generated struct. Outer attributes apply to the
struct; this is the normal place for `#[derive(...)]`, `#[allow(...)]`, and
documentation attributes. The declaration accepts Rust type, lifetime, and
const generics and an optional where-clause. Generated enums and helper items
retain only the generic parameters that occur in their payloads or syntax
payloads. The struct and its trait implementations retain the declaration's
generics.

The optional crate override has the form:

```rust ignore
#[vihaco(crate = ::my_framework)]
composite Machine { /* ... */ }
```

It is useful when the facade is renamed or generated code must use a specific
runtime path.

### 2.2 `error = Type`

`error = Type;` declares the error type used by executable composites. It is
required when at least one runtime route exists and forbidden only by
omission—not by an explicit rule—when a structural composite has no routes.
All failures at the generated execution boundary are converted with
`Into<error type>`:

* message supply and message resolver failures;
* target `Execute` failures;
* observer failures;
* `Absorb` failures; and
* `handle with` method failures.

The generated execution method returns `Result<Execution, ErrorType>`. The
macro does not require a particular error library; `eyre::Report` is common in
the workspace.

### 2.3 Fields

Fields are ordinary named Rust struct fields. Their types are used directly;
the macro does not wrap, clone, or otherwise transform them. A route target,
message source, observer, or absorb destination refers to a field by its
identifier.

The following field attributes are consumed by the macro.

#### `#[device(code, alias = "name", ...)]`

Marks a field as a machine device. `code` is a decimal Rust integer literal
that must fit in `u8`. The only supported optional argument is one or more
`alias = "..."` arguments:

```rust ignore
#[device(0x01, alias = "cpu", alias = "host")]
cpu: Cpu,
```

The implementation accepts integer literals such as `1` and `0x01` that
`syn` can parse as `u8`. Device codes must be unique. A device field's Rust
identifier and each alias become source symbols in composite metadata; all
source-symbol names must be unique across devices and aliases.

Device metadata is independent of runtime routes. A device may be declared
without a route, and a route target need not be marked as a device.

#### `#[syntax]` and `#[syntax = "namespace"]`

Marks a device/component field as a mounted component syntax namespace. The
attribute without arguments uses the field name as the namespace. A name-value
form supplies one namespace, and a list supplies one or more aliases:

```rust ignore
#[syntax]
left: Cpu,
#[syntax("right", "secondary")]
right: Cpu,
```

Every namespace must be a valid Rust identifier and must be unique. The field
type must implement `InstructionSet`; its associated `Instruction`, `Value`,
and `Type` become variants in the generated surface syntax enums. A mounted
field need not have a runtime route, but component syntax lowering methods are
generated only when runtime routes exist.

#### `#[loadable]` and `#[loadable = "section/name"]`

Marks a device field as a direct child of the composite's SST section. Bare
`#[loadable]` uses the field identifier as the section name. The name-value
form supplies the section's local name:

```rust ignore
#[device(0x01)]
#[loadable = "cpu-a"]
cpu: Cpu,
```

The name must be non-empty and must not contain `/`. A loadable field must also
be a device. Names must be unique. The generated loader requires the field
type to implement `LoadSstSubtree<Context>`.

#### `#[program]`

Marks the one field that owns the composite's program module. At most one
field may be marked. The field type participates in generated `resolve_parsed`,
`load_parsed`, and `load_source` methods and must implement the relevant
`BuildProgramModule` and `InstallProgramModule<Context>` contracts. The
standard type is `ProgramImage<Instruction, Context, Value, Type, Info>`.

`#[program]` does not imply `#[device]` or `#[loadable]`; program ownership and
device/SST-tree ownership are separate concerns.

## 3. Runtime routes

The runtime block declares the machine-local instruction set and execution
dispatch. Each route has this form:

```text
VariantName(PayloadType) => target_field {
    message ...;
    effects { ... }
}
```

`VariantName` must be unique in the runtime block. `PayloadType` is passed
unchanged to `Execute<PayloadType>` and becomes the payload of the generated
instruction enum variant. The target field must exist. There is no implicit
conversion between payload types and no component-wide instruction enum
inferred by the macro.

### 3.1 Generated runtime instruction enum

For routes `Add(AddInstruction) => alu` and `Reset(ResetInstruction) => alu`,
the macro emits the public enum:

```rust ignore
pub enum MachineInstruction {
    Add(AddInstruction),
    Reset(ResetInstruction),
}
```

Construct instructions as ordinary Rust values:

```rust ignore
let instruction = MachineInstruction::Add(AddInstruction { /* ... */ });
let execution = machine.execute_generated(&instruction)?;
```

The enum derives `Clone`, but not `Debug`, `PartialEq`, or encoding traits. Its
generic parameters are limited to those used by route payloads. The enum is
public even though dispatch internals are private, so a runtime root, resolver,
or program container can construct it.

### 3.2 Message clauses

Every route requires exactly one message clause.

#### `message none;`

The target's associated message type must be `NoMessage` (or otherwise satisfy
the exact type required by the `Execute` implementation), and the generated
call passes `NoMessage` without reading another field.

#### `message from field;`

The target's associated message type is inferred as
`<Target as Execute<Payload>>::Message`. The source field must implement:

```rust ignore
Supply<Message>
```

The generated dispatch calls `Supply::supply(&mut self.field)` and owns the
returned message before invoking the target. This is important for parked
execution: the target cannot retain a borrow into the composite through the
message path.

#### `message with method;`

The composite must implement the generated message-resolver trait method. The
method receives a shared reference to the route payload and returns the target
message:

```rust ignore
impl MachineMessageResolver for Machine {
    fn resolve_add(
        &mut self,
        instruction: &AddInstruction,
    ) -> Result<<Alu as Execute<AddInstruction>>::Message, MachineError> {
        // Read composite state and construct the owned message.
        todo!()
    }
}
```

The trait is also available as `machine::runtime::MessageResolver`; the facade
re-exports it as `MachineMessageResolver` when routes exist. The method name is
not checked until normal Rust trait resolution, so a missing implementation is
a compiler error.

### 3.3 Effects and handlers

An `effects` block is optional. An effect-producing route normally declares
one handler and may declare observers:

```text
effects {
    observe trace, metrics;
    absorb with stack;
}
```

There may be at most one handler per route. The alternatives are exclusive.

#### `absorb with field;`

The destination field must implement `Absorb<E>`, where:

```text
E = <Target as Execute<Payload>>::Effect
```

For each effect returned by the target, the generated handler invokes
`Absorb::absorb(&mut self.field, effect)`. The effect is moved into the
destination exactly once.

#### `handle with method;`

The composite must provide an inherent method with the owned effect as its
only argument. Its return error must convert into the composite error:

```rust ignore
impl Machine {
    fn handle_output(&mut self, effect: Output) -> Result<(), MachineError> {
        // Composite-owned policy.
        Ok(())
    }
}
```

The generated call does not pass the route marker or instruction. Those are
implementation details of dispatch.

#### Routes without handlers

An effects block may be omitted, or may contain observers without a terminal
handler. A terminal generated observation path must produce `NoEffect`; if no
observer exists, the route's `Execute::Effect` is type-checked directly as
`NoEffect`. This prevents an effect stream from being silently discarded.

### 3.4 Observers

An observer is a named field implementing:

```rust ignore
impl Observe<InputEffect, RouteMarker> for Observer {
    type Effect = ObserverEffect;
    type Error = ObserverError;

    fn observe(
        &mut self,
        effect: &InputEffect,
    ) -> Result<Effects<ObserverEffect>, Self::Error> {
        todo!()
    }
}
```

The route marker is private and unique to the route. The public way to select
an observer behavior is therefore to implement `Observe` for the generated
marker through the route's expansion; users name the observer field only in
the declaration.

Observers run in declaration order. They borrow the incoming effect; they do
not consume or clone it. Each observer's returned effects may be handled by a
nested observer tree:

```text
effects {
    observe trace {
        observe trace_sink;
        absorb with log;
    }
    absorb with stack;
}
```

The outer observer sees the target effect. The nested observer sees each
effect emitted by the outer observer. A nested terminal observer with no
handler must emit `NoEffect`; a nested `absorb` or `handle` consumes each
emitted effect. Observer failures and nested-handler failures are normalized
into the composite error.

The same field cannot appear twice at the same observer-tree level. The
implementation permits the same field at different nesting levels.

## 4. Generated execution behavior

For every route, the macro generates an inherent method with the effective
signature:

```rust ignore
fn execute_generated(
    &mut self,
    instruction: &MachineInstruction,
) -> Result<Execution, MachineError>
```

It is private Rust visibility. Code in the module containing the macro
invocation can call it directly; an external public API should expose its own
wrapper if it needs to execute a composite from another module.

The route algorithm is, conceptually:

```text
match instruction {
    Route(payload) => {
        message = resolve the route's message;
        result = target.execute(&mut target, payload, message)?;
        for effect in result.effects {
            run observers in declaration order;
            pass effect to the route handler, if any;
        }
        return result.execution;
    }
}
```

The target is borrowed mutably only for its `Execute` call. Effects are then
processed in the returned `Effects` stream. `Execution` is returned unchanged;
the macro does not fetch instructions, advance a program counter, schedule
events, or implement resume/continuation policy.

The runtime contracts involved are:

```rust ignore
trait Execute<I> {
    type Message;
    type Effect;
    type Fault;
    fn execute(&mut self, instruction: &I, message: Self::Message)
        -> Result<StepResult<Self::Effect>, Self::Fault>;
}
```

The remaining contracts are `Supply<M>`, `Observe<E, R>`, `Absorb<E>`, and
`Handle<E, R>`. The macro generates private `Handle<E, RouteMarker>`
implementations for route handlers; authors normally interact with handlers
through the declaration rather than naming those marker types.

## 5. Surface syntax block

The `syntax` block is optional. It adds a composite-owned source-language
layer. It can coexist with mounted component syntax from `#[syntax]` fields.

### 5.1 Composite-owned entries

An entry has a Chumsky/parser pattern, a public surface variant, an optional
payload, and either a direct runtime mapping or a named lowerer:

```rust ignore
syntax {
    #[pattern = "'machine::halt"]
    Halt => runtime Halt;

    #[pattern = "'machine::load $0"]
    Load(u64) => lower_load;
}
```

The pattern is a string literal consumed by `#[derive(Parse)]` machinery.
Patterns must be unique. Surface variant names must be unique within the
composite syntax enum.

`=> runtime Route` is allowed only for a unit surface variant. `Route` must be
the name of an existing runtime route. The generated lowering returns a
single runtime instruction containing the route's payload type; therefore a
unit direct mapping is appropriate only when the runtime payload can be
constructed as a unit value.

`=> lowerer` requires a payload. The composite must implement the generated
resolver method:

```rust ignore
impl machine::syntax::Resolver for Machine {
    fn lower_load(
        &mut self,
        value: u64,
    ) -> Result<Vec<machine::runtime::Instruction>, MachineError> {
        Ok(vec![machine::runtime::Instruction::Load(
            LoadInstruction(value),
        )])
    }
}
```

The lowerer may return zero, one, or many runtime instructions. Its error is
converted into the composite error.

### 5.2 Header declaration

```text
syntax {
    header HeaderType => resolve_header;
    ...
}
```

The generated `syntax::Header` is an alias for `HeaderType`, and the generated
resolver trait requires `resolve_header(HeaderType) -> Result<(), Error>`.
When no header clause is present, `syntax::Header` is a generated unit-like
header that implements `FromText` and `SstHeader`. A public
`syntax::parse_header(section)` helper is generated only when a header clause
is present.

### 5.3 Mounted component syntax

For each field marked `#[syntax(...)]`, the generated `machine::syntax`
module contains:

```text
Instruction::FieldName(ComponentInstruction)
Value::FieldName(ComponentValue)
Type::FieldName(ComponentType)
```

`FieldName` is the field identifier converted to PascalCase. The parser accepts
each namespace and alias as a prefix, such as `left::step 7`. The component
type must implement `InstructionSet`.

When the composite has runtime routes, the generated resolver trait also
contains `lower_<field>` for each mounted field. Implementing it lowers the
component instruction into `Vec<machine::runtime::Instruction>`.

### 5.4 Generated syntax API

If a syntax block, syntax mount, or header is present, the macro generates the
snake-case module named after the composite. Its important public items are:

* `syntax::Instruction`, implementing `Parse` and `SurfaceInstruction`;
* `syntax::Value`, implementing `Parse`;
* `syntax::Type`, implementing `Parse`;
* `syntax::Header`;
* `syntax::Module`, implementing `ModuleSyntax`; and
* `syntax::Resolver`, the trait implemented by the composite author.

The module's `runtime::Instruction` is an alias of `<Name>Instruction` when
runtime routes exist. At the parent scope, the macro re-exports
`syntax::Instruction` as `SurfaceInstruction`, `syntax::Resolver` as
`<Name>SyntaxResolver`, and the runtime message resolver as
`<Name>MessageResolver` where applicable.

## 6. Program construction and SST loading

Program loading is generated only when all of the following are true:

1. the composite is executable (`error` and routes are present);
2. it has surface syntax (a syntax entry or syntax mount); and
3. one field is marked `#[program]`.

The program field's type controls storage through `BuildProgramModule`. The
standard `ProgramImage` implementation stores a `LocalModule`, context, and
program counter, but the macro does not require that representation.

### 6.1 `resolve_parsed`

The generated method has the effective shape:

```text
resolve_parsed(
    &mut self,
    ParsedModule<Machine::syntax::Module>,
) -> Result<ProgramField::Module>
```

It resolves an optional header, creates an empty module, interns strings,
copies constants and source symbols, lowers every function instruction, records
function and label metadata, selects a function named `main`, and calls
`BuildProgramModule::finish`. Lowering diagnostics identify the function,
instruction index, and source instruction.

The surface syntax type must convert into the program builder's associated
`Type` type. The program builder's associated instruction type must be the
generated runtime instruction enum.

### 6.2 `load_parsed`

`load_parsed(parsed, context)` calls `resolve_parsed` and installs the resulting
module through `InstallProgramModule<Context>`. A successful standard
`ProgramImage` installation replaces its module and context and resets `pc` to
zero. Installation is delegated to the program container, so custom
containers define their own atomicity and storage policy.

### 6.3 `load_source`

`load_source(section)` parses an SST section using the generated syntax module,
resolves it, validates and forwards direct loadable child sections, and
installs the resulting module using the section's context handle. Every
`#[loadable]` field must have a matching child section; duplicate, unexpected,
root-level, or missing child names are errors.

The composite itself must implement `LoadSstProgram<Context>` for the
composite section. This hook is invoked before generated child forwarding. A
structural or executable composite can therefore keep composite-owned section
behavior explicit in ordinary Rust.

For composites with loadable children, the macro also generates:

```text
load_generated_sst_children(section) -> Result<()>
```

and a `LoadSstSubtree<Context>` implementation for the composite. The latter
loads the composite program hook and is what lets a parent composite forward a
child subtree to it.

## 7. Metadata and structural composites

`GeneratedMachine` is implemented for every expansion. Its `metadata()` method
returns `CompositeMetadata` containing static slices of:

* `DeviceMetadata { code, name }` for every `#[device]` field; and
* `SourceSymbolAliasMetadata { name, device_code }` for every device alias.

The `CompositeMetadata` helpers support device lookup, source-symbol-to-device
resolution, and validation of module source symbols. Device field names are
available through `device_by_name` and are also valid source symbols.

A declaration with no `runtime` block is a structural composite:

```rust ignore
composite! {
    pub composite Fabric {
        clock: GlobalClock,
        #[device(0x01, alias = "cpu-a")]
        cpu_a: Cpu,
        #[device(0x02, alias = "cpu-b")]
        cpu_b: Cpu,
    }
}
```

It generates the struct, device metadata, and any generated section wiring,
but no runtime instruction enum, route dispatch, message-resolver trait, or
`execute_generated`. Scheduling, child selection, timing, continuation, and
deadlock policy remain ordinary Rust.

## 8. Validation and errors

Expansion-time validation rejects:

* non-named fields;
* duplicate device codes, aliases, source symbols, or loadable names;
* a loadable field without a device;
* invalid loadable names;
* multiple `#[program]` fields;
* duplicate syntax variants or patterns;
* invalid or duplicate syntax namespaces;
* direct syntax mappings with payloads;
* unknown direct runtime routes;
* named syntax lowerers without payloads;
* duplicate runtime route variants;
* unknown route targets or message-source fields;
* duplicate message clauses or effects blocks;
* missing message clauses;
* duplicate observers at one tree level;
* unknown observer or absorb fields; and
* duplicate route handlers.

Rust type checking then enforces the semantic contracts: `Execute` on every
target/payload pair, `Supply` for `message from`, resolver methods for
`message with`, `Observe` for every observer, `Absorb` for absorb handlers,
the signatures of composite-owned methods, program-builder/installer bounds,
and all required `Into<Error>` conversions.

## 9. Public integration boundary

The stable author-facing integration points are the declared struct, the
generated `<Name>Instruction`, the generated snake-case syntax/runtime modules,
the resolver traits, `CompositeMetadata`, and the generated program-loading
methods when enabled. Route marker types and the `Handle` implementations are
private implementation details.

The macro intentionally does not define a universal machine execution trait.
A runtime root generally owns the fetch/step loop and calls
`execute_generated`, then interprets `Execution` and applies its own program,
clock, or scheduling policy. This separation allows the same composite
declaration to be embedded in different host runtimes.

For the component-side contracts, see [Building Components](/guide/components).
For message and effect semantics, see [Using Messages](/guide/messages) and
[Observing Effects](/guide/observers). The parser-specific examples are in
[Advanced Parser Integration](/guide/parser-advanced).
