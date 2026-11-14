---
layout: ../../layouts/Guide.astro
title: Emitting SST from a compiler
slug: sst-reference
description: "The SST file structure, instruction syntax, and metadata a compiler must produce."
---

# Emitting SST from a compiler

> **SST's future:** SST may become unsupported or be superseded by a bytecode
> format plus a disassembler. It is not a promised long-term compiler target.
> The [binary format](/guide/instructions-advanced) is also currently unstable;
> this warning does not imply a stable replacement is already available.

A compiler targeting SST needs both a target machine's instruction vocabulary
and the program metadata expected by its loader. SST provides section framing;
the target supplies instruction, type, value, and header grammars. Keep the
compiler and loader on the same vihaco revision and agree on application-specific
declarations and lowering rules.

## File and section structure

SST is a line-based, assembly-style file. Emit `sst v2` first, then an optional
`.global` block, then one or more named sections. Each directive sits on its own
line and opens a block that runs until the next directive or the end of the
file; there are no closing markers. This is a complete program for a machine
with an `execution` device using the standard program dialect:

```text
sst v2

.section main
.header
    metadata {
        strings: ["main", "output.sst"],
        functions: [
            {
                name: 0,
                signature: { params: [], ret: [u64] },
                local_count: 0,
                start_address: 0,
                end_address: 2,
                file: 1,
            },
        ],
        labels: [],
        constants: [],
        main_function: 0,
        source_symbols: [],
        extra: {},
    };
.text
    fn @main() -> u64 {
        execution::program::const_u64 42
        execution::program::ret 1
    }
```

The directives are:

| Directive | Meaning |
|---|---|
| `sst v2` | Format version. Must be the first non-blank line. |
| `.global` | Optional global context in a consumer-defined grammar. Must come before any `.section`; omit it for loaders using `NoContext`. |
| `.section <name>` | Starts a new section. Names are required, unique within the file, and contain no whitespace. |
| `.header` | The current section's header: declarations such as `metadata { ... };`. |
| `.text` | The current section's code: function bodies. |

Sections are a flat list; there is no nesting and no implicit root. Each may
contain at most one `.header` and one `.text`, in either order. An omitted block
yields empty text to the consumer; a runnable program still needs the content
its loader requires. Indentation is free-form, and directive lines may end with
a `//` comment. Every other line belongs to the enclosing block and is passed to
the target's grammar unchanged.

A section is an independently loadable unit. Its name exists only so a loader
can select it with `file.section(name)`. Execution relies on instruction
addresses, which are local to the section. Anything that ties an address to a
name, such as functions and labels, belongs in the header metadata described
below.

## Target instruction and function syntax

The target composite's `<Machine as HasInstructionSet>::Syntax` determines which
instructions the loader accepts. Emit the device route, dialect, and mnemonic,
followed by the operands specified by that instruction's pattern. For example,
`calculator::arithmetic::add` routes to the `calculator` device. A nested device
adds route segments; device aliases may provide additional accepted spellings.
Numeric `#[device(code)]` values are not the source spelling.

Emit functions as `fn @name(param: type, ...) -> type { ... }`. Omit `->` for no
returns, or use `-> (type, type)` for multiple returns. The target type parser
defines accepted type names. Function parameters and return types must agree
with the header metadata.

Inside a function, `label @name` declares a local branch target. It occupies no
instruction slot. Instructions such as `execution::program::br @done` can refer
forward to a label. Symbolic operands and their punctuation are dialect-specific;
see the [parser pattern reference](/guide/parser-patterns).

Each parsed instruction lowers to exactly one runtime instruction. Expand any
compiler pseudo-instructions before calculating ranges and emitting SST.
Concatenating function bodies in source order defines the section's code vector.

## The metadata header

The standard loading path uses `HeaderDeclarations<ProgramMetadata>` or an
application declaration enum containing `ProgramMetadata`. Declarations end in
`;`. The first-machine loader requires exactly one `metadata { ... };` record;
other loaders choose their own required declarations and duplicate policy.

Emit every field below, including empty arrays. Records use named fields and
comma-separated lists; trailing commas are accepted. The standard schema is
`ProgramMetadata<Value, Type, NoInfo>`:

| Field | What the compiler supplies |
|---|---|
| `strings` | An ordered array of quoted strings, including function names, parameter names, filenames, label names, and literals needed by lowering. String indices are zero-based. |
| `functions` | An ordered array of function records, described below. Array position is the function index; this order may differ from function-body order. |
| `labels` | Records `{ name: string_index, address: instruction_address }`. Every source label needs a matching name and absolute section-local address. |
| `constants` | An ordered array of values parsed by the target's value parser. Constant indices are array positions. |
| `main_function` | The entry function's table index, or `null` when no entry is selected. It is not an instruction address. |
| `source_symbols` | Records `{ index: number, name: "symbol" }`. Indices must be unique; names are inline strings. Agree with the consumer on their meaning. |
| `extra` | Application metadata in the target's chosen schema. With the default `NoInfo`, emit `{}`. |

Each function record contains:

| Field | Meaning |
|---|---|
| `name` | Index of the function name in `strings`, without the source `@` prefix. Function names must be unique for standard metadata-backed lookup. |
| `signature` | `{ params: [{ name: string_index, ty: type }, ...], ret: [type, ...] }`. Parameter names, order, and types and return types must match the source declaration. |
| `local_count` | Total reserved local slots, including parameters. Must be at least the parameter count; the loader does not infer extra locals. |
| `start_address` | Index of the first instruction in this function's body in the concatenated code vector. |
| `end_address` | Exclusive end of that body's instruction range. |
| `file` | Index of the source filename in `strings`. |

Addresses and table references use `u32`. Count decoded instructions, not bytes,
source lines, or labels. In the example, `main` covers `0..2`. In a two-function
program whose bodies have six and two instructions, their ranges are `0..6` and
`6..8`, even if their metadata records appear in reverse order. Calls that lower
to function indices follow metadata table order.

## Constants, strings, and symbol resolution

With standard `Value`, constants use typed literals such as `u64 42`, `i64 -1`,
`bool true`, `string 0`, and `fn 0`. String values reference `strings`;
function references select `functions`. The loader checks these references when
assembling the module. Use `f32_bits` followed by a decimal `u32` or `f64_bits`
followed by a decimal `u64` to preserve exact floating-point bits, including NaNs.
Custom value types can define a different grammar.

Populate strings before lowering. `MetadataLoweringContext` looks up strings
without appending or reordering them; a missing literal is an error. Equal
strings resolve to the first matching table entry. Function lookup preserves
the supplied function indices. Label lookup is scoped to the current function,
so different functions can reuse a label name. Its resolved address is still
absolute within the section.

Application header directives can describe device configuration or add lowering
capabilities. These require an explicit agreement with the consuming loader;
the standard metadata record does not define their schema. See
[loading SST programs](/guide/parser-advanced) for declaration preparation and
custom contexts.

## Validate compiler output

Load emitted files through the same path as the target application:
`SSTFile::from_text`, `ParsedModule::parse_section`, declaration preparation,
`MetadataLoweringContext::new`, `lower_functions_with_metadata`, then
`metadata.into_module(code)`. This checks function declarations against the
supplied tables, verifies source labels and instruction ranges, resolves symbols,
and checks standard constant references. Loading preserves table order and does
not invent missing metadata or select an entry function for you.

For the minimal file above, this loader validates and assembles the program:

```rust
use vihaco::{HasInstructionSet, NoContext, SSTFile, Type, composite};
use vihaco::loader::MetadataLoweringContext;
use vihaco::module::ProgramMetadata;
use vihaco::syntax::{HeaderDeclarations, ParsedModule, lower_functions_with_metadata};

#[composite]
struct Machine {
    #[device(0)]
    execution: vihaco_stdlib::ExecutionState,
}

fn validate(source: &str) -> eyre::Result<()> {
    type SourceInstruction = <Machine as HasInstructionSet>::Syntax;
    let file = SSTFile::<NoContext>::from_text(source)?;
    let section = file
        .section("main")
        .ok_or_else(|| eyre::eyre!("missing main section"))?;
    let ParsedModule { header, functions } = ParsedModule::<
        SourceInstruction, Type, HeaderDeclarations<ProgramMetadata>
    >::parse_section(section)?;
    let mut declarations = header.into_iter();
    let metadata = declarations.next().ok_or_else(|| eyre::eyre!("missing metadata"))?;
    eyre::ensure!(declarations.next().is_none(), "duplicate metadata");
    let mut context = MetadataLoweringContext::new(&metadata)?;
    let code = lower_functions_with_metadata(functions, &metadata, &mut context)?;
    let module = metadata.into_module(code)?;
    assert_eq!(module.code.len(), 2);
    assert_eq!(module.main_function, Some(0));
    Ok(())
}
# let page = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/src/pages/guide/sst-reference.md"));
# let source = page.split_once("```text\n").unwrap().1.split_once("```").unwrap().0;
# validate(source)?;
# Ok::<(), eyre::Report>(())
```

Use [your first machine](/guide/first-machine) as a minimal consumer and the
[Loadable example](/guide/parser-advanced#run-the-complete-example) for multiple
sections, reordered function metadata, a helper call, and a forward branch.
Test both accepted programs and malformed output: missing strings, mismatched
signatures or ranges, insufficient locals, unresolved labels, and invalid table
references. Execute accepted programs to check behavior as well as syntax.

Continue with [binary formats](/guide/instructions-advanced) for encoded programs
and compatibility planning.
