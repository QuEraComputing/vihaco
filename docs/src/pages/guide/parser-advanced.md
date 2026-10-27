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

Start with `SstFile<C>::from_text(source)`. The type `C` interprets the file's
global context; use `NoContext` when you have no global data. From `root()`,
`child(name)` and `children()` let you pick sections for independently loaded
programs. These section names describe the file structure, rather than routes
to instruction destinations.

A section view gives you the header and source body. Pass it to
`ParsedModule::<SourceInstruction, Type, Header>::parse_section(section)`, using
your composite's syntax instruction type as `SourceInstruction`. You'll get
header declarations and functions with typed instruction bodies.

## Collect declarations before lowering

Use `HeaderDeclarations<Declaration>` to parse the declarations individually.
Your metadata-class enum can include `ProgramMetadata` alongside your own
configuration declarations. Collect them into a state object you write, reject
duplicates, and check required settings before building execution artifacts.
You can prepare the declarations in a different order from how they appear in
the file.

`ProgramMetadata` supplies every non-code field:

- Functions, with their signatures, reserved local counts, and instruction ranges.
- Labels, strings, constants, and source symbols.
- The entry function, if there is one.
- Your application's `extra` metadata.

The order of each table determines its indices. Function names, parameter names,
and filenames refer into the string table. `main_function` selects an entry in
the function table; it isn't an instruction address.

Function ranges count decoded instructions and use an exclusive end address.
A function's local count includes its parameters. The producer supplies any
extra locals; the loader doesn't infer them from instructions.

## Prepare capabilities and lower

`MetadataLoweringContext::new(&metadata)` prepares the standard function, label,
and string capabilities. If custom instructions need more lookup services, you
can extend it or use your own context.

Call `lower_functions_with_metadata(functions, &metadata, &mut context)` to check
the declarations and tables, then lower the bodies. Before lowering each
function, the driver prepares its full local label scope. That's how a branch
can refer to a label farther down the function.

Assemble the result with `metadata.into_module(code)`. It keeps your table order
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
put the whole sequence together. They include an application header, tables in a
different order from the function bodies, a helper call, and a forward branch.

Run it from a checkout:

```sh
cargo run -p vihaco-stdlib --features program-image-execution --example loadable
```

The result is `[42]`. To try another file, pass its source path after `--`.
Once construction finishes, the loader drops the file view and source. The image
owns the runtime instructions and metadata it needs.

[Binary formats](/guide/instructions-advanced) covers loading encoded programs.
