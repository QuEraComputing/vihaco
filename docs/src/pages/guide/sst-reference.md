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

### Comments

A comment runs from `//` to the end of the line. Comments may end a directive
line and may appear wherever the header and `.text` grammars allow whitespace:
on their own line, after an instruction or declaration, or between the fields of
a `metadata { ... }` record. They are never part of a string literal. The
`.global` block is consumer-defined and decides its own comment rules.

Comments that start with `//!` are *generated*: tools such as the
[formatter](#formatting-sst-files) write them to explain the file and replace
them every time they run, so do not write `//!` comments by hand. `//!` starts
generated text wherever it appears, including inside a `//` comment: in
`name: 1, // the helper //! "helper"`, `// the helper` is yours and
`//! "helper"` is generated. Parsers treat both kinds as ordinary comments.

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

## Formatting SST files

`vihaco-sst` also formats SST files into one canonical layout. Like the checker,
the formatter is generic over your machine and never executes anything. It
keeps your `//` comments and writes `//!` comments that spell out what indices
and symbols refer to:

```rust
# macro_rules! include_str {
#     ("first-machine.sst") => {
#         std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/examples/first-machine.sst"))
#     };
# }
# use vihaco::{Execute, PopFromStack, PushToStack, component, composite, dialect};
# #[dialect]
# pub mod arithmetic {
#     #[instruction]
#     pub struct Add;
# }
# #[component]
# #[instructions { arithmetic }]
# #[derive(Default)]
# pub struct Calculator;
# impl Execute<arithmetic::Add> for Calculator {
#     type Message = PopFromStack<(u64, u64)>;
#     type Effect = PushToStack<(u64,)>;
#     type Error = eyre::Report;
#     fn execute(&mut self, _: &arithmetic::Add, message: Self::Message) -> eyre::Result<Self::Effect> {
#         let PopFromStack((lhs, rhs)) = message;
#         Ok(PushToStack((lhs + rhs,)))
#     }
# }
# #[composite]
# struct Machine {
#     #[device(0)]
#     #[program]
#     execution: vihaco_stdlib::ExecutionState,
#     #[device(1)]
#     calculator: Calculator,
# }
use vihaco_sst::{FormatOptions, SourceFile, check_program, format_program};

fn main() {
    let source = SourceFile::new("first-machine.sst", include_str!("first-machine.sst"));
    let formatted = format_program::<Machine>(&source, FormatOptions::default())
        .unwrap_or_else(|report| panic!("{}", report.render(&source)));

    // Formatting is idempotent and the result is still a valid program.
    let again = SourceFile::new("first-machine.sst", formatted.as_str());
    assert_eq!(format_program::<Machine>(&again, FormatOptions::default()).unwrap(), formatted);
    assert!(check_program::<Machine>(&again).is_ok());

    // Without generated comments, only the layout changes.
    let plain = format_program::<Machine>(&source, FormatOptions::without_generated_comments());
    assert!(!plain.unwrap().contains("//!"));
#   let page = std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/src/pages/guide/sst-reference.md"));
#   let section = page.split_once("## Formatting SST files").unwrap().1;
#   let expected = section.split_once("```text\n").unwrap().1.split_once("```").unwrap().0;
#   assert_eq!(formatted, expected);
}
```

For [`first-machine.sst`](/guide/first-machine) the result is:

```text
sst v2

.section main
.header
    metadata {
        functions: [
            {
                name: 0,   //! "main"
                signature: { params: [], ret: [u64] },
                local_count: 0,
                file: 1,   //! "first-machine.sst"
            },
        ],
        constants: [],
        strings: ["main", "first-machine.sst"],
        main_function: 0,  //! @main
        source_symbols: [],
        extra: {},
    };
.text
@main:  //! fn main() -> u64, locals 0, functions[0]
    execution::program::const_u64 20
    execution::program::const_u64 22
    calculator::arithmetic::add
    execution::program::ret 1
```

The file is checked first, with the same `ProgramChecker` as
[`check_program`](#checking-sst-files); if it has errors, `format_program`
returns the `Report` instead. Instructions and header declarations are
re-spelled with their `Print` implementations, so operands, keywords, and
punctuation take their canonical spelling. A value that has no spelling that
parses back to itself is reported as an error rather than altered. Before
returning, the formatter parses its own output and checks that every section
has the same headers, code, symbols, and comments as the input.

### Layout

- Directives start at column 0, one per line; a directive's comment follows it
  after two spaces. One blank line precedes `.global` and every `.section`.
  `.header` and `.text` follow without a blank line and keep their order.
- `.global` content is consumer-defined and kept verbatim, except that trailing
  whitespace and trailing blank lines are removed.
- Header declarations are indented four spaces and end with `;`. A
  `metadata { ... }` record always puts one field per line. Nested records and
  lists stay on one line when they fit in `max_width` (100 columns by default)
  and contain no comments; otherwise they are broken one item per line with a
  trailing comma, like rustfmt. Record fields appear in their declaration
  order, so the standard record starts with `functions`. Tuples and
  declarations whose pattern needs its punctuation on one line are never
  broken.
- In `.text`, each symbol definition and each instruction gets its own line:
  `@name:` at column 0 and instructions indented four spaces.
- Blank lines between items are kept, collapsed to at most one. Blank lines at
  the start of a block and inside a declaration are removed.
- Trailing comments of a block are aligned to one column: two spaces past the
  longest commented line, but no further than `comment_column` (column 60 by
  default). Longer lines put their comment two spaces after the code.
- Lines have no trailing whitespace, line endings become `\n`, and the file
  ends with a single newline.

### Comments

A `//` comment on its own line stays above the item that follows it. A trailing
comment stays at the end of its item's line; inside a header record, the item
is the field or list entry, and it travels with that field when `Print`
reorders fields. Comments at the end of a block stay there, unless a blank line
separates them from the last item: then they introduce the next directive and
are printed just above it. When one item had several trailing comments, the
first stays trailing and the others move above the item. If a comment's
position inside a declaration cannot be matched to the printed declaration, it
is moved above the declaration.

Generated comments are rewritten every time. Without them
(`FormatOptions::without_generated_comments()`), existing `//!` comments are
still removed. The formatter writes:

| Where | Generated comment |
|---|---|
| Function symbol | `@main:  //! fn main() -> u64, locals 0, functions[1]` |
| Other symbol | `@done:  //! 0x0005` (its address) |
| `@name` operand | `br @done  //! @done = 0x0005`; functions add their index: `//! @helper = 0x0006 (functions[0])` |
| String literal operand | `print "hi"  //! "hi" = strings[3]` |
| `functions[i].name`, `.file` | `name: 1,  //! "helper"` and `file: 2,  //! "loadable.sst"` |
| `functions[i].signature` with parameters | `//! fn helper(x: u64) -> u64` |
| `main_function` | `main_function: 1,  //! @main` |
| `str N` / `fn N` constants | `str 3,  //! "hi"` and `fn 0,  //! @helper` |

Notes are looked up in the section's metadata and symbol table. Several notes
on one line are separated by `; `. Machine-specific notes can be added with
`Formatter::with_annotator`, which receives each instruction with its address,
the symbol table, and the string table. For an application header, use
`Formatter::with_checker` with the same checker you would use for
`ProgramChecker`, for example
`ProgramChecker::<Machine>::new().with_header::<Declarations<YourDeclaration>>()`.
The `metadata { ... }` record inside an application declaration gets the same
layout and notes.

Continue with [binary formats](/guide/instructions-advanced) for encoded programs
and compatibility planning.
