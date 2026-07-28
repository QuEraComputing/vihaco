---
layout: ../../layouts/Guide.astro
title: Parser Integration
slug: parser
description: "Derive strict, typed chumsky parsers with syntax classes and declarative patterns."
---

# Parser Integration for Component Instructions

Source parsing has two crates:

1. **`vihaco-parser-core`** defines `Parse<'src>` and implements it for common
   primitive field types.
2. **`vihaco-parser`** provides `#[derive(Parse)]`, which generates a
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
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

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
        .parse("counter::add -5")
        .into_result(),
    Ok(CounterInstruction::Add(-5)),
);
assert_eq!(
    CounterInstruction::parser()
        .parse("counter::print")
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
binds the first tuple field.

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

`vihaco-parser-core` implements `Parse` for common primitives:

| Type | Accepted form |
|---|---|
| `u32`, `u64`, `usize` | Decimal digits without a sign |
| `i32`, `i64` | Optional leading `-`, then digits |
| `f32`, `f64` | Optional `-`, decimal fraction, and exponent |
| `bool` | `true` or `false` |
| `String` | An identifier-shaped token, stopping at whitespace or structural punctuation |

The free `vihaco_parser_core::ident()` parser accepts non-whitespace characters
except `, ; ( ) { } [ ]`. It is useful when hand-writing `Parse` for a local
field type.

## Syntax classes

Every derived parser declares exactly one syntax class:

| Attribute | Role |
|---|---|
| `#[syntax_class(instruction, head = "dialect")]` | A namespaced instruction such as `dialect::load` |
| `#[syntax_class(value)]` | A value expression |
| `#[syntax_class(type)]` | A type expression with an explicit pattern |

Put the attribute on the enum or struct definition. Instruction heads omit the
trailing `::`; the derive supplies it.

## Patterns

A pattern is a space-separated sequence of:

- `'mnemonic` for an instruction token;
- `$0`, `$1`, … for tuple fields;
- `$field` for named fields; and
- backtick literals such as `` `,` ``, `` `@` ``, or `` `before` ``.

For example:

```rust
use chumsky::Parser as _;
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

#[derive(Debug, PartialEq, Parse)]
#[syntax_class(instruction, head = "control")]
enum ControlInstruction {
    #[pattern = "'branch `@` $0"]
    Branch(String),
    #[pattern = "'select $0 `,` $1"]
    Select(bool, u32),
}

assert_eq!(
    ControlInstruction::parser()
        .parse("control::branch @done")
        .into_result(),
    Ok(ControlInstruction::Branch("done".into())),
);
assert_eq!(
    ControlInstruction::parser()
        .parse("control::select true, 3")
        .into_result(),
    Ok(ControlInstruction::Select(true, 3)),
);
```

See [Pattern Parser Generator](/guide/parser-patterns) for the complete grammar,
generated defaults, whitespace rules, validation, structs, and large enums.

## Custom field syntax

Every pattern binding calls the field type's `Parse::parser()`. When a field
needs custom syntax, define a local type and implement `Parse` for it. A
newtype also solves Rust's orphan-rule restriction for foreign types.

```rust ignore
use chumsky::Parser as _;
use vihaco_parser_core::Parse;

struct Address(String);

impl<'src> Parse<'src> for Address {
    fn parser() -> impl chumsky::Parser<
        'src,
        &'src str,
        Self,
        chumsky::extra::Err<chumsky::error::Simple<'src, char>>,
    > {
        vihaco_parser_core::ident().map(Address)
    }
}
```

If a whole type needs grammar beyond declarative patterns, implement `Parse`
for that type with ordinary chumsky combinators.

## What comes next

- For section headers, functions, typed bodies, and `Resolve`, see
  [Advanced Parser Customization](/guide/parser-advanced).
- To attach an instruction type to a component, see
  [Building Components With `vihaco`](/guide/components).
