---
layout: ../../layouts/Guide.astro
title: Pattern Parser Generator
slug: parser-patterns
description: "Generate Parse implementations from declarative syntax patterns for instruction, value, and type enums and structs."
---

# Pattern Parser Generator

The pattern generator is the recommended way to derive source parsers for new
syntax types. A pattern describes the concrete spelling of a value and binds
parts of that spelling to Rust fields. The derive validates the description at
compile time, then emits an ordinary `vihaco_parser_core::Parse` implementation.

Use two attributes:

- `#[syntax_class(...)]` on the enum or struct selects the role of the syntax.
- `#[pattern = "..."]` on an enum variant, or on a struct itself, overrides the
  generated pattern.

The older `#[head]`, `#[token]`, `#[delimiters]`, `#[delegate]`, and
`#[parse_with]` generator remains supported. It is documented in
[Parser Integration](/guide/parser), but its attributes cannot be mixed with
the pattern generator on one type.

## A complete instruction example

```rust
use chumsky::Parser as _;
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

#[derive(Debug, PartialEq, Parse)]
#[syntax_class(instruction, head = "memory")]
enum MemoryInstruction {
    Halt,
    #[pattern = "'load $0"]
    Load(u32),
    #[pattern = "'store $0 `,` $1"]
    Store(u32, i64),
}

let halt = MemoryInstruction::parser()
    .parse("memory::halt")
    .into_result();
assert_eq!(halt, Ok(MemoryInstruction::Halt));

let load = MemoryInstruction::parser()
    .parse("memory::load 4")
    .into_result();
assert_eq!(load, Ok(MemoryInstruction::Load(4)));

let store = MemoryInstruction::parser()
    .parse("memory::store 7, -2")
    .into_result();
assert_eq!(
    store,
    Ok(MemoryInstruction::Store(7, -2)),
);
```

The `head` is a dialect namespace. The derive appends `::`, so
`head = "memory"` combines with the pattern token `'load` to accept
`memory::load`.

Every bound field is parsed with that field type's
`vihaco_parser_core::Parse::parser()`. Pattern mode has no field-level
`#[parse_with]` escape hatch: implement `Parse` for a local field type, wrap a
foreign type in a local newtype, or use the legacy generator when a field needs
a one-off custom parser.

## Syntax classes

Every type using pattern generation must declare a syntax class.

| Attribute | Meaning | Additional rules |
|---|---|---|
| `#[syntax_class(instruction, head = "dialect")]` | An instruction in the `dialect::` namespace. | Every pattern starts with an instruction token such as `'load`. |
| `#[syntax_class(value)]` | A value expression. | Instruction tokens are forbidden. Simple defaults are available. |
| `#[syntax_class(type)]` | A type expression. | Instruction tokens are forbidden and every variant or struct needs an explicit pattern. |

Put `#[syntax_class]` on the enum or struct definition, never on a variant or
field. An instruction head is required and is written without trailing `::`.

## Generated patterns

You can omit `#[pattern]` when the conventional syntax is sufficient. Names
are the lowercase Rust variant or struct name; acronym boundaries are not
split (`HttpServer` becomes `httpserver`).

| Rust shape | Generated pattern | Accepted source |
|---|---|---|
| instruction `Halt` | `'halt` | `dialect::halt` |
| instruction `Move(i64, bool)` | <code>'move $0 `,` $1</code> | `dialect::move 3, true` |
| instruction struct `Set { x: i64, enabled: bool }` | <code>'set $x `,` $enabled</code> | `dialect::set 3, true` |
| value `Nothing` | `` `nothing` `` | `nothing` |
| value `Number(i64)` | `$0` | `3` |
| value `Wrapper { value: i64 }` | `$value` | `3` |

Defaults intentionally stop there:

- A value with more than one field must spell out how those fields are
  separated.
- A type always requires an explicit pattern.
- A unit value defaults to its lowercase name, while a unit instruction
  defaults to its lowercase instruction token.

Explicit patterns may reorder fields. Bindings identify constructor fields,
not capture order, so <code>#[pattern = "$right `,` $left"]</code> still constructs a
named type using the correct field names.

## Pattern grammar

The complete grammar is small:

```text
pattern       = atom, { " ", atom } ;
atom          = instruction-token | binding | literal ;
instruction-token = "'", ascii-identifier ;
binding       = "$", (ascii-identifier | decimal-index) ;
literal       = "`", (ascii-identifier | "," | "@"), "`" ;
```

The atoms mean:

| Form | Purpose | Example |
|---|---|---|
| `'name` | Match an instruction mnemonic. It is not included in the constructed value. | `'load` |
| `$0`, `$1`, … | Parse and capture a tuple field by zero-based index. | `$1` |
| `$field` | Parse and capture a named field. Raw Rust identifiers bind by their unraw name, so `r#type` uses `$type`. | `$address` |
| `` `word` `` | Match an exact, case-sensitive keyword. | `` `before` `` |
| `` `,` `` | Match a comma with punctuation-aware spacing. | <code>$0 `,` $1</code> |
| `` `@` `` | Match an at sign with punctuation-aware spacing. | <code>$0 `@` $1</code> |

Only comma and at sign are currently supported as symbol literals. Arbitrary
punctuation and quoted strings are not part of the pattern language.

## Whitespace is part of the grammar

Pattern atoms must be separated by exactly one ASCII space inside the Rust
string. Leading spaces, trailing spaces, repeated spaces, and tab separators
are compile-time errors.

In source text, a normal boundary between atoms accepts one or more ASCII
spaces—not tabs or newlines. Symbol literals adjust just one boundary so common
punctuation looks natural:

- `` `,` `` suppresses whitespace before itself but still requires whitespace
  after itself: <code>$0 `,` $1</code> accepts `1, true`, not `1 , true` or `1,true`.
- `` `@` `` still requires whitespace before itself but suppresses whitespace
  after itself: <code>$0 `@` $1</code> accepts `1 @target`, not `1@target` or
  `1 @ target`.

Keywords and symbols match exactly. The generated parser does not consume
indentation, line endings, or comments; the surrounding module/list parser is
responsible for those boundaries.

## Field-shape invariants

The derive treats a pattern as a checked mapping from source captures to a
Rust constructor:

- Tuple structs and tuple variants use only numeric bindings.
- Named structs use only named bindings.
- A pattern cannot mix numeric and named bindings.
- Every field appears exactly once. Missing and duplicate bindings are errors.
- Numeric bindings must be in bounds; named bindings must name a real field.
- Unit structs and variants cannot contain bindings.

These rules allow fields to appear in any source order without making
construction ambiguous.

```rust
use chumsky::Parser as _;
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

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

Pattern generation supports both enums and structs:

- On an enum, put `#[pattern]` on each variant that needs an override.
- On a struct, put its single `#[pattern]` on the struct definition.
- Enum variants may be tuple or unit variants. Struct-style enum variants are
  not supported; use a tuple variant containing a named struct when named
  fields are useful.
- Generic types and types that already use a lifetime named similarly to the
  derive's internal lifetime are supported.

For instruction enums, alternatives are ordered by mnemonic length before
emission. This prevents a short token such as `v2` from consuming the prefix of
`v25`. Pattern-generated enums also support more than chumsky's 26-element
tuple-choice limit; the derive groups large enums into shallow nested choices.
Alternatives with equal-length instruction tokens, and value/type alternatives
without instruction tokens, retain declaration order.

## Compile-time validation

Errors point at the `#[pattern = "..."]` literal when possible. Validation runs
in stages, so failures tend to describe the earliest broken contract:

| Stage | Examples of rejected input |
|---|---|
| Pattern syntax | Empty patterns; leading, trailing, or repeated spaces; tabs; malformed bindings; unsupported symbols; unterminated literals; indices larger than `u32`. |
| Syntax class | Missing `#[syntax_class]`; instruction pattern not beginning with `'name`; instruction tokens in value/type patterns; implicit type patterns; implicit multi-field value patterns. |
| Field mapping | Struct-style enum variants; mixed binding styles; named bindings on tuple fields; indexed bindings on named fields; missing, duplicate, unknown, or out-of-bounds fields; bindings on unit forms. |
| Generator selection | Any legacy parser attribute combined with `#[syntax_class]`; `#[pattern]` used without a syntax class; `#[syntax_class]` placed on a variant or field instead of the type definition. |

The generated parser then relies on Rust's type checker to verify that every
captured field type implements `Parse<'src>`.

## Choosing between pattern and legacy generation

Use patterns when the syntax can be expressed as exact mnemonics/keywords,
typed field parsers, comma, and `@`. Patterns are especially useful when you
want the source grammar visible in one string and checked against the Rust
constructor.

Use the legacy generator when a field needs `#[parse_with]`, a variant delegates
to another parser, or delimiters outside the pattern grammar are required.
Choose one generator for the entire enum or struct: placing
`#[syntax_class(...)]` selects pattern generation, while omitting it selects
legacy enum generation.

For module parsing, fallback raw forms, symbol resolution, and sugar expansion,
continue to [Advanced Parser Customization](/guide/parser-advanced).
