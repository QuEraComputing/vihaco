# Component Instruction Sets and Composite Module Syntax

## Purpose

This document refines the composite syntax/runtime plan around a clearer
ownership boundary:

```text
component
    owns instruction syntax, value/type syntax, and runtime instruction products

composite
    owns SST section headers, namespaces, syntax composition, lowering, and routes
```

The source dialect consumed by an SST module is represented by one
`ModuleSyntax` type. Its instruction, value, and source-type parts are generated from
the composite's participating components, while its header part is defined by the
composite that owns the SST section.

Acamar demonstrates why headers remain composite-owned. Its
`AcamarHeaderBlock` is parsed from the Acamar section and then resolved by
`AcamarResolver` into `DeviceInfo`. It is not syntax owned by the CPU, FPGA,
camera, or other child components.

## Component instruction sets

Components may expose an instruction-set syntax product:

```rust
pub trait InstructionSet {
    type Instruction: SurfaceInstruction;
    type Value;
    type Type;
}
```

A component can provide syntax alongside its runtime instruction products:

```rust
pub mod processor {
    pub mod syntax {
        pub enum Value {
            U32(u32),
            Label(LabelRef),
        }

        pub enum Instruction {
            Step(Value),
            Branch(Value),
            Add(Type),
            Reset,
        }

        pub enum Type {
            I64,
            U32,
        }
    }

    pub mod instruction {
        pub struct Step {
            pub duration: u64,
        }

        pub struct Reset;
    }
}
```

The component declaration that produces these syntax types is declarative:

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

The macro uses the shared pattern parser to generate `Parse` implementations;
component authors do not write Chumsky parsers manually.

The component owns the grammar and parser implementations for its instruction
and source-type syntax. It does not own:

- SST section headers;
- device aliases or public namespaces;
- device codes;
- runtime route identity;
- machine-wide metadata;
- composite-specific lowering policy.

Syntax is optional. Runtime-only components remain valid components without an
`InstructionSet` implementation.

## Composite-generated source sums

Given:

```rust
#[device(0x01, alias = "processor")]
processor: Processor,

#[device(0x02, alias = "waveform")]
waveform: Waveform,
```

the composite generates source sums:

```rust
pub enum SurfaceInstruction {
    Processor(processor::syntax::Instruction),
    Waveform(waveform::syntax::Instruction),
}

pub enum SurfaceValue {
    Processor(processor::syntax::Value),
    Waveform(waveform::syntax::Value),
}

pub enum SurfaceType {
    Processor(processor::syntax::Type),
    Waveform(waveform::syntax::Type),
}
```

The generated sum is explicit Rust enum composition, not an implicit union.
The composite must define how duplicate or ambiguous source spellings are
handled. Namespaced type syntax may be required when component type grammars
overlap.

The composite may also contribute shared/core syntax types if the module
language has types that are not owned by one device:

```rust
pub enum SurfaceType {
    Core(CoreType),
    Processor(processor::syntax::Type),
    Waveform(waveform::syntax::Type),
}
```

## Composite-owned headers

Headers are defined by the composite syntax declaration because the composite
owns the SST section:

```rust
syntax {
    header ControlHeaderBlock => resolve_header;
}
```

The composite may wrap device-specific header fragments, but those fragments
remain part of the composite's header grammar:

```rust
pub enum ControlHeader {
    Processor(ProcessorHeader),
    Waveform(WaveformHeader),
    Clock(ClockHeader),
}

pub struct ControlHeaderBlock {
    pub headers: Vec<ControlHeader>,
}
```

The component does not define or parse these headers as part of its
instruction set. A header can configure multiple devices, machine-wide
scheduling, source symbols, or the program's module metadata.

## Namespaces and parsing

Components define local instruction grammar. The composite defines the public
namespace:

```text
processor::step 100
processor::reset
waveform::play 50
```

The generated composite parser delegates the namespaced instruction to the
component parser:

```text
processor::step 100
    -> SurfaceInstruction::Processor(
           processor::syntax::Instruction::Step(...)
       )
```

The same component syntax can be mounted more than once:

```text
cpu_a::step 100
cpu_b::step 100
```

The component does not need to know which alias or device field selected it.

## ModuleSyntax

`ModuleSyntax` describes one complete source dialect:

```rust
pub trait ModuleSyntax {
    type Instruction: SurfaceInstruction;
    type Value;
    type Type;
    type Header: SstHeader;
}
```

For a composite, the associated types have these owners:

```text
ModuleSyntax::Instruction
    generated sum of component surface instructions

ModuleSyntax::Value
    generated sum of component and core source values

ModuleSyntax::Type
    generated sum of component and core source types

ModuleSyntax::Header
    composite-owned parsed section-header syntax
```

The composite generates a marker and implementation:

```rust
pub mod control_machine {
    pub mod syntax {
        pub struct Module;

        pub enum Instruction {
            Processor(processor::syntax::Instruction),
            Waveform(waveform::syntax::Instruction),
        }

        pub enum Type {
            Processor(processor::syntax::Type),
            Waveform(waveform::syntax::Type),
        }

        pub enum Value {
            Processor(processor::syntax::Value),
            Waveform(waveform::syntax::Value),
        }

        impl ::vihaco::ModuleSyntax for Module {
            type Instruction = Instruction;
            type Value = Value;
            type Type = Type;
            type Header = ControlHeaderBlock;
        }
    }
}
```

`ParsedModule` becomes:

```rust
pub struct ParsedModule<S>
where
    S: ModuleSyntax,
{
    pub header: S::Header,
    pub functions: Vec<ParsedFunction<S>>,
}
```

The parser and resolver now operate on:

```rust
ParsedModule<control_machine::syntax::Module>
```

rather than independent instruction, type, and header parameters.

## Semantic analysis and lowering

Parsing produces syntax values and types. The composite performs semantic
analysis during resolution, using the marked `#[program]` field to resolve
labels, types, constants, and other module-wide symbols before constructing
runtime instruction products.

For example:

```text
processor::br @loop
    -> processor::syntax::Instruction::Branch(Value::Label(...))
    -> program.resolve_label(...)
    -> processor::instruction::Branch { target: u32 }
```

This keeps machine-specific policy in the composite:

- route identity;
- one-to-many expansion;
- source sugar;
- device selection;
- scheduling or timing policy;
- conversions requiring composite state.

The component defines what its instruction means locally. The composite
decides which machine route receives it.

An illustrative generated resolver implementation is:

```rust
impl ControlMachineSyntaxResolver for ControlMachine {
    fn lower_processor(
        &mut self,
        instruction: processor::syntax::Instruction,
    ) -> Result<Vec<ControlRuntimeInstruction>, ControlFault> {
        match instruction {
            processor::syntax::Instruction::Step(value) => {
                let value = match value {
                    processor::syntax::Value::U32(value) => value,
                    other => {
                        return Err(ControlFault::type_error(
                            "processor::step expects a u32 value",
                            other,
                        ));
                    }
                };

                Ok(vec![ControlRuntimeInstruction::ProcessorStep(
                    processor::instruction::Step { value },
                )])
            }
            processor::syntax::Instruction::Branch(value) => {
                let label = match value {
                    processor::syntax::Value::Label(label) => label,
                    other => {
                        return Err(ControlFault::type_error(
                            "processor::br expects a label",
                            other,
                        ));
                    }
                };
                let target = self.program.resolve_label(&label)?;

                Ok(vec![ControlRuntimeInstruction::ProcessorBranch(
                    processor::instruction::Branch { target },
                )])
            }
            processor::syntax::Instruction::Add(ty) => {
                let ty = self.program.resolve_type(ty)?;

                Ok(vec![ControlRuntimeInstruction::ProcessorAdd(
                    processor::instruction::Add { ty },
                )])
            }
            processor::syntax::Instruction::Reset => Ok(vec![
                ControlRuntimeInstruction::ProcessorReset(
                    processor::instruction::Reset,
                ),
            ]),
        }
    }
}
```

Broad value operands may produce semantic errors during resolution. Syntax
declarations may instead constrain an operand to a particular value variant
when earlier parser rejection is preferable:

```rust
instruction {
    Step(value: U32) = "'step $value";
    Branch(target: Label) = "'br $target";
}
```

## Header resolution

Parsing a header and resolving a header are separate operations. The generic
resolver consumes the complete parsed module:

```rust
pub trait Resolve<S>
where
    S: ModuleSyntax,
{
    type Module;

    fn resolve_module(
        &mut self,
        parsed: ParsedModule<S>,
    ) -> eyre::Result<Self::Module>;
}
```

Generated composites also expose a header-resolution boundary:

```rust
pub trait ControlMachineSyntaxResolver {
    fn resolve_header(
        &mut self,
        header: ControlHeaderBlock,
    ) -> Result<(), ControlMachineFault>;

    // Named instruction lowerers follow.
}
```

The header resolver may mutate composite state, produce program module
metadata, configure multiple devices, or validate machine-wide constraints.
It must run before module installation and must not depend on arbitrary live
child-device state during source resolution.

If resolved headers populate the program module's `Info`,
`BuildProgramModule` needs an explicit metadata assignment operation such as:

```rust
fn set_info(module: &mut Self::Module, info: Self::Info);
```

The exact metadata flow should follow the Acamar pattern:

```text
ControlHeaderBlock
    -> composite header resolver
    -> resolved Info
    -> runtime module installation
```

## Loading and nesting

Generated loading uses the composite's complete syntax dialect:

```rust
let parsed = ParsedModule::<control_machine::syntax::Module>
    ::parse_section(section.clone())?;

let module = self.resolve_parsed(parsed)?;
```

The loading sequence is:

```text
parse section
    -> ParsedModule<CompositeModuleSyntax>
    -> resolve composite-owned header
    -> lower component surface instructions
    -> resolve module metadata
    -> build runtime module
    -> install runtime module
```

Each nested composite owns its own generated module syntax:

```text
RootMachine::syntax::Module
ChildMachine::syntax::Module
```

Recursive loading requires only:

```text
ChildMachine: LoadSstSubtree<Context>
```

The parent does not name the child's source types, instruction enum, or header
type.

## Implementation order

1. Add component instruction-set syntax products for instructions, values, and
   types; keep them optional for runtime-only components.
2. Add composite-generated instruction, value, and type sums.
3. Add composite-owned header syntax and header-resolution hooks.
4. Add `ModuleSyntax` and refactor `ParsedModule`/`ParsedFunction`.
5. Update `Resolve<S>` and migrate standalone syntax tests.
6. Update generated composite parsing, semantic analysis, lowering, and module
   loading.
7. Add program-backed resolution for labels, types, constants, and metadata.
8. Decide and implement the module metadata assignment operation, if needed.
9. Rename `LoadOwnSstSection`/`LoadSstSection` to the program/subtree model.
10. Generate recursive nested composite loading.
11. Add Acamar-shaped header, source-sum, semantic-analysis, and nested-loading
    tests.
12. Migrate the demo and documentation.

## Non-goals

Components do not own SST section headers, device aliases, route identity, or
machine-wide lowering policy. Components may own parsers for their local
instruction and value/type syntax, but they do not become aware of the
composite that mounts them.
