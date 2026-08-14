---
layout: ../../layouts/Guide.astro
title: Module Parsing and Resolution
slug: parser-advanced
description: Module-level parsing with section headers, typed function bodies, pattern-derived syntax, and Resolve implementations.
---

# Module Parsing and Resolution

This guide picks up where [Pattern Parser Integration](/guide/parser) ends. A
pattern-derived parser handles one syntax type. The module layer adds:

- an SST section header;
- one or more `fn @name() { ... }` blocks;
- indentation, blank lines, and `//` comments; and
- a resolver that turns the typed parsed module into the runtime module your
  machine loads.

Module parsing is strict. Each function body is a `Vec<I>` produced by
`I::parser()`. Symbolic operands, source sugar, and other source-only forms are
represented explicitly in the syntax type and its patterns.

## Parsed module types

`vihaco::syntax` exposes the typed intermediate representation:

```rust ignore
use vihaco::SurfaceInstruction;
use vihaco_parser::Ident;

pub struct ParsedModule<I, Ty, H>
where
    I: SurfaceInstruction,
{
    pub header: H,
    pub functions: Vec<ParsedFunction<I, Ty>>,
}

pub struct ParsedFunction<I, Ty>
where
    I: SurfaceInstruction,
{
    pub name: Ident,
    pub params: Vec<Param<Ty>>,
    pub return_ty: Option<Ty>,
    pub body: Vec<I>,
}

pub struct Param<Ty> {
    pub name: Ident,
    pub ty: Ty,
}
```

Whitespace and `//` comments are skipped between instructions. An unknown
instruction or a partially matched pattern makes the function parse fail.
The consumer-provided `Ty` parses parameter and return-type syntax, so the
framework does not impose a universal type language.

## Step 1: define the instruction and type syntax

Types used in parsed function bodies implement the `SurfaceInstruction`
marker in addition to `Parse`. The pattern derive emits both implementations
for instruction enums. The source type is any consumer-owned type that derives
`Parse`.

```rust ignore
use vihaco::Instruction;
use vihaco_parser_derive::Parse;

#[derive(Debug, Clone, PartialEq, Instruction, Parse)]
#[syntax_class(instruction, head = "device")]
enum DeviceInstruction {
    Halt,
    #[pattern = "'wait $0"]
    Wait(u32),
}

#[derive(Debug, Clone, PartialEq, Parse)]
#[syntax_class(type)]
enum DeviceType {
    #[pattern = "`i64`"]
    I64,
    #[pattern = "`f64`"]
    F64,
}
```

The source for that type uses fully qualified instructions:

```text
fn @main() {
    device::wait 10
    device::halt
}
```

## Step 2: define the section header

Section headers implement `FromText`; `SstHeader` marks types accepted by the
SST section parser.

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceHeader {
    pub core_count: u32,
}

impl vihaco::FromText for DeviceHeader {
    fn from_text(text: &str) -> eyre::Result<Self> {
        Ok(Self {
            core_count: text.trim().parse()?,
        })
    }
}

impl vihaco::SstHeader for DeviceHeader {}
```

Use a zero-sized header type when a section has no header data.

## Step 3: parse the section

```rust ignore
use vihaco::{NoContext, SstFile};
use vihaco::syntax::ParsedModule;

let file = SstFile::<NoContext>::from_text(source)?;
let parsed =
    ParsedModule::<DeviceInstruction, DeviceType, DeviceHeader>::parse_section(file.root())?;
```

`parsed.header` is the typed `DeviceHeader`, while each function body contains
only `DeviceInstruction` values and its signature uses `DeviceType`.

## Step 4: resolve into a runtime module

`Resolve<I, Ty, H>` owns the application-specific conversion from a
`ParsedModule<I, Ty, H>` to any output module type.

```rust ignore
use vihaco::module::LocalModule;
use vihaco::syntax::{ParsedModule, Resolve};
use vihaco::{Type, Value};

#[derive(Default)]
struct DeviceResolver;

impl Resolve<DeviceInstruction, DeviceType, DeviceHeader> for DeviceResolver {
    type Module = LocalModule<DeviceInstruction, Value, Type>;

    fn resolve_module(
        &mut self,
        parsed: ParsedModule<DeviceInstruction, DeviceType, DeviceHeader>,
    ) -> eyre::Result<Self::Module> {
        let mut module = LocalModule::default();
        for function in parsed.functions {
            module.code.extend(function.body);
        }
        Ok(module)
    }
}
```

A resolver may instead translate a source-oriented instruction type into a
different runtime instruction type, expand an explicitly modeled sugar
variant, or intern data carried by typed fields. The important boundary is
that parsing has already produced typed variants; resolution never receives an
unstructured source line.

## Model source-only forms explicitly

Patterns can represent symbols and sugar directly:

```rust ignore
use vihaco_parser::Ident;

#[derive(vihaco_parser_derive::Parse)]
#[syntax_class(instruction, head = "control")]
enum ControlSurface {
    #[pattern = "'branch `@` $0"]
    Branch(Ident),
    #[pattern = "'repeat $0"]
    Repeat(u32),
}
```

A resolver can map `Branch(Ident)` through a label table and expand
`Repeat(u32)` into multiple runtime instructions. Malformed spellings are
rejected by the parser instead of being deferred as untyped data.

Quoted strings use `QuotedString`; domain expressions use nested local enums
or structs that derive `Parse`. Keep state such as intern tables in the
resolver; the parsed field should carry the owned source value needed for that
later conversion.

## Parse composite sections by component

A generated composite runtime instruction enum is the runtime dispatch type. SST
source is parsed through user-declared surface instruction types, each deriving
`Parse` with its own namespace and patterns. Parse each component section as a
`ParsedModule<ComponentSurface, ComponentType, ComponentHeader>`, resolve it,
and load the resulting runtime instructions into that component.

This keeps source syntax attached to the component that owns it. Composite
loading routes sections to components; it does not require a second source
grammar for the generated composite runtime instruction enum.
