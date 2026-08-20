---
layout: ../../layouts/Guide.astro
title: Pattern Parser Integration
slug: parser
description: "Derive strict, typed chumsky parsers with syntax classes and declarative patterns."
---

# Pattern Parser Integration for Component Instructions

Source syntax is defined with the pattern parser, which is split across two
crates:

1. **`vihaco-parser`** defines `Parse<'src>` and `SurfaceInstruction`, and
   implements `Parse` for common lexical, primitive, and collection field
   types.
2. **`vihaco-parser-derive`** provides `#[derive(Parse)]`, which generates a
   [chumsky](https://github.com/zesterer/chumsky) parser from
   `#[syntax_class]` and `#[pattern]`.

The generated parser is strict: every accepted source form must be described
by the Rust syntax type. Unknown or malformed input is an error.

If you are new to instruction enums, read
[Defining Instructions With `vihaco`](/guide/instructions) first.

## A complete instruction parser

```rust
use chumsky::Parser as _;
use vihaco::Instruction;
use vihaco_parser_derive::Parse;
use vihaco_parser::Parse as ParseTrait;

#[derive(Debug, Clone, PartialEq, Instruction, Parse)]
#[instruction(width = 8)]
#[syntax_class(instruction, head = "counter")]
enum CounterInstruction {
    #[pattern = "'add $0"]
    Add(i64),
    Print,
}

assert_eq!(
    CounterInstruction::parser()
        .parse("counter.add -5")
        .into_result(),
    Ok(CounterInstruction::Add(-5)),
);
assert_eq!(
    CounterInstruction::parser()
        .parse("counter.print")
        .into_result(),
    Ok(CounterInstruction::Print),
);
```

The two derives are independent:

- `Instruction` defines bytecode encoding and runtime opcode behavior.
- `Parse` defines source syntax.

`#[syntax_class(instruction, head = "counter")]` places every instruction in
the `counter::` namespace. A unit variant receives a conventional lowercase
pattern automatically. `#[pattern = "'add $0"]` spells out the mnemonic and
binds the first tuple field. The derive also implements
`SurfaceInstruction` for an instruction-class enum.

## The `Parse` trait

```rust ignore
pub trait Parse<'src>: Sized {
    fn parser() -> impl chumsky::Parser<
        'src,
        &'src str,
        Self,
        chumsky::extra::Err<chumsky::error::Simple<'src, char>>,
    >;
}
```

`vihaco-parser` implements `Parse` for common primitives:

| Type | Accepted form |
|---|---|
| `u32`, `u64`, `usize` | Decimal digits without a sign |
| `i32`, `i64` | Optional leading `-`, then digits |
| `f32`, `f64` | Optional `-`, decimal fraction, and exponent |
| `bool` | `true` or `false` |
| `Ident` | An unquoted identifier without a leading `@`; dots and colons are accepted |
| `BareToken` | An unquoted token whose interpretation is deferred |
| `QuotedString` | A double-quoted string with common backslash escapes |
| `Vec<T>` | Comma-separated values inside `[...]` |
| `(A, B)` | A pair inside `(a, b)` |

`String` intentionally does not implement `Parse`: it has no single canonical
source spelling. Choose a lexical newtype that describes the field's grammar,
or define a domain-specific enum or struct that also derives `Parse`.

## Syntax classes

Every derived parser declares exactly one syntax class:

| Attribute | Role |
|---|---|
| `#[syntax_class(instruction)]` | An unnamespaced instruction such as `load`. |
| `#[syntax_class(instruction, head = "dialect")]` | A namespaced instruction such as `dialect.load` |
| `#[syntax_class(metadata, head = "device")]` | A line-oriented metadata record such as `device module.setting ...` |
| `#[syntax_class(value)]` | A value expression |
| `#[syntax_class(type)]` | A type expression with an explicit pattern |

Put the attribute on the enum or struct definition. Instruction heads omit the
trailing separator; the derive supplies `.`. Metadata heads omit their trailing
space; the derive supplies that separator instead.

The `head` argument is optional for instruction syntax. Without it, the parser
consumes only the instruction pattern itself. `component!` supplies the
component's snake_case name as the dialect head; `#[composite]` adds the device
field name (or one of its aliases) as an outer head when it builds the
composite parser.

## Patterns

A pattern is a space-separated sequence of:

- `'mnemonic` for an instruction token;
- `$0`, `$1`, … for tuple fields;
- `$field` for named fields; and
- backtick literals such as `` `,` ``, `` `@` ``, or `` `before` ``.

For example:

```rust
use chumsky::Parser as _;
use vihaco_parser_derive::Parse;
use vihaco_parser::{Ident, Parse as ParseTrait};

#[derive(Debug, PartialEq, Parse)]
#[syntax_class(instruction, head = "control")]
enum ControlInstruction {
    #[pattern = "'branch `@` $0"]
    Branch(Ident),
    #[pattern = "'select $0 `,` $1"]
    Select(bool, u32),
}

assert_eq!(
    ControlInstruction::parser()
        .parse("control.branch @done")
        .into_result(),
    Ok(ControlInstruction::Branch(Ident("done".to_owned()))),
);
assert_eq!(
    ControlInstruction::parser()
        .parse("control.select true, 3")
        .into_result(),
    Ok(ControlInstruction::Select(true, 3)),
);
```

See [Pattern Parser](/guide/parser-patterns) for the complete grammar,
generated defaults, whitespace rules, validation, structs, and large enums.

## Nested field syntax

Every pattern binding uses the field type's pattern-derived parser. Define a
local enum or struct when a field has its own domain syntax; this also keeps
foreign types behind an application-owned syntax boundary.

```rust
use chumsky::Parser as _;
use vihaco_parser_derive::Parse;
use vihaco_parser::{Ident, Parse as ParseTrait};

#[derive(Debug, PartialEq, Parse)]
#[syntax_class(value)]
#[pattern = "$0"]
struct Address(Ident);

#[derive(Debug, PartialEq, Parse)]
#[syntax_class(instruction, head = "control")]
enum ControlInstruction {
    #[pattern = "'branch `@` $0"]
    Branch(Address),
}

assert_eq!(
    ControlInstruction::parser()
        .parse("control.branch @done")
        .into_result(),
    Ok(ControlInstruction::Branch(Address(Ident(
        "done".to_owned()
    )))),
);
```

Pattern-derived types compose recursively, so a field can be a lexical
newtype, another syntax enum or struct, `Vec<T>`, or a tuple.

## What comes next

- For section headers, functions, typed bodies, and `Resolve`, see
  [Module Parsing and Resolution](/guide/parser-advanced).
- To attach an instruction type to a component through `dispatch`, see
  [Building Components With `vihaco`](/guide/components).
