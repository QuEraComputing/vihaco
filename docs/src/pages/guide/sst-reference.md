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

Emit `sst v1` first, followed by an optional `.global:` / `.global.` block and
exactly one root section named `root`. Put structural markers on their own
lines. This is a complete program for a machine with an `execution` device
using the standard program dialect:

```text
sst v1
.section(root):
    .header(root):
        metadata {
            strings: ["main", "output.sst"],
            functions: [
                {
                    name: 0,
                    signature: { params: [], ret: [u64] },
                    locals: [],
                    start_address: 0,
                    end_address: 2,
                    file: 1,
                },
            ],
            structs: [],
            types: [],
            labels: [],
            constants: [],
            main_function: 0,
            source_symbols: [],
            extra: {},
        };
    .header(root).
    .text(root):
        fn @main() -> u64 {
            execution::program::const_u64 42
            execution::program::ret 1
        }
    .text(root).
.section(root).
```

Each section may contain one header block, one text block, and nested
`.section(child):` / `.section(child).` blocks. Header and text marker names
must match their containing section's local name. Sibling section names must
be unique. A section without header or text yields an empty block to the
consumer; a runnable program still needs the content its loader requires.

Sections identify independently loadable units, not instruction destinations.
All program tables and instruction addresses below are local to the selected
section. A `.global` block has a consumer-defined grammar; omit it for loaders
using `NoContext`. Structural framing is line-based; use `//` comments inside
headers and function bodies rather than appending them to marker lines.

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
| `structs` | An ordered array of struct declarations, described below. Array position is the struct index that `struct N` types refer to. Emit `[]` when the program declares none. |
| `types` | An ordered array of named-type declarations, described below. Array position is the index that `type N` types refer to. Emit `[]` when the program declares none. |
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
| `locals` | The declared type of every local slot, parameters first: `[u64, f64]`. The leading entries must equal the parameter types, so there are at least as many locals as parameters; the loader does not infer extra locals. At most 65,536 entries (`MAX_LOCALS`). |
| `start_address` | Index of the first instruction in this function's body in the concatenated code vector. |
| `end_address` | Exclusive end of that body's instruction range. |
| `file` | Index of the source filename in `strings`. |

Each struct record is `{ name: string_index, fields: [{ name: string_index, ty: type }, ...] }`.
Struct names must be unique, and field names must be unique within a struct.
Field order defines the slot layout: field `i` is slot `i` of the heap object.
The CPU instructions name the struct, by name or by table index:

| Instruction | Effect |
|---|---|
| `struct_new @Point` or `struct_new 0` | Pop one value per field (pushed in declaration order) and push a reference to a new `Point`. |
| `get_field @Point.y` or `get_field 0, 1` | Pop a `Point` reference and push field `y`. |
| `set_field @Point.y` or `set_field 0, 1` | Pop a value, then a `Point` reference, and overwrite field `y`. |

Lowering resolves every form against this table: the field count of
`struct_new` always comes from the table, and an unknown struct, field, or slot
is a load-time error. (The legacy CPU instruction enum has no lowering step and
takes `struct_new 0, 2` and `get_field 0, 1` with explicit numbers. Those are
checked against the struct table only if the embedder calls
`vihaco_cpu::check_struct_operands` when assembling the module; otherwise the
CPU trusts the written field count.)

Heap objects are type-checked at runtime. Each object records what it is: an
array with a specific element type, or a specific struct. `get_field` and
`set_field` fail unless the reference is to a live struct of the field's struct
type, and the array instructions reject structs, so a struct's shape never
changes. Field values themselves are untyped words; the CPU does not check that
a value stored in an `f64` field is an `f64`.

Heap references are opaque 64-bit values. A reference to a freed object stays
invalid after its slot is reused (for up to 2^31 reuses of that slot), so it
does not reach the new object. References are predictable rather than secret,
so the typed dialects have no instruction that writes one as a literal: use the
reference an allocating instruction returned, or one the host passed in. (The
legacy enum build still has `const_heap_ref N`; runtime tag checks apply to
whatever word it pushes.)

Types in signatures and fields may name a struct as `struct N`, its index in
`structs`. At runtime such a value is a heap reference to a `struct N` object.
Module assembly rejects `struct N` indices outside the table, including ones
nested in array types. Structs may refer to themselves or to each other, for
example a linked node with a `next: struct 0` field.

Every type position also accepts a struct by name, `struct @Point`, and a named
type as `type @location`: `fn` headers, `signature`, `locals`, struct fields,
and instruction operands. This is the default on the standard loading path:
`ProgramMetadata`'s type parameter defaults to `vihaco::module::SourceType`,
and the loader parses `fn` lines with `SourceType` too. Instruction operands
are resolved during lowering; the header is resolved against its own tables
with `ProgramMetadata::resolve_types` (which `ParsedModule::lower_with_metadata`
does itself):

```rust,ignore
let ParsedModule { header, functions } =
    ParsedModule::<Syntax, SourceType, HeaderDeclarations<ProgramMetadata>>
        ::parse_section(file.root())?;
let metadata = header.into_iter().next().unwrap().resolve_types::<Type>()?;
let mut context = MetadataLoweringContext::new(&metadata)?;
let code = lower_functions_with_metadata(functions, &metadata, &mut context)?;
let module = metadata.into_module(code)?;
```

A loader that parses `ProgramMetadata<Value, Type>` (or `fn` lines with
`Type`) accepts only the numeric forms (`struct 0`, `type 0`); a name there is
a parse error whose message says to parse with `SourceType`.

An unknown name is reported at its metadata path, for example
``metadata.functions[0].locals[1]: undefined struct `@Pointt` ``. Binary bytecode
always stores indices.

### Arrays

Arrays are typed by their element: `array f64`, `array struct 0`, or nested,
`array array u64`. The type can appear anywhere a type can, such as a function
return type or a struct field. Types nest at most 32 levels deep
(`Type::MAX_ARRAY_DEPTH`): a deeper type is a parse or decode error that names
its depth, and lowering rejects an array instruction whose element type is
already 32 levels deep, because its array type would be 33. Elements are stored
inline, one word each; struct and array elements are references to their own
heap objects.

| Instruction | Effect |
|---|---|
| `array_new T, N` | Pop `N` values of type `T` (pushed in order) and push a reference to a full `array T`. |
| `array_reserve T` | Pop a `u64` capacity and push a reference to an empty `array T`. |
| `array_push T` | Pop a value and an `array T` reference, append the value (below capacity), and push the reference back. |
| `array_get T` | Pop a `u64` index and an `array T` reference and push that element. |
| `array_set T` | Pop a value, a `u64` index, and an `array T` reference, and overwrite that element (index below the current length). |
| `array_len` | Pop a reference to an array of any element type and push its length as `u64`. |
| `heap_dealloc` | Pop a reference to any heap object and free it. |

Every array instruction checks the object at runtime: it must be a live array
whose element type is exactly `T`, so `array_get u64` on an `array f64` fails
with `is array f64, expected array u64`. Program-image execution also reports
where, and names structs by name (a `StepError` printed with `{:#}`):

```text
at @main+3 (address 3) `cpu::cpu::array_get u64`: type error: heap object (slot 2, generation 1) is array f64, expected array u64
```

Indices are always `u64` and are
bounds-checked against the current length. Element types may name structs,
`array_new struct @Point, 4`, resolved through the struct table during lowering;
an unknown struct name or index is a load-time error.

```text
metadata {
    strings: ["main", "test.sst", "Point", "x", "y"],
    functions: [/* ... */],
    structs: [
        { name: 2, fields: [{ name: 3, ty: f64 }, { name: 4, ty: i64 }] },
    ],
    // ... remaining tables
};
```

### Named types

A named type is a nominal type represented by a scalar word. Each record in
`types` is `{ name: string_index, repr: type }`, where `repr` is one of `i32`,
`i64`, `u32`, `u64`, `f32`, `f64`, or `bool`, and names are unique. Types
anywhere in the header, including array elements and struct fields, may refer
to entry `N` as `type N`; module assembly rejects indices outside the table.

At runtime a named-type value is the same word as its representation. The
name only matters for verification, where `type N` satisfies only itself:
not its representation, and not another named type with the same
representation. Two CPU instructions convert explicitly, and do nothing at
runtime:

| Instruction | Effect |
|---|---|
| `wrap @location` or `wrap 0` | Reinterpret the top word, of the representation type, as the named type. |
| `unwrap @location` or `unwrap 0` | Reinterpret the top word, of the named type, as its representation. |

Type operands of the array instructions accept `type @location` as well as
`type 0`; lowering resolves names against `types`, and an unknown name or index
is a load-time error. The same holds for the types in a `call_indirect`
signature, which may also name structs:
`call_indirect (struct @Point, type @location) -> u64`. Binary bytecode always
stores the resolved indices.

```text
metadata {
    strings: ["main", "lanes.sst", "location", "lane"],
    types: [
        { name: 2, repr: u64 },
        { name: 3, repr: u64 },
    ],
    // ... remaining tables
};
```

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

String literals, here and in instructions such as `const_string`, take any
character raw except `"` and `\`, and the escapes `\"`, `\\`, `\n`, `\t`, `\r`
and `\0`. Float literals in instructions (`const_f64 -0.0`, `const_f64 2.5e-3`)
have no spelling for infinity or NaN: write an infinity as an overflowing
literal such as `1e999` or `-1e999`, which is how instruction text shows one.
A NaN operand cannot be written in instruction text.

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
`SstFile::from_text`, `ParsedModule::parse_section`, declaration preparation,
`MetadataLoweringContext::new`, `lower_functions_with_metadata`, then
`metadata.into_module(code)`. This checks function declarations against the
supplied tables, verifies source labels and instruction ranges, resolves symbols,
and checks standard constant references. Loading preserves table order and does
not invent missing metadata or select an entry function for you.

For the minimal file above, this loader validates and assembles the program:

```rust
use vihaco::{HasInstructionSet, NoContext, SstFile, Type, composite};
use vihaco::loader::MetadataLoweringContext;
use vihaco::module::{ProgramMetadata, SourceType};
use vihaco::syntax::{HeaderDeclarations, ParsedModule, lower_functions_with_metadata};

#[composite]
struct Machine {
    #[device(0)]
    execution: vihaco_stdlib::ExecutionState,
}

fn validate(source: &str) -> eyre::Result<()> {
    type SourceInstruction = <Machine as HasInstructionSet>::Syntax;
    let file = SstFile::<NoContext>::from_text(source)?;
    let ParsedModule { header, functions } = ParsedModule::<
        SourceInstruction, SourceType, HeaderDeclarations<ProgramMetadata>
    >::parse_section(file.root())?;
    let mut declarations = header.into_iter();
    let metadata = declarations
        .next()
        .ok_or_else(|| eyre::eyre!("missing metadata"))?
        .resolve_types::<Type>()?;
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
