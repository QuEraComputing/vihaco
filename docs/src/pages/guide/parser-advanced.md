---
layout: ../../layouts/Guide.astro
title: Loading SST programs
slug: parser-advanced
description: "Build an image from sectioned source, explicit metadata, and symbolic lowering."
---

# Loading SST programs

> **SST's future:** SST may become unsupported or be superseded by a bytecode
> format plus a disassembler. It is not a promised long-term compiler target.

If you're writing a compiler, start with
[emitting SST from a compiler](/guide/sst-reference) for the file format and
metadata you must produce. This chapter describes the consuming loader.

Loading an SST program means choosing its section, reading the declarations,
resolving symbols, and building a module. Do this outside the mutable execution
machine, then pair the result with the machine when it's ready.

## Read a section

Start with `SSTFile<C>::from_text(source)`. The type `C` interprets the file's
global context; use `NoContext` when you have no global data. A file holds a
flat list of named sections: `section(name)` picks one and `sections()` lists
them in file order. Each section is an independently loaded program. Section
names are only for selecting a section; execution uses instruction addresses,
and any mapping from addresses to names belongs in the header.

A section view gives you the header and source body. Pass it to
`ParsedModule::<SourceInstruction, Header>::parse_section(section)`, using
your composite's syntax instruction type as `SourceInstruction`. You'll get the
header declarations, the section's flat code stream, and its symbol table: one
`Symbol { name, address }` for each `@name:` definition in `.text`.

## Collect declarations before lowering

Use `HeaderDeclarations<Declaration>` to parse the header one directive per
line. The standard directives (`.file`, `.func`, `.entry`, `.const`,
`.source_symbol`) are `ProgramDirective` values; an application enum adds its
own directives next to them and implements `StandardDirective`, so the checker
and formatter can find the standard ones:

```rust
use vihaco::module::{ProgramDirective, StandardDirective};

#[derive(Debug, vihaco_parser_derive::Parse)]
#[syntax_class(metadata)]
enum Declaration {
    #[pattern = "$0"]
    Standard(ProgramDirective),
    #[pattern = "experiment.description $0"]
    Description(String),
}

impl StandardDirective for Declaration {
    type Value = vihaco::Value;
    type Type = vihaco::Type;

    fn as_standard(&self) -> Option<&ProgramDirective> {
        match self {
            Self::Standard(directive) => Some(directive),
            Self::Description(_) => None,
        }
    }
}
```

Push each standard directive into a `ProgramHeader`, in header order, and
collect your own directives into a state object you write: reject duplicates
and check required settings before building execution artifacts. Then call
`header.finish(extra)` to assemble a `ProgramMetadata` whose `extra` is your
application metadata. It supplies every non-code field:

- Function declarations, with their signatures and reserved local counts.
- Strings, constants, and source symbols.
- The entry function, if there is one.
- Your application's `extra` metadata.

No index is written by hand. Function order is `.func` order and constant order
is `.const` order; `.entry @name` and `.const fn @name` name functions; the
string table is derived from the names, files, and `.const str` literals the
header mentions (see the [SST reference](/guide/sst-reference#the-header) for
the exact order). `finish` reports duplicate functions, a bad `.entry`, too few
locals, and the other header rules; `assemble` returns every problem instead.

A `.func` doesn't say where its function is. Loading binds it to the
`@name:` symbol with the same name and derives the range from there to the next
function symbol, so there are no addresses to keep in sync. A function's local
count includes its parameters. The producer supplies any
extra locals; the loader doesn't infer them from instructions.

## Prepare capabilities and lower

`MetadataLoweringContext::new(&metadata, &symbols)` prepares the standard symbol,
function, and string capabilities. Its string lookup interns: a literal already
in the table keeps its index, and a new one is appended, so string operands in
`.text` need no header entry. If custom instructions need more lookup services,
you can extend it or use your own context.

Call `lower_code_with_metadata(code, &symbols, &metadata, &mut context)` to check
the declarations, symbols, and tables, then lower the code. The context knows the
whole symbol table before the first instruction is lowered. That's how a branch
or call can refer to a symbol farther down the section.

Store the completed string table with `metadata.strings = context.into_strings()`
(contexts that wrap `MetadataLoweringContext` implement `InternedStrings` for
`ParsedModule::lower_with_metadata`, which does this for you). Then assemble the
result with `metadata.into_module(code, symbols)`. It keeps your table order
and checks standard value references. If your declarations need a different
runtime metadata type, use `map_extra`. You can then wrap the module in an Image
and pair it with a machine in a Loadable.

If lowering fails, it returns no partial instruction vector. Any changes already
made to the context remain, though; the pipeline doesn't roll them back.

## Run the complete example

The repository's
[Loadable example](https://github.com/QuEraComputing/vihaco/blob/main/crates/vihaco-stdlib/examples/loadable.rs)
and its
[SST file](https://github.com/QuEraComputing/vihaco/blob/main/crates/vihaco-stdlib/examples/loadable.sst)
put the whole sequence together. They include an application header, a function
table in a different order from the code, a helper call, and a forward branch.

Run it from a checkout:

```sh
cargo run -p vihaco-stdlib --features program-image-execution --example loadable
```

The result is `[42]`. To try another file, pass its source path after `--`.
Instead of running, `--check` reports every problem in the file, `--fmt` (or
`--fmt-plain`, without generated notes) prints it formatted, and `--disasm`
loads it and prints the loaded program
[disassembled](/guide/sst-reference#disassembling-programs) back to SST. The
`heterogeneous` example in the same directory loads a file whose two sections
target different machines.
Once construction finishes, the loader drops the file view and source. The image
owns the runtime instructions and metadata it needs.

[Binary formats](/guide/instructions-advanced) covers loading encoded programs.
