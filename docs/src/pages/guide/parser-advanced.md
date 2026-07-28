---
layout: ../../layouts/Guide.astro
title: Advanced Parser Customization
slug: parser-advanced
description: Module-level parsing with section headers, typed function bodies, Resolve implementations, and custom Parse composition.
---

# Advanced Parser Customization

This guide picks up where [Parser Integration](/guide/parser) ends. A derived
parser handles one syntax type. The module layer adds:

- an SST section header;
- one or more `fn @name() { ... }` blocks;
- indentation, blank lines, and `//` comments; and
- a resolver that turns the typed parsed module into the runtime module your
  machine loads.

Module parsing is strict. Each function body is a `Vec<I>` produced by
`I::parser()`. There is no untyped fallback: symbolic operands, source sugar,
and other source-only forms must be represented explicitly in the syntax type
and its patterns.

## Parsed module types

`vihaco::syntax` exposes the typed intermediate representation:

```rust ignore
pub struct ParsedModule<I, H>
where
    I: SurfaceInstruction,
{
    pub header: H,
    pub functions: Vec<ParsedFunction<I>>,
}

pub struct ParsedFunction<I>
where
    I: SurfaceInstruction,
{
    pub name: String,
    pub params: Vec<Param>,
    pub return_ty: Option<SurfaceType>,
    pub body: Vec<I>,
}
```

Whitespace and `//` comments are skipped between instructions. An unknown
instruction or a partially matched pattern makes the function parse fail.

## Step 1: mark the instruction syntax

Types used in parsed function bodies implement the `SurfaceInstruction`
marker in addition to `Parse`.

```rust ignore
use vihaco::Instruction;
use vihaco::syntax::SurfaceInstruction;
use vihaco_parser::Parse;

#[derive(Debug, Clone, PartialEq, Instruction, Parse)]
#[syntax_class(instruction, head = "device")]
enum DeviceInstruction {
    Halt,
    #[pattern = "'wait $0"]
    Wait(u32),
}

impl SurfaceInstruction for DeviceInstruction {}
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
    ParsedModule::<DeviceInstruction, DeviceHeader>::parse_section(file.root())?;
```

`parsed.header` is the typed `DeviceHeader`, while each function body contains
only `DeviceInstruction` values.

## Step 4: resolve into a runtime module

`Resolve<I, H>` owns the application-specific conversion from a
`ParsedModule<I, H>` to any output module type.

```rust ignore
use vihaco::module::LocalModule;
use vihaco::syntax::{ParsedModule, Resolve};
use vihaco::{Type, Value};

#[derive(Default)]
struct DeviceResolver;

impl Resolve<DeviceInstruction, DeviceHeader> for DeviceResolver {
    type Module = LocalModule<DeviceInstruction, Value, Type>;

    fn resolve_module(
        &mut self,
        parsed: ParsedModule<DeviceInstruction, DeviceHeader>,
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
#[derive(vihaco_parser::Parse)]
#[syntax_class(instruction, head = "control")]
enum ControlSurface {
    #[pattern = "'branch `@` $0"]
    Branch(String),
    #[pattern = "'repeat $0"]
    Repeat(u32),
}
```

A resolver can map `Branch(String)` through a label table and expand
`Repeat(u32)` into multiple runtime instructions. Malformed spellings are
rejected by the parser instead of being deferred as untyped data.

Quoted strings and richer expressions require field types with suitable
`Parse` implementations. Keep state such as intern tables in the resolver;
the parsed field should carry the owned source value needed for that later
conversion.

## Hand-write `Parse` for generated composite enums

`#[derive(Parse)]` works on types you declare. A macro-generated composite
instruction enum cannot be annotated at its generated definition, so compose
its device parsers manually:

```rust ignore
use chumsky::prelude::*;
use vihaco_parser_core::Parse;

impl<'src> Parse<'src> for MachineInstruction {
    fn parser() -> impl Parser<
        'src,
        &'src str,
        Self,
        chumsky::extra::Err<chumsky::error::Simple<'src, char>>,
    > {
        let cpu = CpuSurface::parser().map(MachineInstruction::Cpu);
        let signal = SignalSurface::parser().map(MachineInstruction::Signal);
        choice((cpu, signal))
    }
}
```

Each nested parser owns its namespace, so dispatch remains explicit and typed.

## When to hand-write a complete parser

Use a manual `Parse` implementation when the grammar needs recursion,
context-sensitive coordination, quoted/nested structures, or recovery that
cannot be expressed by the pattern grammar. The result should still be a typed
syntax value that can participate in `ParsedFunction` and `ParsedModule`.

For ordinary instruction, value, and type shapes, prefer
`#[syntax_class]` plus `#[pattern]`: the derive validates field coverage,
punctuation, and constructor mapping at compile time.
