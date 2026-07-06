---
layout: ../../layouts/Guide.astro
title: Defining a Composite
slug: composites
description: "Composite structs are the composition root in vihaco — #[composite] generates the outer instruction enum and device metadata; message resolution and effect delivery are hand-written."
---

# Defining A Composite With `vihaco`

Composite structs are the composition root in `vihaco`.
They own components, observers, and device codes, and they are where the
generated wiring meets your hand-written runtime.

This guide shows how to wire components and observers into a composite using the current macro surface.

If you have not read the observer guide yet, read [Observing Effects With `#[observe]`](/guide/observers) first.
For a focused guide to composite-side message resolution before component execution, read [Using Messages With `vihaco`](/guide/messages).

## A Small Composite

Assume you already have:

- a component such as `Counter`
- an effect type such as `StdoutEffect`
- a type that observes that effect

```rust
use eyre::Result;
use vihaco::{Effects, observe};

#[derive(Debug, Clone)]
pub struct StdoutEffect(pub String);

#[derive(Debug, Default)]
pub struct StdoutCollector {
    lines: Vec<String>,
}

#[observe(StdoutEffect)]
impl StdoutCollector {
    fn observe_stdout_effect(&mut self, effect: &StdoutEffect) -> Result<Effects<()>> {
        self.lines.push(effect.0.clone());
        Ok(Effects::none())
    }
}
```

Now you can compose a runtime root:

```rust
# use eyre::Result;
# use vihaco::{Effects, Instruction, component, observe};
# #[derive(Debug, Clone, Instruction)]
# pub enum CounterInst { Print }
# #[derive(Debug, Default)]
# pub struct Counter;
# #[component(instruction = CounterInst, message = ())]
# impl Counter {
#     fn execute(&mut self, _inst: CounterInst, _msg: ()) -> Result<Effects<()>> { Ok(Effects::none()) }
# }
# #[derive(Debug, Clone)]
# pub struct StdoutEffect(pub String);
# #[derive(Debug, Default)]
# pub struct StdoutCollector { lines: Vec<String> }
# #[observe(StdoutEffect)]
# impl StdoutCollector {
#     fn observe_stdout_effect(&mut self, effect: &StdoutEffect) -> Result<Effects<()>> {
#         self.lines.push(effect.0.clone());
#         Ok(Effects::none())
#     }
# }
use vihaco::composite;

#[composite]
#[derive(Debug, Default)]
pub struct CounterComposite {
    #[device(0x00, alias = "count")]
    counter: Counter,

    // A plain observer field — the runtime delivers StdoutEffect to it.
    stdout: StdoutCollector,
}
```

## What `#[composite]` Generates

`#[composite]` is transitional scaffolding that generates the repetitive composition glue from the `#[device(...)]` fields:

- **An outer instruction enum** named `<StructName>Instruction`, with one variant per device field. Each variant is the PascalCase of the field name and wraps that component's instruction type. For `CounterComposite` above the macro emits, roughly:

  ```rust ignore
  #[derive(Debug, Clone, Instruction)]
  pub enum CounterCompositeInstruction {
      Counter(<Counter as GeneratedComponent>::Instruction),
  }
  ```

- **Composite metadata** — an `impl GeneratedMachine` whose `metadata()` returns a `CompositeMetadata` listing each device's code and field name, plus the source-symbol aliases (so a loader can map a name like `"counter"` to its device code).

- **Section loading glue** — `LoadBytecodeSection` and `LoadSstSection` impls that call your own-section loader for the composite's own section, then route direct child sections to `#[loadable]` devices.

The `#[device]` and `#[loadable]` attributes are stripped from the struct the macro emits, so they don't leak into your type.

The long-term model is still explicit Rust composition. The macro is convenience for the device dispatch and metadata, not the semantic center of the design — message resolution and effect delivery stay in hand-written runtime code.

## The Field Attributes

### `#[device(CODE, alias = "…")]`

Associates a component field with a device code and optional source aliases.

```rust ignore
#[device(0x00, alias = "count")]
counter: Counter,
```

- `CODE` is a `u8` device code; it must be unique across the composite (a duplicate is a compile error).
- `alias = "…"` registers a source-symbol alias for the field; you can repeat it for multiple aliases. The field name itself is always registered as a source symbol, and every name (field or alias) must be unique across the composite.

The field type must implement `GeneratedComponent` (which `#[component(...)]` provides). The device code and aliases are what a loader uses to validate source symbols and route instructions when a composite loads a module.

### Own Section Loading

Headers, program streams, and program-counter delegation are ordinary Rust. Implement `LoadOwnBytecodeSection` or `LoadOwnSstSection` for the composite to load the current section's own data:

```rust ignore
#[derive(Default)]
pub struct CpuHeader {
    cores: u32,
}

#[composite]
#[derive(Default)]
pub struct CpuMachine {
    info: CpuHeader,
    program: vihaco::ProgramLoader<CpuInst>,
}

impl<C: vihaco::GlobalContext> vihaco::LoadOwnBytecodeSection<C> for CpuMachine {
    fn load_own_bytecode_section<'a>(
        &mut self,
        input: vihaco::BytecodeLoadInput<'a, C>,
    ) -> eyre::Result<()> {
        self.info = input.section.decode_header::<CpuHeader>()?;
        self.program.load_bytecode_section(input)?;
        Ok(())
    }
}
```

For SST, use `SstSectionView::parse_header::<H>()` and `ProgramLoader`'s `LoadSstSection<C>` impl. If a composite drives an instruction pointer, implement `ProgramCounter` manually by delegating to the field that owns it.

For structural composites that only route child sections, make that no-op explicit:

```rust ignore
impl<C: vihaco::GlobalContext> vihaco::LoadOwnBytecodeSection<C> for Machine {
    fn load_own_bytecode_section<'a>(
        &mut self,
        _input: vihaco::BytecodeLoadInput<'a, C>,
    ) -> eyre::Result<()> {
        Ok(())
    }
}
```

### `#[loadable]`

Marks a device field that should receive its own direct child section when loading v1 multi-section bytecode or SST. The device owns its loader internally; the generated composite loader only routes the section to the marked device's concrete load impl.

```rust ignore
#[composite]
#[derive(Default)]
pub struct Machine {
    #[device(0x01)]
    #[loadable("signal")]
    signal: SignalMachine,
}
```

`#[loadable]` must be used on a `#[device(...)]` field whose type implements the loader trait for the representation you are loading as well as `GeneratedComponent`. Binary bytecode delegates through `LoadBytecodeSection<C>`; SST delegates through `LoadSstSection<C>`. The attribute uses the field name as the local section name. `#[loadable("name")]` overrides it. Names must be non-empty direct child names, so they cannot contain `/`.

Section identity is represented as a `SectionPath`, which is a vector of resolved local section names. The root section is `SectionPath::root()` with zero components. A root child named `cpu` has the path `cpu`; a child named `alu` inside that section has `cpu/alu`. Generated loading asks the current section for direct children by local name, so the same composite can be loaded at the root or under another parent section.

Manual loaders can inspect a section through the concrete view type for the format being loaded: `BytecodeSectionView<'bc, C>` for bytecode and `SstSectionView<'bc, C>` for SST. Both expose `child(name)` for a direct child and `children()` for all direct children; start from `BytecodeFile::root()` or `SstFile::root()` to inspect an entire file.

The generated loader is strict:

- any present direct child section must correspond to a `#[loadable]` device field
- `#[loadable]` device fields are optional; if a file omits that child section, the device is left unchanged
- the composite's own section header/body is loaded only through `LoadOwnBytecodeSection` or `LoadOwnSstSection`
- manual loaders can inspect bytecode headers through `BytecodeSectionView::header_bytes()` / `BytecodeSectionView::decode_header::<H>()`, or SST headers through `SstSectionView::header_text()` / `SstSectionView::parse_header::<H>()`

## Multi-Section Bytecode

The read-side bytecode API lives in `vihaco` and `vihaco::loader`.

```rust ignore
fn load_machine<'bc>(file: &'bc vihaco::BytecodeFile) -> eyre::Result<Machine> {
    let mut machine = Machine::default();
    machine.load_bytecode_section(vihaco::BytecodeLoadInput::from(file))?;
    Ok(machine)
}

let file: vihaco::BytecodeFile = vihaco::BytecodeFile::from_bytes(bytes)?;
let machine = load_machine(&file)?;
```

The v1 file layout is:

```text
VHBC magic
u16 version = 1
u16 flags = 0
u64 program_context_len
program context bytes
root section bytes
```

The program context contains the shared `Module` tables except `code` and `extra`: constants, strings, functions, labels, `main_function`, `file`, and source symbols. `vihaco::program::ProgramContext<V = Value, Ty = Type>` is the default context representation and is generic over the VM's constant value and type encodings. A binary bytecode file can also use a custom context type by implementing `GlobalContext`; generated composite loading is generic over that context type, so Rust infers it from `BytecodeFile<C>` / `BytecodeLoadInput<C>`. Section bytecode can refer to shared constants with `vihaco::ConstantId`, a `u32` newtype that implements the bytecode field traits.

Each section is:

```text
section frame:
u64 section_len
u64 header_len
composite header bytes
section bytecode header:
u64 bytecode_len
bytecode bytes
child section table header:
u32 child_count
child table entries
child section bytes
```

Each child table entry stores:

```text
u32 local_name_string
u64 section_offset
```

The section frame is part of every section, including the root section. The bytecode header starts after the composite header, and the section's bytecode immediately follows that length. Child-related metadata comes after the parent bytecode. `local_name_string` is resolved through `GlobalContext::section_name` and represents the child's local section name. The parser builds each child's `SectionPath` by appending that resolved name to the parent path. Child section offsets are relative to the start of the containing section.

### SST Multi-Section Bytecode

SST uses `SstFile<C>`, `SstSectionView<'bc, C>`, `SstLoadInput<C>`, and generated `LoadSstSection<C>` machinery. The backing contents are the original SST and each section stores ranges into that string. Parse SST files with `SstFile::<C>::from_text(source)`:

```rust ignore
let file: vihaco::SstFile<MyContext> =
    vihaco::SstFile::from_text(source)?;

let mut machine = Machine::default();
machine.load_sst_section(vihaco::SstLoadInput::from(&file))?;
```

The file begins with the text magic/version marker, then a global context block:

```text
sst v1

.global:
global context text
.global.
```

`sst v1` is the text spelling of version 1. The context body is delegated to `C::from_text(context_text)`; custom SST formats usually provide a custom `GlobalContext` that interprets this block. The context start marker `.global:` and end marker `.global.` must be at indentation level 0.

After the context comes the root section. Sections use `.section(name):` to begin and `.section(name).` to end. The top-level section must be named `root` (`.section(root):` and `.section(root).`), and it is parsed as `SectionPath::root()`. Direct child section names become path components.

```text
.section(root):
	.header(root):
		root header
	.header(root).

	.text(root):
		root bytecode
	.text(root).

	.section(cpu):
		.text(cpu):
			cpu bytecode
		.text(cpu).
	.section(cpu).
.section(root).
```

Inside a section:

- `.header(name):` / `.header(name).` delimit the composite header text for `SstSectionView::header_text()`
- `.text(name):` / `.text(name).` delimit the section bytecode text for `SstSectionView::sst()`
- child sections are nested directly inside their parent section
- header, bytecode, and direct child section markers must be indented with exactly one tab more than their parent section
- section end markers must use the same indentation as their matching section start marker
- section names must be local names; `/` is rejected in a child marker name

The SST parser preserves the original header and bytecode ranges, including their leading tabs. Load section headers in `LoadOwnSstSection` with `section.parse_header::<H>()`; load section programs by delegating to `ProgramLoader<I, C>`, which parses `section.sst()` with `SstSectionView::parse_instructions::<I>()`. Instruction types loaded this way must implement both `Instruction` and `vihaco_parser_core::Parse`.

`ProgramLoader<I, C = ProgramContext, Info = NoInfo>` is the standard loader for section program streams. For binary bytecode it decodes `section.bytecode()` with `BytecodeSectionView::decode_instructions::<I>()`; for SST it parses `section.sst()` with `SstSectionView::parse_instructions::<I>()`. In both cases it stores a cloned `ContextHandle<C>`, implements `ProgramCounter`, and exposes functions, strings, and constants through `GetProgramGlobal` when `C: ProgramGlobals`.

## Effect Continuation Is Hand-Written

`#[composite]` generates the instruction enum and metadata, but it does **not** auto-deliver effects to observers. Continuing effects is something the runtime does explicitly: execute a component, then hand each returned effect to the types that observe it by calling their `Observe` impls.

```rust ignore
use vihaco::{GeneratedComponent, Observe};

impl CounterComposite {
    fn print(&mut self, msg: PrintPrefix) -> eyre::Result<()> {
        // Counter executes Print and returns a StdoutEffect.
        let effects = self.counter.execute_generated(CounterInst::Print, msg)?;
        // Deliver each effect to the observer that handles it.
        for effect in effects {
            Observe::<StdoutEffect>::observe(&mut self.stdout, &effect)?;
        }
        Ok(())
    }
}
```

Conventions to follow when you write that delivery:

- components return `Effects<T>`
- the runtime continues those effects to all matching observer fields
- both standalone observers and components that also observe receive effects through the same `Observe::observe` call
- follow-up effects continue depth-first
- `Effects::Many(...)` is continued left-to-right
- if an observer needs more data, stage it into a richer effect instead of relying on delivery context

## Hand-Written Runtimes

Not every runtime uses a generic step loop. Hand-written runtimes often call `execute_generated(...)` directly, extract the returned effects, and then interpret or re-deliver them themselves.

The common pattern is:

- use `effect = StepOutcome` when a component's direct output is control flow
- define a runtime-local sum-effect enum when a step needs to mix control flow with other follow-up values
- continue that runtime-local effect set in one place, forwarding observer-facing effects as needed

## Design Guidance

- Keep the composite struct explicit and readable.
- Put `#[observe]` on the type that actually consumes the effect.
- Use `#[device(...)]` aliases that match your source model.
- Implement `LoadOwnBytecodeSection` / `LoadOwnSstSection` for headers and program streams owned by the current section.
- Implement `ProgramCounter` manually when the composite drives an instruction pointer.
- Prefer staged effect types over hidden cross-component observer context.
- Let generated code own the device dispatch and metadata; keep effect delivery and message resolution in one clear place in your runtime.

## What Comes Next

At this point you have the core authoring model:

- components execute instructions
- `#[observe]` reacts to delivered effects
- composites generate the device wiring; the runtime resolves messages and continues effects

From here, the next useful step is to apply the same structure to your own domain types and source model.
