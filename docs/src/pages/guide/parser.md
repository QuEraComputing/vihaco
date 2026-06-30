---
layout: ../../layouts/Guide.astro
title: Parser Integration
slug: parser
description: "Derive a chumsky parser for an instruction enum with #[derive(vihaco_parser::Parse)] — the head/token/delimiters/parse_with attributes and the Parse trait."
---

# Parser Integration for Component Instructions

The parser pipeline has two layers:

1. **`vihaco-parser-core`** — defines the `Parse<'src>` trait and supplies blanket impls for primitives (`i64`, `u64`, `f64`, `bool`, `String`, …). Every parser in the workspace is just a `Parse` impl.
2. **`vihaco-parser`** — proc-macro crate. The `#[derive(Parse)]` derive turns an enum into a `chumsky::Parser` that tries each variant in declaration order.

If you are new to instruction enums, read [Defining Instructions With `vihaco`](/guide/instructions) first. This guide picks up where instruction definitions end and teaches the parser how to accept your source syntax.

For most new work the flow is:

1. Add `#[derive(vihaco_parser::Parse)]` to your `#[derive(Instruction)]` enum.
2. Annotate the enum with `#[head]` (optional) and each variant with `#[token]` / `#[delimiters]` / `#[parse_with]` as needed.
3. Call `<MyInstruction as Parse>::parser()` to obtain a `chumsky::Parser`.

That's the whole instruction-level integration. Module-level orchestration (headers, function bodies, sugar, labels) is covered in [Advanced Parser Customization](/guide/parser-advanced).

## The `Parse` trait

```rust ignore
pub trait Parse<'src>: Sized {
    fn parser() -> impl chumsky::Parser<'src, &'src str, Self, extra::Err<Simple<'src, char>>>;
}
```

`vihaco-parser-core` already implements `Parse` for the common primitives:

| Type | Accepted form |
|---|---|
| `u32`, `u64`, `usize` | Decimal digits (no sign) |
| `i32`, `i64` | Optional leading `-`, then digits |
| `f32`, `f64` | Optional leading `-`, decimal, optional `.frac`, optional `e[+-]?digits` |
| `bool` | `true` / `false` |
| `String` | One-or-more non-whitespace chars (stops at whitespace) |

There is also a free function `vihaco_parser_core::ident()` returning a parser that accepts non-whitespace characters except the structural punctuation `, ; ( ) { } [ ]`. Use it via `#[parse_with]` (see below) when you want identifier-shaped input like `ch0:band1` or `gate:0`.

## Step 1: derive `Parse` on the instruction enum

```rust ignore
use vihaco::Instruction;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Instruction, vihaco_parser::Parse)]
#[instruction(width = 8)]
#[head = "signal::"]
pub enum SignalInst {
    Poly(Address),
    Play,
    Ramp(Address),
    Gate(Address),
}
```

The two derives are orthogonal and coexist on every instruction enum:

- `Instruction` owns opcode / bytecode / width / runtime semantics.
- `Parse` owns source-text parsing.

This enum's parser accepts `signal::Poly(ch0:band1)`, `signal::Play`, `signal::Ramp(ramp:0)`, `signal::Gate(gate:0)`. The exact form is determined by the attributes — covered next. (Here `Address` is a foreign address type; the `#[parse_with]` section below shows how a field whose syntax isn't a primitive `Parse` impl is parsed.)

## Step 2: attributes

### Enum-level

| Attribute | Effect |
|---|---|
| *(none)* | Each variant's default token is the lowercase variant name (e.g. `Foo` → `"foo"`). |
| `#[head]` | Prefix every variant's token with `"EnumName::"`. Variant casing is preserved (`Foo` → `"EnumName::Foo"`). |
| `#[head = "X::"]` | Custom prefix string. |

### Variant-level

| Attribute | Effect |
|---|---|
| `#[token = "name"]` | Override the per-variant token. With `#[head]`, the result is `"<head><name>"`. |
| `#[delimiters(open = "(", close = ")", separator = ",")]` | Override the delimiters surrounding the fields and the separator between them. All three keys are optional. Defaults shown. |
| `#[delegate]` | Skip the variant's own token and delimiters; delegate directly to the inner type's `Parse::parser()`. Only valid on single-field tuple variants. |

Setting `open = ""` (or `close = ""`) means *no delimiter at that position* — useful for bare forms like `ret`, `play 5`, `const.i64 -3`, or `add.i64`.

### Field-level

| Attribute | Effect |
|---|---|
| `#[parse_with = "path::to::fn"]` | Use the named function instead of `<T as Parse>::parser()` for this field. The function must have signature `fn() -> impl Parser<'src, &'src str, T, ...>`. |

`#[parse_with]` covers two real cases:
1. Foreign types where you can't write `impl Parse` (orphan rule).
2. Operands whose syntax is richer than the type's primitive `Parse` impl — for example, a CPU `add.i64` parses `.i64` into a `vihaco::Type`, which has no useful primitive impl.

## Step 3: use the generated parser

```rust ignore
use chumsky::Parser as _;
use vihaco_parser_core::Parse;

let got = SignalInst::parser()
    .parse("signal::Poly(ch0:band1)")
    .into_result()
    .unwrap();
assert!(matches!(got, SignalInst::Poly(_)));
```

That's it for the instruction-level surface.

## Worked example — CPU

CPU instructions are mostly bare-form mnemonics with optional dot-qualified types:

```rust ignore
use vihaco::program::{Type, Value};
use vihaco::Instruction;

#[derive(Debug, Clone, PartialEq, Instruction, vihaco_parser::Parse)]
pub enum Instruction {
    /// `breakpoint`. Must precede `Branch` (whose token `br` would be a
    /// prefix of `breakpoint`).
    Breakpoint,

    /// `br <target>` — symbolic; the orchestrator handles it via `never_u32`.
    #[token = "br"]
    #[delimiters(open = "", close = "", separator = "")]
    Branch(#[parse_with = "crate::parse_helpers::never_u32"] u32),

    Halt,
    Print,
    Dup,

    /// `const.<type> <literal>` — numeric/bool only here; strings are deferred.
    #[token = "const"]
    #[delimiters(open = "", close = "", separator = "")]
    Const(#[parse_with = "crate::parse_helpers::cpu_const_value"] Value),

    /// `add.<type>` etc. `cpu_type` consumes `.i64` / `.f64` / … into a `Type`.
    #[delimiters(open = "", close = "", separator = "")]
    Add(#[parse_with = "crate::parse_helpers::cpu_type"] Type),
}
```

The two `parse_helpers` functions are tiny chumsky combinators that live next to the enum:

```rust ignore
pub fn cpu_type<'src>() -> impl Parser<'src, &'src str, Type, E<'src>> {
    just('.').ignore_then(choice((
        just("i64").to(Type::I64),
        just("u64").to(Type::U64),
        just("f64").to(Type::F64),
        just("bool").to(Type::Bool),
    )))
}
```

This is the canonical pattern for foreign-type operands: keep the helper next to the enum, point at it with `#[parse_with]`.

## Variant ordering rules

The derive tries variants in declaration order. Two rules matter:

1. **Prefix rule** — if two token-bearing variants share a prefix, declare the longer one first. The derive emits a compile error if one variant's full token is a strict prefix of another that comes before it. Example: CPU declares `breakpoint` before `br`, and `call_indirect` before `call`.
2. **`#[delegate]` rule** — `#[delegate]` variants must come *after* all token-bearing variants in the same enum (the derive enforces this). Use `#[delegate]` for "outer" enums that compose smaller `Parse`-deriving enums.

## Deferred operands with `never_u32`

Some operands can't be parsed at instruction level because they reference symbols (`@label`) or interner-managed state (`"strings"`) that the resolver owns. The convention is a "never succeeds" helper like:

```rust ignore
pub fn never_u32<'src>() -> impl Parser<'src, &'src str, u32, E<'src>> {
    empty().try_map(|_, span| Err(Simple::new(None, span)))
}
```

Wired in via `#[parse_with]`, this makes `Instruction::parser()` fail on the variant's mnemonic — the orchestrator's fallback path (next guide) captures the source line as a `RawForm` instead.

## What comes next

- For module-level orchestration (`ParsedModule`, device headers, function bodies, sugar expansion, labels, string interning), see [Advanced Parser Customization](/guide/parser-advanced).
- To attach the instruction type to a component, see [Building Components With `vihaco`](/guide/components).
