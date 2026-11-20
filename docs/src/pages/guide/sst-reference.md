---
layout: ../../layouts/Guide.astro
title: Emitting SST from a compiler
slug: sst-reference
description: "The SST file structure, line rules, header directives, and instruction syntax a compiler must produce."
---

# Emitting SST from a compiler

> **SST's future:** SST may become unsupported or be superseded by a bytecode
> format plus a disassembler. It is not a promised long-term compiler target.
> The [binary format](/guide/instructions-advanced) is also currently unstable;
> this warning does not imply a stable replacement is already available.

A compiler targeting SST needs both a target machine's instruction vocabulary
and the header directives expected by its loader. SST provides section framing
and the standard program directives; the target supplies instruction, type, and
value grammars and any application directives. Keep the compiler and loader on
the same vihaco revision and agree on application-specific directives and
lowering rules.

## File and section structure

SST is a line-based, assembly-style file. Emit `sst v2` first, then an optional
`.global` block, then one or more named sections. Each block directive sits on
its own line and opens a block that runs until the next block directive or the
end of the file; there are no closing markers. This is a complete program for a
machine with an `execution` device using the standard program dialect:

```text
sst v2

.section main
.header
    .file "output.sst"
    .func @main() -> u64, locals 0
    .entry @main
.text
@main:
    execution::program::const_u64 42
    execution::program::ret 1
```

The block directives are:

| Directive | Meaning |
|---|---|
| `sst v2` | Format version. Must be the first non-blank line. |
| `.global` | Optional global context in a consumer-defined grammar. Must come before any `.section`; omit it for loaders using `NoContext`. |
| `.section <name>` | Starts a new section. Names are required, unique within the file, and contain no whitespace. |
| `.header` | The current section's header: one directive per line, such as `.func @main() -> u64, locals 0`. |
| `.text` | The current section's code: a flat instruction stream with `@name:` symbol definitions. |

Sections are a flat list; there is no nesting and no implicit root. Each may
contain at most one `.header` and one `.text`, in either order. An omitted block
yields empty text to the consumer; a runnable program still needs the content
its loader requires. Every other line belongs to the enclosing block and is
passed to the target's grammar unchanged; only these five directives are
reserved, so header directives such as `.func` are block content.

A section is an independently loadable unit. Its name exists only so a loader
can select it with `file.section(name)`. Execution relies on instruction
addresses, which are local to the section. Names for addresses within a
section come from its symbol definitions, described below.

### Lines and comments

SST reads like assembly: every statement (a directive in `.header`, a symbol
definition or instruction in `.text`) ends at the end of its line. There is no
statement terminator. A statement continues onto the next line only while a
`(`, `[`, or `{` is open, as in Python; there is no `\` continuation. Two
statements on one line, or an operand moved to the next line outside brackets,
is an error. Indentation and spacing carry no meaning, and `\r\n` is a newline.
A bracket left open at the end of a block is reported where it was opened, as
``unclosed `[` opened at line N``, and the lines after it are checked normally.

A comment runs from `;` to the end of the line. Comments may end any line,
including block directive lines, and may stand on their own line anywhere in
the file, even before `sst v2` or between `.section` and its first block. Inside a
bracket-continued statement they may also end each continued line. A `;`
inside a string literal is part of the string, so string literals cannot span
lines (write `\n`). The `.global` block is consumer-defined and decides its own
comment rules.

A comment on its own line that starts with `;;` (but not `;;;`) is
*generated*: tools such as the [formatter](#formatting-sst-files) write it
directly above the item it explains and replace it every time they run, so do
not write such lines by hand. Every other comment is yours, including a
trailing comment that starts with `;;` or contains it, such as
`call 0, @helper  ; see ;; notes`, and `;;;` headings. Parsers treat both kinds
as ordinary comments.

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
@done: execution::program::ret 1
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
  while a function-reference constant takes the function's table index.
- Symbol names are unique within a section. There is no per-function scope; a
  compiler that reuses local label names should qualify them, for example
  `@main.done`.
- Each instruction and each symbol definition goes on its own line, except
  that one symbol definition may be followed by one instruction on its line,
  as in `@done: execution::program::ret 1`, like `label: mov` in assembly.

A function is a symbol that has a `.func` directive with the same name in the
header. Its signature and locals come only from that directive. Its executable
range runs from its symbol to the next function symbol, or to the end of the
section; other symbols inside it, such as `@done`, do not end it. Nothing stops
execution from running past the end of one function into the next.

Each parsed instruction lowers to exactly one runtime instruction. Expand any
compiler pseudo-instructions before emitting SST. Symbolic operands and their
punctuation are dialect-specific; see the
[parser pattern reference](/guide/parser-patterns).

## The header

The standard header is a list of flat directives, one per line, that name
things by symbol. The loader builds every table and index from them; no index
or string table is written by hand:

```text
.header
    .file "loadable.sst"                  ; the file of the .func lines that follow
    .func @helper(x: u64) -> u64, locals 1
    .func @main() -> (u64, bool), locals 0
    .entry @main
    .const u64 42                         ; constants[0]
    .const str "hello"                    ; a string constant, by its text
    .const fn @helper                     ; a function reference, by name
    .source_symbol 12 "user"
    experiment.description "an example"   ; an application directive
```

| Directive | Meaning |
|---|---|
| `.file "path"` | The source file of the `.func` directives that follow, until the next `.file`. A `.func` before any `.file` belongs to the empty file `""`. |
| `.func @name(param: type, ...) -> ret, locals N` | Declares a function. The return is omitted (no results), one type (`-> u64`), or a parenthesized list (`-> (u64, bool)`). `locals N` is the total number of reserved local slots, including parameters, and must be at least the parameter count; the loader does not infer extra locals. The function table is in `.func` order, and each name must match a `@name:` symbol in `.text` and be unique. |
| `.entry @name` | Selects the entry function, by name. Optional; at most one. |
| `.const <value>` | Appends to the constant pool; the pool is in `.const` order. `.const str "text"` is a string constant and `.const fn @name` a reference to a declared function. Other values use the value type's syntax: with standard `Value`, typed literals such as `u64 42`, `i64 -1`, `bool true`, `f64 -0.5`, or `f64_bits` followed by a decimal `u64` (and `f32_bits` with a `u32`) to preserve exact floating-point bits, including NaNs. A table index such as `str 3` or `fn 0` is rejected. |
| `.source_symbol <index> "<name>"` | A source-level symbol. Indices are unique; agree with the consumer on their meaning. |

Directives may appear in any order; their relative order matters only within a
kind (`.func` order, `.const` order, and which `.file` precedes a `.func`). The
grammar leaves room for named constants (`.const @name <value>`) later.

Parameter and symbol names contain no whitespace and none of
`` : @ , ; ( ) [ ] { } " ' ` ``. A long `.func` may be broken inside its
parentheses.

### Derived tables

`ProgramHeader` assembles the directives into `ProgramMetadata`:

- **functions**: one per `.func`, in order; `.entry` and `.const fn` resolve to
  these indices.
- **constants**: one per `.const`, in order.
- **source symbols**: one per `.source_symbol`, in order.
- **strings**: each distinct string once, at its first occurrence, in this
  order: function names in `.func` order; then parameter names, by function and
  then by position; then file names in `.func` order; then `.const str`
  literals in `.const` order. Lowering then appends the string literals that
  `.text` instructions intern, in address order. The same file always yields
  the same table.

For the example above, `strings` is `["helper", "main", "x", "loadable.sst",
"hello"]`, `functions[0]` is `@helper`, `main_function` is `1`, and the
constants are `[u64 42, str 4, fn 0]`.

Addresses and table references use `u32` and count decoded instructions, not
bytes, source lines, or symbol definitions. In the first example, `main`
covers `0..2`. Function-reference constants lower to function-table indices,
independently of address order.

### Application directives

A loader can accept its own directives in the same header, such as device
configuration or `experiment.description` above. Its declaration enum holds a
`ProgramDirective` variant (usually `#[pattern = "$0"]`) next to its own
variants and implements `StandardDirective` to expose it; the loader pushes the
standard directives into a `ProgramHeader` and handles the rest, then passes
its own result as the metadata's `extra`. These directives require an explicit
agreement with the consuming loader. See
[loading SST programs](/guide/parser-advanced) for declaration preparation and
custom contexts.

## Constants, strings, and symbol resolution

`MetadataLoweringContext` resolves symbols through the whole section's table
and returns absolute, section-local addresses. Function lookup preserves the
`.func` indices. Its string lookup interns: a string already in the table
(including the header's strings) keeps its index, and a new one is appended,
so `.text` string literals need no header entry. Store the completed table with
`metadata.strings = context.into_strings()` before assembling the module.

## Validate compiler output

Load emitted files through the same path as the target application:
`SSTFile::from_text`, `ParsedModule::parse_section`, assembling the directives
with `ProgramHeader`, `MetadataLoweringContext::new`, `lower_code_with_metadata`,
storing the interned strings, then `metadata.into_module(code, symbols)`. This
binds each function to its symbol, rejects duplicate or missing symbols,
derives instruction ranges, resolves symbolic operands, and checks standard
constant references. Loading preserves directive order and does not invent
missing declarations or select an entry function for you.

For the minimal file above, this loader validates and assembles the program:

```rust
use vihaco::{HasInstructionSet, NoContext, SSTFile, composite};
use vihaco::loader::MetadataLoweringContext;
use vihaco::module::{NoInfo, ProgramDirective, ProgramHeader};
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
        SourceInstruction, HeaderDeclarations<ProgramDirective>
    >::parse_section(section)?;
    let mut metadata = header.into_iter().collect::<ProgramHeader>().finish(NoInfo)?;
    let mut context = MetadataLoweringContext::new(&metadata, &symbols)?;
    let code = lower_code_with_metadata(code, &symbols, &metadata, &mut context)?;
    metadata.strings = context.into_strings();
    let module = metadata.into_module(code, symbols)?;
    assert_eq!(module.code.len(), 2);
    assert_eq!(module.main_function, Some(0));
    assert_eq!(module.strings, ["main", "output.sst"]);
    Ok(())
}
# let page = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/src/pages/guide/sst-reference.md"));
# let source = page.split_once("```text\n").unwrap().1.split_once("```").unwrap().0;
# validate(source)?;
# Ok::<(), eyre::Report>(())
```

Use [your first machine](/guide/first-machine) as a minimal consumer and the
[Loadable example](/guide/parser-advanced#run-the-complete-example) for
an application directive, a function table in a different order from the code,
a helper call, and a forward branch. Test both accepted programs and malformed
output: duplicate or missing symbols, `.func` without a symbol, insufficient
locals, an unknown `.entry`, and unresolved symbolic operands. Execute accepted
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
        .replace(".entry @main", ".entry @start")
        .replace("const_u64 22", "const_u64 x")
        .replace("arithmetic::add", "arithmetic::sub");
    let source = SourceFile::new("broken.sst", broken);
    let report = check_program::<Machine>(&source);
    let messages: Vec<String> = report.iter().map(ToString::to_string).collect();
    assert_eq!(messages, [
        "7:12: error: `.entry @start` names no `.func` directive",
        "11:35: error: unexpected `x` in operands of `execution::program::const_u64`",
        "12:5: error: expected an instruction, found `calculator::arithmetic::sub`",
    ]);
    eprint!("{}", report.render(&source));
}
```

Each diagnostic carries a severity, a message, a file span with its line and
column, and optional secondary labels and notes. `render` prints them in the
style of rustc:

```text
error: unexpected `x` in operands of `execution::program::const_u64`
   --> broken.sst:11:35
   |
11 |     execution::program::const_u64 x
   |                                   ^ unexpected operand
```

`check_program` checks every section with the standard directive header and
`MetadataLoweringContext` lowering. It reports framing errors; header and
`.text` syntax errors, one per bad line, including unknown directives, two
statements on a line, and unclosed brackets; duplicate symbols; header
directive problems (duplicate `.func` names, too few `locals`, more than one
`.entry` or one naming no `.func`, a `.const fn` naming no `.func`, a constant
written as a table index, duplicate `.source_symbol` indices), each pointing at
the offending part of its line; `.func` directives without a `@name:` symbol;
and instructions that fail to lower, such as an unresolved `@name`. A bad
directive does not stop the `.text` checks: they run against the directives
that were fine. Use `ProgramChecker` to select sections with
`require_sections`, parse an application header with
`with_header::<Declarations<YourDeclaration>>()`, or lower with your own
context. `check_framing` needs no machine, and `check_file` accepts any
`SectionChecker` for other header or instruction grammars.

## Formatting SST files

`vihaco-sst` also formats SST files into one canonical layout. Like the checker,
the formatter is generic over your machine and never executes anything. It
keeps your `;` comments and writes `;;` comments that spell out what indices
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
    assert!(!plain.unwrap().lines().any(|line| line.trim_start().starts_with(";;")));
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
    .file "first-machine.sst"
    ;; functions[0], @main = 0x0000..0x0004
    .func @main() -> u64, locals 0
    .entry @main
.text
;; fn main() -> u64, locals 0, functions[0]
@main:
    execution::program::const_u64 20
    execution::program::const_u64 22
    calculator::arithmetic::add
    execution::program::ret 1
```

The file is checked first, with the same `ProgramChecker` as
[`check_program`](#checking-sst-files); if it has errors, `format_program`
returns the `Report` instead. Instructions and header directives are
re-spelled with their `Print` implementations, so operands, keywords, and
punctuation take their canonical spelling. A value that has no spelling that
parses back to itself is reported as an error rather than altered. Before
returning, the formatter parses its own output and checks that every section
has the same headers, code, symbols, and comments as the input.

### Layout

- Block directives start at column 0, one per line; a directive's comment
  follows it after two spaces. One blank line precedes `.global` and each
  `.section`, except that the first section after `.global` keeps its spacing.
  `.header` and `.text` follow without a blank line and keep their order.
- The consumer defines the grammar for the `.global` body. The formatter
  preserves every byte of the body, including whitespace before the next
  section.
- Header directives are indented four spaces under `.header`, like
  instructions under `.text`, one per line, in source order. Indentation is
  only layout: the parser ignores it.
  Their order is meaningful (`.func` and `.const` order define the tables, and
  `.file` applies to the `.func` lines after it), so the formatter never
  reorders them, and application directives stay where they are. A directive
  stays on one line; an application directive with a list or record that does
  not fit in `max_width` (80 columns by default), or that carries comments, is
  broken one item per line inside its brackets with a trailing comma, like
  rustfmt. Tuples and declarations whose pattern needs its punctuation on one
  line are never broken.
- In `.text`, each symbol definition and each instruction gets its own line:
  `@name:` at column 0 and instructions indented four spaces.
- Blank lines between items are kept, collapsed to at most one. Blank lines at
  the start of a block and inside a declaration are removed.
- Generated `;;` comments always sit on their own lines, directly above the
  item they describe and at its indentation, one line per note.
- Trailing user comments of a block are aligned to one column: two spaces past
  the longest commented line, but no further than `comment_column` (column 60
  by default). Longer lines put their comment two spaces after the code.
- Formatted lines have no trailing whitespace and use `\n` line endings. The
  formatter ends the file with a single newline when the final block is
  formatted.

### Comments

A `;` comment on its own line stays above the item that follows it. A trailing
comment stays at the end of its item's line; inside a bracket-continued
declaration, the item is the list entry or record field, and it travels with
that field when `Print` reorders fields. A comment never leaves the block it
was written in: comments after the last item of a `.header` or `.text` stay at
the end of that block, after a blank line if there was one. When one item had
several trailing comments, the first stays trailing and the others move above
the item. If a comment's position inside a declaration cannot be matched to the
printed declaration, it is moved above the declaration. A comment outside
every block (before `sst v2`, before the first `.global` or `.section`, or
between `.section` and its first block) stays directly above the directive
that follows it.

Generated comments are rewritten every time: every own-line `;;` comment is
removed and fresh ones are written. Without them
(`FormatOptions::without_generated_comments()`), existing own-line `;;`
comments are still removed. Each note is written on the line above the item it
describes. Directives that only restate their operand, `.entry` and `.file`,
get none:

| Item | Generated comment |
|---|---|
| `.func @main(...)` | `;; functions[1], @main = 0x0000..0x0006` (its index and range) |
| `.const u64 42` | `;; constants[0]` |
| `.const str "hello"` / `.const fn @helper` | `;; constants[1] = strings[3]` / `;; constants[2] = functions[0]` |
| Function symbol `@main:` | `;; fn main() -> u64, locals 0, functions[1]` |
| Other symbol `@done:` | `;; 0x0005` (its address) |
| Instruction with an `@name` operand | `;; @done = 0x0005`; functions add their index: `;; @helper = 0x0006 (functions[0])` |
| Instruction with a string literal | `;; "hi" = strings[3]` |

For example:

```text
;; fn main() -> u64, locals 0, functions[1]
@main:
    ;; @helper = 0x0006 (functions[0])
    execution::program::call 0, @helper
    execution::program::const_u64 999  ; never reached
```

Notes are looked up in the section's assembled metadata and symbol table. The
string indices of `.text` literals follow the [derived table](#derived-tables),
assuming each string literal operand is interned as it lowers, as the standard
dialects do. An item with several notes gets one `;;` line per note.
Machine-specific notes can be added with `Formatter::with_annotator`, which
receives each instruction with its address, the symbol table, and the string
table. For an application header, use `Formatter::with_checker` with the same
checker you would use for `ProgramChecker`, for example
`ProgramChecker::<Machine>::new().with_header::<Declarations<YourDeclaration>>()`.
The standard directives inside an application header get the same notes.

Continue with [binary formats](/guide/instructions-advanced) for encoded programs
and compatibility planning.
