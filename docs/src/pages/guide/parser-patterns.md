---
layout: ../../layouts/Guide.astro
title: Pattern Parser
slug: parser-patterns
description: "Define dialect instruction patterns, lower parsed operands, and derive parsers for metadata, values, and types."
---

# Pattern parser

A pattern describes how a value is written in source and which parts become
Rust fields. For instructions, declare a `#[dialect]` and mark its instruction
structs with `#[instruction]`. The dialect generates their parsing support;
you can customize each spelling with `#[pattern]`.

Use `#[dialect]` rather than independent `#[syntax_class(instruction)]` enums.
For other syntax, such as metadata, values, and types, derive `Parse` and choose
the appropriate `#[syntax_class]` yourself.

Composite parsers add the device route and dialect name to each instruction's
local spelling. See [parsing source](/guide/parser) for how these parsers fit into
loading a program.

## A complete instruction example

This dialect gives us three instructions. Each leaf parser recognizes its local
mnemonic; a composite handles the qualified source name.

```rust
use vihaco::{Parse, chumsky::Parser as _, dialect};

#[dialect]
pub mod memory {
    #[instruction]
    pub struct Halt;

    #[instruction]
    #[pattern = "'load $0"]
    pub struct Load(pub u32);

    #[instruction]
    #[pattern = "'store $0 `,` $1"]
    pub struct Store(pub u32, pub i64);
}

fn main() {
    assert_eq!(memory::Halt::parser().parse("halt").into_result(), Ok(memory::Halt));
    assert_eq!(memory::Load::parser().parse("load 4").into_result(), Ok(memory::Load(4)));
    assert_eq!(
        memory::Store::parser().parse("store 7, -2").into_result(),
        Ok(memory::Store(7, -2)),
    );
}
```

Put `#[pattern]` on the instruction struct inside the dialect. You don't need
a separate parser enum or a manual instruction syntax class.

## Bindings and lowering

Without an annotation, a binding calls its field type's `Parse::parser()`.
An annotation such as `$1:u64` keeps a different source operand type until
`Lower` converts it to the runtime field type:

```rust
use vihaco::{Lower, Parse, Syntax, chumsky::Parser as _, dialect};

#[dialect]
pub mod control {
    #[instruction]
    #[pattern = "'call $0 `,` $1:u64"]
    pub struct Call(pub u32, pub u32);
}

impl<C: ?Sized> Lower<C> for control::Call<Syntax> {
    type Runtime = control::Call;

    fn lower(self, _: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(control::Call(self.0, self.1.try_into()?))
    }
}

fn main() -> eyre::Result<()> {
    let source = control::Call::<Syntax>::parser()
        .parse("call 2, 10").into_result().unwrap();
    assert_eq!(source.lower(&mut ())?, control::Call(2, 10));

    let too_large = control::Call::<Syntax>::parser()
        .parse("call 2, 4294967296").into_result().unwrap();
    assert!(too_large.lower(&mut ()).is_err());
    Ok(())
}
```

Here `$0` uses `u32::parser()`, and `$1:u64` parses a `u64`. The conversion
happens in `Lower`, where it can reject a value that won't fit the runtime
`u32`. Write that impl outside the dialect module, in the same crate. If an
operand needs no conversion, generated lowering carries it through for you.

The `Syntax` spelling in the impl selects the parsed input to `Lower`; the
plain instruction type is its executable output. You don't need to manage the
generated representation behind those types.

Binding indices always identify constructor fields, even if you change capture
order. Named bindings for value and metadata structs can have annotations too,
such as `$address:u64`. A standalone `Parse` derive expects the annotated type
to match the field; it doesn't insert a conversion.

Type aliases, qualified paths, and nested generic arguments use normal Rust
scope. If a type needs whitespace outside delimiters, put it in parentheses.

For an encoded literal, you can give a local newtype a `Parse` implementation.
The parsed instruction keeps that value until `Lower` encodes it. For example,
`I64Literal` can hold a signed integer until lowering turns it into a CPU `Word`,
preserving negative bit patterns. Local conversions can work with any context;
symbol lookup and similar work can require capabilities on the loading context.

## Syntax classes

Instruction declarations get their syntax support from `#[dialect]`. Use the
other syntax classes when deriving parsers for the values and headers around
those instructions:

| Declaration | What it parses | Pattern rules |
|---|---|---|
| `#[instruction]` inside `#[dialect]` | An instruction's local spelling | Start with an instruction token such as `'load`. |
| `#[syntax_class(metadata, head = "device")]` | Metadata in the `device ` namespace | Use bare names such as `module.setting`; instruction tokens aren't required. |
| `#[syntax_class(value)]` | A value expression | No instruction tokens. Simple defaults are available. |
| `#[syntax_class(type)]` | A type expression | No instruction tokens. Every variant or struct needs an explicit pattern. |

Put `#[syntax_class]` on the value, type, or metadata enum or struct, rather than
on a variant or field. A metadata `head` is optional. Leaving it off is useful
for nested records, where you don't want to repeat a declaration prefix.
Write the head without a separator; the parser adds one ASCII space.

For a header made up of lines, you can write a metadata pattern like this:

```rust
use chumsky::Parser as _;

use vihaco_parser::Parse as ParseTrait;

use vihaco_parser_derive::Parse;

#[derive(Debug, PartialEq, Parse)]
#[syntax_class(metadata, head = "device")]
enum DeviceHeader {
    #[pattern = "module.setting $0 `,` $1"]
    ModuleSetting(i64, i64),
}

assert_eq!(
    DeviceHeader::parser().parse("device module.setting 3, 4").into_result(),
    Ok(DeviceHeader::ModuleSetting(3, 4)),
);
```

## Generated patterns

If the default spelling fits, leave out `#[pattern]`. The parser uses the
lowercase Rust name without splitting acronyms: `HttpServer` becomes
`httpserver`.

| Rust declaration | Generated pattern | Local source spelling |
|---|---|---|
| Instruction struct `Halt` | `'halt` | `halt` |
| Instruction struct `Move(i64, bool)` | <code>'move $0 `,` $1</code> | `move 3, true` |
| Value variant `Nothing` | `` `nothing` `` | `nothing` |
| Value variant `Number(i64)` | `$0` | `3` |
| Value struct `Wrapper { value: i64 }` | `$value` | `3` |

Dialect instructions use unit or tuple structs. Use value or metadata types
for richer nested operands and records.

For the other cases, supply a little more information:

- A value with several fields needs a pattern showing how to separate them.
- A type always needs an explicit pattern.
- Unit values and metadata variants default to their lowercase names. A unit
  instruction defaults to its lowercase instruction token.

You can reorder fields in an explicit pattern. Bindings refer to constructor
fields, not capture order, so <code>#[pattern = "$right `,` $left"]</code> still
puts values in the correct fields of a named value or metadata struct.

## Pattern grammar

A pattern consists of atoms separated by spaces:

```text
pattern       = atom, { " ", atom } ;
atom          = instruction-token | metadata-name | binding | literal ;
instruction-token = "'", ascii-identifier ;
metadata-name = ascii-identifier, { ".", ascii-identifier } ;
binding       = "$", (ascii-identifier | decimal-index) ;
literal       = "`", (ascii-identifier | "," | "@" | ":" | ";" | "{" | "}" | "[" | "]" | "(" | ")"), "`" ;
```

Each atom has a specific job:

| Form | Purpose | Example |
|---|---|---|
| `'name` | Match an instruction mnemonic. It is not included in the constructed value. | `'load` |
| `name` or `name.part` | Match a metadata name. It is not included in the constructed value. | `module.setting` |
| `$0`, `$1`, … | Parse and capture a tuple field by zero-based index. | `$1` |
| `$field` | Parse and capture a named field. Raw Rust identifiers bind by their unraw name, so `r#type` uses `$type`. | `$address` |
| `` `word` `` | Match an exact, case-sensitive keyword. | `` `before` `` |
| `` `,` `` | Match a comma with punctuation-aware spacing. | <code>$0 `,` $1</code> |
| `` `@` `` | Match an at sign with punctuation-aware spacing. | <code>$0 `@` $1</code> |

Punctuation literals support commas, at signs, colons, semicolons, braces,
brackets, and parentheses. Metadata names can also contain dots without
backticks. Other punctuation and quoted strings aren't pattern literals.
To parse a quoted source string, use a `String` or `QuotedString` field.

## Whitespace is part of the grammar

There are two sets of spacing rules to keep in mind: how you write the pattern
in Rust, and what the parser accepts in source.

Inside the Rust string, separate pattern atoms with exactly one ASCII space.
Leading, trailing, or repeated spaces and tab separators are compile-time errors.

In source, an ordinary boundary accepts one or more ASCII spaces, but no tabs
or newlines. Commas and at signs adjust one side of that boundary:

- `` `,` `` removes the space before itself but requires whitespace after it.
  <code>$0 `,` $1</code> accepts `1, true`, but rejects `1 , true` and `1,true`.
- `` `@` `` requires whitespace before itself but removes it after itself.
  <code>$0 `@` $1</code> accepts `1 @target`, but rejects `1@target` and
  `1 @ target`.

Keywords and symbols match exactly. Ordinary pattern boundaries don't consume
indentation, line endings, or comments; the surrounding module or list parser
handles those.

The punctuation literals `:`, `;`, `{`, `}`, `[`, `]`, `(`, and `)` accept
optional whitespace and line comments on either side. Delimited metadata
records use these rules too. Commas and at signs keep the spacing rules above.

## Delimited metadata records

For a named struct, you can generate a key/value record parser without writing
a pattern:

```text
#[derive(Parse)]
#[syntax_class(metadata, head = "metadata", open = "{", close = "}")]
struct MetadataHeader {
    functions: Vec<FunctionMetadata>,
    main_function: Option<u32>,
}

#[derive(Parse)]
#[syntax_class(metadata, open = "{", close = "}")]
struct FunctionMetadata {
    name: u32,
    local_count: u32,
}
```

You can omit `head` for nested records. Supply `open` and `close` together;
they must be nonempty and different. This form requires a named struct and
can't also have a `#[pattern]`.

Each Rust field becomes a bare or double-quoted key, followed by `:` and a value
parsed with the field type's `Parse` impl. Fields can appear in any order.
Commas separate entries, and the parser accepts trailing commas and `//` comments.
Arrays use brackets and accept comments and trailing commas too.

Unknown, duplicate, and missing keys are errors. That includes `Option<T>` fields:
write `null` for `None`. Leaving the key out is still an error.

The record parser leaves a terminating semicolon to the surrounding
header-declaration parser. That lets metadata records and `device ...;`
declarations share a header.

## Field-shape invariants

The derive checks that each capture has a clear destination in the Rust
constructor:

- Tuple structs and tuple variants use numeric bindings only.
- Named structs use named bindings only.
- A pattern can't mix the two styles.
- Every field must appear exactly once; missing or duplicate bindings are errors.
- Numeric indices must be in bounds, and named bindings must name real fields.
- Unit structs and variants can't have bindings.

You can change the source order of fields while keeping their destinations
unambiguous:

```rust
use chumsky::Parser as _;

use vihaco_parser_derive::Parse;

use vihaco_parser::Parse as ParseTrait;

#[derive(Debug, PartialEq, Parse)]
#[syntax_class(value)]
#[pattern = "$right `,` $left"]
struct Pair {
    left: i64,
    right: bool,
}

assert_eq!(
    Pair::parser().parse("true, 42").into_result(),
    Ok(Pair {
        left: 42,
        right: true,
    }),
);
```

## Enums, structs, and dispatch

For instructions, use the unit and tuple structs declared in your dialect.
Let the composite generate the parser that selects among its available
instructions.

For value, type, and metadata parsers, you can derive `Parse` on enums or structs:

- On an enum, put `#[pattern]` on each variant whose pattern you want to override.
- On a struct, put its single `#[pattern]` on the struct definition.
- Enum variants can be unit, tuple, or struct-style. Tuple variants use numeric
  bindings; struct-style variants use named bindings.
- Generic types work too.

Value and type alternatives keep their declaration order, so order matters when
several alternatives can parse the same input.

## Compile-time validation

Where possible, errors point to the `#[pattern = "..."]` string. Validation
happens in stages, so the first error usually describes the earliest rule that
needs attention:

| Stage | Examples of rejected input |
|---|---|
| Pattern syntax | Empty patterns; leading, trailing, or repeated spaces; tabs; malformed bindings; unsupported symbols; unterminated literals; indices larger than `u32`. |
| Syntax class | Missing `#[syntax_class]` on a standalone value/type/metadata derive; an instruction pattern not beginning with `'name`; instruction tokens in value/type patterns; implicit type patterns; implicit multi-field value patterns. |
| Field mapping | Mixed binding styles; named bindings on tuple fields; indexed bindings on named fields; missing, duplicate, unknown, or out-of-bounds fields; bindings on unit forms. |
| Attribute placement | Instruction helpers outside a dialect's marked structs; `#[pattern]` without its owning derive or declaration; `#[syntax_class]` on a variant or field instead of a value/type/metadata definition. |

Rust's type checker then checks that every captured field type implements
`Parse<'src>`.

## Compose nested syntax

Patterns let you keep exact mnemonics, keywords, typed field parsers, commas,
and `@` in one string, checked against the Rust constructor.

When a field needs a richer grammar, give it another local enum or struct with
its own pattern derive. `Ident`, `BareToken`, and `QuotedString` cover lexical
leaves; `Vec<T>` and tuples cover collections. Each binding uses its field's
parser, so the complete grammar stays typed and pattern-derived.

For module parsing, typed function bodies, resolution, and explicit source
sugar, see [loading SST programs](/guide/parser-advanced).
