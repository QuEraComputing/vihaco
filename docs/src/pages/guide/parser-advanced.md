---
layout: ../../layouts/Guide.astro
title: Module Parsing and Loading
slug: parser-advanced
description: Load compiler-produced SST metadata and function bodies through generated section hooks and instruction-level lowering.
---

# Module Parsing and Loading

SST loading uses the existing section hooks. The upstream compiler supplies
complete program metadata; Vihaco parses it, checks it against `fn` bodies,
invokes each instruction's `Lower<C>`, and assembles the loaded image.

`#[composite]` generates section traversal and child routing. The machine author
implements `LoadOwnSstSection` for the data owned directly by that section and
provides the context capabilities its instructions use during lowering. There
is no second compiler or instruction-by-instruction resolver to implement.

## Complete loading example

This example loads a CPU program into a child machine through its parent's
**generated** `LoadSstSection` implementation. It stops at loading. A composite
still requires `Step` coverage for its instruction inventory, so this example
includes a guard that rejects execution. Replace that guard with your machine's
normal `Step` / `BuildMessage` implementations before running instructions.

Enable `typed-declarations` on **both** `vihaco` and `vihaco-cpu`. The standalone
`Parse` derive currently also requires direct `vihaco-parser` and `chumsky`
dependencies. The example uses `eyre` for errors.

Save this compiler-produced source as `program.sst`:

```text
sst v1
.section(root):
    .section(controller):
    .header(controller):
        device cpu.clock_hz 1000;
        metadata {
            strings: ["main", "helper", "program.sst", "hello", "done"],
            functions: [
                {
                    name: 0,
                    signature: { params: [], ret: [] },
                    local_count: 0,
                    start_address: 0,
                    end_address: 4,
                    file: 2,
                },
                {
                    name: 1,
                    signature: { params: [], ret: [] },
                    local_count: 0,
                    start_address: 4,
                    end_address: 5,
                    file: 2,
                },
            ],
            labels: [{ name: 4, address: 3 }],
            constants: [],
            main_function: 0,
            source_symbols: [],
            extra: {},
        };
    .header(controller).
    .text(controller):
        fn @main() {
            cpu::cpu::const_string "hello"
            cpu::cpu::call 0, @helper
            cpu::cpu::br @done
            label @done
            cpu::cpu::ret 0
        }
        fn @helper() {
            cpu::cpu::ret 0
        }
    .text(controller).
    .section(controller).
.section(root).
```

Every metadata field is required, including empty tables and `extra`. Array
positions define table indices. Function and label addresses count runtime
instructions, not bytes; `end_address` is exclusive. `label` declarations do not
occupy instruction slots. Device declarations keep their existing syntax and
can appear before or after `metadata`.

The Rust code below contains the machine-owned header parser and authored
own-section hooks. `MetadataLoweringContext` provides the standard CPU lookup
capabilities, and `ParsedModule::lower_with_metadata` handles validation,
function scopes, instruction lowering, and module assembly. Record parsing is
derived; the header parser handles declarations and their semicolons.

```rust no_run
# #[cfg(feature = "typed-declarations")]
# mod loading_example {
use eyre::{eyre, Result};
use vihaco::chumsky::prelude::*;
use vihaco::module::ProgramMetadata;
use vihaco::loader::MetadataLoweringContext;
use vihaco::syntax::{skip, ParsedModule};
use vihaco::traits::FromText;
use vihaco::{
    composite, HasInstructionSet, LoadOwnSstSection, LoadSstSection,
    NoContext, Parse, ProgramImage, SstFile, SstHeader, SstSectionView, Type,
};
// Imported typed components need their generated inventory companions.
pub use vihaco_cpu::{CPU, __vihaco_fields_CPU, __vihaco_inventory_CPU};

type MachineInstruction = <Machine as HasInstructionSet>::Runtime;
type MachineSyntax = <Machine as HasInstructionSet>::Syntax;

#[composite]
#[derive(Default)]
struct Machine {
    #[device(0)]
    cpu: CPU,
    image: ProgramImage<MachineInstruction, NoContext>,
    clock_hz: u64,
}

// A composite requires Step coverage for its instruction inventory even when
// only loading. This example rejects execution; a real machine supplies its
// normal Step / BuildMessage implementations here.
impl<I> vihaco::Step<vihaco::field!(Machine::cpu), I> for Machine
where
    CPU: vihaco::Execute<I>,
{
    fn step(&mut self, _: I) -> Result<()> {
        Err(eyre!("this example loads programs but does not execute them"))
    }
}

#[composite]
#[derive(Default)]
struct Rig {
    #[device(0)]
    #[loadable]
    controller: Machine,
}

// The machine owns the declaration list; the record parser owns its fields.
#[derive(Parse)]
#[syntax_class(metadata)]
enum HeaderItem {
    #[pattern = "$0"]
    Metadata(ProgramMetadata),
    #[pattern = "device cpu.clock_hz $0"]
    ClockHz(u64),
}

struct Header {
    metadata: ProgramMetadata,
    clock_hz: u64,
}

impl FromText for Header {
    fn from_text(text: &str) -> Result<Self> {
        let items = HeaderItem::parser()
            .padded_by(skip())
            .then_ignore(just(';').padded_by(skip()))
            .repeated()
            .collect::<Vec<_>>()
            .then_ignore(skip())
            .then_ignore(end())
            .parse(text)
            .into_result()
            .map_err(|errors| eyre!("invalid header: {errors:?}"))?;

        let mut metadata = None;
        let mut clock_hz = 1000;
        for item in items {
            match item {
                HeaderItem::Metadata(value) => {
                    eyre::ensure!(metadata.is_none(), "duplicate metadata declaration");
                    metadata = Some(value);
                }
                HeaderItem::ClockHz(value) => clock_hz = value,
            }
        }
        Ok(Self {
            metadata: metadata.ok_or_else(|| eyre!("missing program metadata"))?,
            clock_hz,
        })
    }
}

impl SstHeader for Header {}

// Authored hook: load only the program owned by this section.
impl LoadOwnSstSection<NoContext> for Machine {
    fn load_own_sst_section(
        &mut self,
        section: SstSectionView<'_, NoContext>,
    ) -> Result<()> {
        let context_handle = section.context_handle();
        let ParsedModule { header, functions } =
            ParsedModule::<MachineSyntax, Type, Header>::parse_section(section)?;
        let Header { metadata, clock_hz } = header;
        let mut context = MetadataLoweringContext::new(&metadata)?;
        let module = ParsedModule { header: (), functions }
            .lower_with_metadata(metadata, &mut context)?;

        // Install only after validation and lowering succeed.
        self.image = ProgramImage {
            module,
            context: Some(context_handle),
            pc: 0,
        };
        self.clock_hz = clock_hz;
        Ok(())
    }
}

// This parent section only contains children, so it has no program to load.
impl LoadOwnSstSection<NoContext> for Rig {
    fn load_own_sst_section(
        &mut self,
        _: SstSectionView<'_, NoContext>,
    ) -> Result<()> {
        Ok(())
    }
}

fn load(source: &str) -> Result<Rig> {
    let file = SstFile::<NoContext>::from_text(source)?;
    let mut rig = Rig::default();
    // Generated LoadSstSection calls the root hook, routes "controller", and
    // invokes the child's generated loader and authored own-section hook.
    rig.load_sst_section(file.root())?;
    Ok(rig)
}

fn main() -> Result<()> {
    let source = std::fs::read_to_string("program.sst")?;
    let rig = load(&source)?;
    println!("Loaded {} instructions", rig.controller.image.module.code.len());
    Ok(())
}
# }
```

The call sequence is:

```text
SstFile::from_text
  → Rig::load_sst_section                     generated
    → Rig::load_own_sst_section               authored, no root program
    → Machine::load_sst_section               generated child routing
      → Machine::load_own_sst_section         authored
        → ParsedModule::parse_section
        → MetadataLoweringContext::new
        → ParsedModule::lower_with_metadata
          → validate declarations and metadata
          → FunctionScope::enter_function before each body
          → Lower::lower for each instruction
          → ProgramMetadata::into_module
        → install ProgramImage
```

After loading, the child image contains five instructions. The call to `helper`
targets address `4`, the branch to `done` targets address `3`, and the string
literal references string index `3`. The image owns its program data and retains
the global context handle after the `SstFile` is dropped.

### Responsibilities and validation

| Responsibility | Owner |
|---|---|
| Produce tables, signatures, addresses, entrypoint, and local counts | Upstream compiler |
| Locate section/header/body ranges | `SstFile` |
| Route independently loadable child sections | Generated `LoadSstSection` |
| Choose device declarations and apply configuration | Machine-owned header and hook |
| Parse typed `fn` bodies and check agreement with supplied metadata | `ParsedModule` |
| Convert one instruction | Its `Lower<C>` implementation |
| Supply fixed function and string lookups and function-local label scopes | `MetadataLoweringContext` |
| Validate and lower functions with instruction error context | `ParsedModule::lower_with_metadata` |
| Check final table bounds and assemble standard `Value` metadata | `ProgramMetadata::into_module` |

`validate_metadata` checks function names, signatures, ranges, and declared
labels, including duplicate function-local labels. It does not fill missing
metadata. `into_module` checks final table bounds, local counts, and standard
constant references while moving the supplied fields into `LocalModule`.
Custom value models can use `ProgramMetadata::validate` and assemble their own
`LocalModule` with their value-specific checks.

The context above deliberately **looks up** strings instead of appending them.
It rejects a literal absent from the compiler-supplied table. Function indices
and function addresses remain separate lookups, and each function gets its own
label scope. An instruction can require additional application capabilities,
such as qubit pools or durations, through its `Lower<C>` bounds.

The own-section hook uses a temporary context because `Lower<C>` accepts any
context type. A consumer can wrap `MetadataLoweringContext`, add capabilities,
and implement `FunctionScope` by delegating to the wrapped context. The driver
prepares the complete function-local label scope before lowering its first
instruction, so forward references work. Each syntax instruction produces one
runtime instruction, preserving the supplied instruction addresses.
`ProgramMetadata::map_extra` changes consumer-specific extra information without
copying or reordering tables. Configuration-only parent sections can keep
device-only headers, while each executable section requires its program metadata.

The example installs its image after its own parsing, validation, and lowering
succeed. Generated loading is not transactional across the whole tree: an
already-loaded parent or child is not rolled back if a later child fails.

The complete code block is compile-checked with:

```sh
cargo test -p vihaco-doctests --doc --features typed-declarations guide_parser_advanced
```

## Existing declaration-based parsing

The following sections describe the general parsed-module API and the legacy
feature-off `Resolve` path. With `typed-declarations`, use the generated syntax
aggregate and instruction-level `Lower<C>` as above.

## Parsed module types

`vihaco::syntax` exposes the typed intermediate representation:

```rust ignore
use vihaco_parser::Ident;

pub struct ParsedModule<I, Ty, H> {
    pub header: H,
    pub functions: Vec<ParsedFunction<I, Ty>>,
}

pub struct ParsedFunction<I, Ty> {
    pub name: Ident,
    pub params: Vec<Param<Ty>>,
    pub return_types: Vec<Ty>,
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

Parameters are written as comma-separated `name: type` pairs. An empty
parameter list is written as `()`, as in `fn @main() { ... }`:

```text
fn @compute(input: i64, output: f64) {
    // ...
}
```

## Step 1: define the instruction and type syntax

Function-body instructions and source types implement `Parse`. The pattern
derive also emits the `SurfaceInstruction` marker for instruction enums, but
`ParsedModule::parse_section` itself only requires their parsers.

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
    device.wait 10
    device.halt
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

## Step 4: legacy feature-off resolution

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


Function return types are stored in `ParsedFunction::return_types`. A single
return uses `-> u64`, multiple returns use `-> (u64, bool)`, and no annotation
means an empty return list. Supplied metadata must agree with this list.
