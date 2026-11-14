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
                file: 1,
            },
        ],
        constants: [],
        main_function: 0,
        source_symbols: [],
        extra: {},
    };
.text
@main:
    execution::program::const_u64 42
    execution::program::ret 1
```

The directives are:

| Directive | Meaning |
|---|---|
| `sst v2` | Format version. Must be the first non-blank line. |
| `.global` | Optional global context in a consumer-defined grammar. Must come before any `.section`; omit it for loaders using `NoContext`. |
| `.section <name>` | Starts a new section. Names are required, unique within the file, and contain no whitespace. |
| `.header` | The current section's header: declarations such as `metadata { ... };`. |
| `.text` | The current section's code: a flat instruction stream with `@name:` symbol definitions. |

Sections are a flat list; there is no nesting and no implicit root. Each may
contain at most one `.header` and one `.text`, in either order. An omitted block
yields empty text to the consumer; a runnable program still needs the content
its loader requires. Indentation is free-form, and directive lines may end with
a `//` comment. Every other line belongs to the enclosing block and is passed to
the target's grammar unchanged.

A section is an independently loadable unit. Its name exists only so a loader
can select it with `file.section(name)`. Execution relies on instruction
addresses, which are local to the section. Names for addresses within a
section come from its symbol definitions, described below.

## Target instruction and function syntax

The target composite's `<Machine as HasInstructionSet>::Syntax` determines which
instructions the loader accepts. Emit the device route, dialect, and mnemonic,
followed by the operands specified by that instruction's pattern. For example,
`calculator::arithmetic::add` routes to the `calculator` device. A nested device
adds route segments; device aliases may provide additional accepted spellings.
Numeric `#[device(code)]` values are not the source spelling.

## Symbols

`.text` is not structured code. It is one flat instruction stream, written in
address order, plus symbol definitions:

```text
.text
@main:
    execution::program::call 0, @helper
    execution::program::br @done
    execution::program::const_u64 999
@done:
    execution::program::ret 1
@helper:
    execution::program::const_u64 21
    execution::program::ret 1
```

- `@name:` defines a symbol: it binds `name` to the address of the next
  instruction and occupies no instruction slot. Several symbols may share an
  address, and a symbol may follow the last instruction.
- `@name` in an operand refers to a symbol. Lowering looks it up in the
  section's symbol table, so forward references work. Each instruction decides
  what it needs from the entry: `br` and `call` take the symbol's address,
  while a function-reference constant takes the function's metadata index.
- Symbol names are unique within a section. There is no per-function scope; a
  compiler that reuses local label names should qualify them, for example
  `@main.done`.
- Layout and indentation carry no meaning. Instructions and symbol definitions
  are separated by whitespace or `//` comments, and may share a line.

A function is a symbol that has a declaration in the header's `functions` table
with the same name. Its signature and locals come only from that declaration.
Its executable range runs from its symbol to the next function symbol, or to the
end of the section; other symbols inside it, such as `@done`, do not end it.
Nothing stops execution from running past the end of one function into the next.

Each parsed instruction lowers to exactly one runtime instruction. Expand any
compiler pseudo-instructions before emitting SST. Symbolic operands and their
punctuation are dialect-specific; see the
[parser pattern reference](/guide/parser-patterns).

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
| `strings` | An ordered array of quoted strings, including function names, parameter names, filenames, and literals needed by lowering. String indices are zero-based. Plain symbols such as branch targets need no entry. |
| `functions` | An ordered array of function declarations, described below. Array position is the function index; this order may differ from address order. |
| `constants` | An ordered array of values parsed by the target's value parser. Constant indices are array positions. |
| `main_function` | The entry function's table index, or `null` when no entry is selected. It is not an instruction address. |
| `source_symbols` | Records `{ index: number, name: "symbol" }`. Indices must be unique; names are inline strings. Agree with the consumer on their meaning. |
| `extra` | Application metadata in the target's chosen schema. With the default `NoInfo`, emit `{}`. |

Each function record contains:

| Field | Meaning |
|---|---|
| `name` | Index of the function name in `strings`, without the `@` prefix. It must match a `@name:` symbol in `.text`, and names must be unique. |
| `signature` | `{ params: [{ name: string_index, ty: type }, ...], ret: [type, ...] }`. This is the only place a function's signature is stated. |
| `local_count` | Total reserved local slots, including parameters. Must be at least the parameter count; the loader does not infer extra locals. |
| `file` | Index of the source filename in `strings`. |

A declaration says *what* a function is; its symbol says *where* it is. There
are no address fields to keep in sync. Addresses and table references use `u32`
and count decoded instructions, not bytes, source lines, or symbol definitions.
In the example, `main` covers `0..2`. Function-reference constants lower to
metadata table indices, independently of address order.

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
the supplied function indices. Symbol lookup uses the whole section's table and
returns absolute, section-local addresses.

Application header directives can describe device configuration or add lowering
capabilities. These require an explicit agreement with the consuming loader;
the standard metadata record does not define their schema. See
[loading SST programs](/guide/parser-advanced) for declaration preparation and
custom contexts.

## Validate compiler output

Load emitted files through the same path as the target application:
`SSTFile::from_text`, `ParsedModule::parse_section`, declaration preparation,
`MetadataLoweringContext::new`, `lower_code_with_metadata`, then
`metadata.into_module(code, symbols)`. This binds each function declaration to
its symbol, rejects duplicate or missing symbols, derives instruction ranges,
resolves symbolic operands, and checks standard constant references. Loading preserves table order and does
not invent missing metadata or select an entry function for you.

For the minimal file above, this loader validates and assembles the program:

```rust
use vihaco::{HasInstructionSet, NoContext, SSTFile, composite};
use vihaco::loader::MetadataLoweringContext;
use vihaco::module::ProgramMetadata;
use vihaco::syntax::{HeaderDeclarations, ParsedModule, lower_code_with_metadata};

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
    let ParsedModule {
        header,
        code,
        symbols,
    } = ParsedModule::<
        SourceInstruction, HeaderDeclarations<ProgramMetadata>
    >::parse_section(section)?;
    let mut declarations = header.into_iter();
    let metadata = declarations.next().ok_or_else(|| eyre::eyre!("missing metadata"))?;
    eyre::ensure!(declarations.next().is_none(), "duplicate metadata");
    let mut context = MetadataLoweringContext::new(&metadata, &symbols)?;
    let code = lower_code_with_metadata(code, &symbols, &metadata, &mut context)?;
    let module = metadata.into_module(code, symbols)?;
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
Test both accepted programs and malformed output: missing strings, missing
or duplicate symbols, functions without a symbol, insufficient locals,
unresolved symbolic operands, and invalid table references. Execute accepted
programs to check behavior as well as syntax.

## Checking SST files

The loading path stops at the first problem. To report every problem in a file
at once, with its line and column, use the `vihaco-sst` crate. It checks a file
against your machine's instruction set without loading or executing anything:

```rust
# macro_rules! include_str {
#     ("first-machine.sst") => {
#         std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/examples/first-machine.sst"))
#     };
# }
use vihaco::{Execute, PopFromStack, PushToStack, component, composite, dialect};
use vihaco_sst::{SourceFile, check_program};

#[dialect]
pub mod arithmetic {
    #[instruction]
    pub struct Add;
}

#[component]
#[instructions { arithmetic }]
#[derive(Default)]
pub struct Calculator;

impl Execute<arithmetic::Add> for Calculator {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(&mut self, _: &arithmetic::Add, message: Self::Message) -> eyre::Result<Self::Effect> {
        let PopFromStack((lhs, rhs)) = message;
        Ok(PushToStack((lhs + rhs,)))
    }
}

#[composite]
struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    calculator: Calculator,
}

fn main() {
    let text = include_str!("first-machine.sst");
    let source = SourceFile::new("first-machine.sst", text);
    assert!(check_program::<Machine>(&source).is_ok());

    let broken = text
        .replace("main_function: 0", "main_function: 3")
        .replace("const_u64 22", "const_u64 x")
        .replace("arithmetic::add", "arithmetic::sub");
    let source = SourceFile::new("broken.sst", broken);
    let report = check_program::<Machine>(&source);
    let messages: Vec<String> = report.iter().map(ToString::to_string).collect();
    assert_eq!(messages, [
        "16:24: error: metadata.main_function: function index 3 is out of range (metadata.functions has 1 entry)",
        "23:35: error: unexpected `x` in operands of `execution::program::const_u64`",
        "24:5: error: expected an instruction, found `calculator::arithmetic::sub`",
    ]);
    eprint!("{}", report.render(&source));
}
```

Each diagnostic carries a severity, a message, a file span with its line and
column, and optional secondary labels and notes. `render` prints them in the
style of rustc:

```text
error: unexpected `x` in operands of `execution::program::const_u64`
   --> broken.sst:23:35
   |
23 |     execution::program::const_u64 x
   |                                   ^ unexpected operand
```

`check_program` checks every section with the standard `metadata { ... };`
header and `MetadataLoweringContext` lowering. It reports framing errors,
header and `.text` syntax errors, duplicate symbols, metadata table errors,
functions without a symbol, and instructions that fail to lower, such as an
unresolved `@name` or a string missing from `strings`. Use `ProgramChecker` to
select sections with `require_sections`, parse an application header with
`with_header::<Declarations<YourDeclaration>>()`, or lower with your own
context. `check_framing` needs no machine, and `check_file` accepts any
`SectionChecker` for other header or instruction grammars.

Continue with [binary formats](/guide/instructions-advanced) for encoded programs
and compatibility planning.
