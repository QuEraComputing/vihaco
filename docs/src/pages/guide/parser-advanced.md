---
layout: ../../layouts/Guide.astro
title: Advanced Parser Customization
slug: parser-advanced
description: Module-level orchestration — device headers, the ParsedModule two-pass design, Resolve impls, sugar expansion, string interning, and label resolution.
---

# Advanced Parser Customization

This guide picks up where [Parser Integration for Component Instructions](/guide/parser) ends. The integration guide shows how to derive `Parse` on a single instruction enum. This guide covers what surrounds the instruction enum:

- Module-level orchestration: device headers, function bodies, blank lines, `//` comments.
- The two-pass design that separates *parsing* from *resolving* (sugar expansion, string interning, label resolution).
- Hand-writing `Parse` for composite enums where `#[derive(Parse)]` can't reach (e.g. the enum is itself produced by another macro).

Almost every real source module needs all three.

## The two-pass design

A source module contains:

- Device headers (`device tone 0, 32;`).
- Function blocks (`fn @main() -> i64 { ... }`).
- Inside each function: canonical instructions (`signal::Play`), sugar (`poly addr 0.0 0.2 0.0 0.0`), symbolic operands (`br @body`, `const.str "hi"`), labels (`@entry:`).

Parsing alone can't produce a runtime `Module<I, V, Ty, Info>` directly: sugar expands into multiple instructions, strings need interning into the module's string table, labels need a forward-reference table. The pipeline splits that work in two:

1. **Parse pass** — `<ParsedModule<I, H> as Parse>::parser()` consumes the source into a lossless intermediate shape. Canonical instructions become `BodyItem::Direct(I)`; anything else becomes `BodyItem::Raw(RawForm)`.
2. **Resolve pass** — your `Resolve` impl walks the `ParsedModule`, applies headers to a device-info value, expands sugar, interns strings, and produces a final `Module`.

Each consumer crate owns the resolver. Parsing has no consumer-specific state.

## The intermediate types

`vihaco::syntax` exposes everything you need.

```rust ignore
pub struct ParsedModule<I, H> {
    pub headers: Vec<H>,
    pub functions: Vec<ParsedFunction<I>>,
}

pub struct ParsedFunction<I> {
    pub name: String,
    pub params: Vec<Param>,
    pub return_ty: Option<RawType>,
    pub body: Vec<BodyItem<I>>,
}

pub enum BodyItem<I> {
    /// I::parser() succeeded — fully-typed instruction.
    Direct(I),
    /// I::parser() failed — captured as a lossless source form.
    Raw(RawForm),
}

pub struct RawForm {
    pub mnemonic: String,
    pub operands: Vec<RawOperand>,
}

pub enum RawOperand {
    Ident(String),   // ch0:band1, gate:0
    Int(i64),
    UInt(u64),
    Float(f64),
    Bool(bool),
    StringLit(String),
    Symbol(String),  // @name — leading @ consumed
}
```

The parser is whitespace- and comment-aware: blank lines, indentation, and `//`-to-end-of-line comments are skipped between items.

`ParsedModule` derives nothing — its `Parse` impl is hand-written in `vihaco::syntax::parse` and works for any `I: Parse` and `H: Parse`.

## Step 1: define the header enum

Headers are derived just like instructions, with a `#[head = "device "]` prefix:

```rust
#[derive(Debug, Clone, PartialEq, vihaco_parser::Parse)]
#[head = "device "]
pub enum DeviceHeader {
    #[token = "tone"]
    #[delimiters(open = "", close = "", separator = ",")]
    Tone(i64, i64),

    #[token = "clock.resolution_ns"]
    #[delimiters(open = "", close = "", separator = "")]
    ClockResolutionNs(i64),

    // Block-body header — `device grid { 1 1\n 5 1\n ... };`
    #[token = "grid"]
    #[delimiters(open = "{", close = "}", separator = "")]
    Grid(#[parse_with = "vihaco::syntax::block_i64_pairs"] Vec<(i64, i64)>),

    #[token = "mask"]
    #[delimiters(open = "{", close = "}", separator = "")]
    Mask(#[parse_with = "vihaco::syntax::block_i64_flat"] Vec<i64>),
}
```

The `;` after each header is owned by the module parser, not the header derive.

`vihaco::syntax` ships two block-body helpers for common shapes:

- `block_i64_flat()` parses whitespace-separated `i64`s inside `{ ... }`.
- `block_i64_pairs()` parses rows of `i64 i64` inside `{ ... }`.

When you need a richer block body, write your own `fn() -> impl Parser<...>` and point at it with `#[parse_with]`.

## Step 2: parse a module

```rust ignore
use chumsky::Parser as _;
use vihaco::syntax::ParsedModule;
use vihaco_parser_core::Parse;

let parsed = ParsedModule::<MyInstruction, MyHeader>::parser()
    .parse(&source)
    .into_result()
    .map_err(|errs| eyre::eyre!("parse failed: {errs:?}"))?;
```

`parsed.headers` is `Vec<MyHeader>`. `parsed.functions[i].body` is `Vec<BodyItem<MyInstruction>>`.

## Step 3: implement `Resolve`

```rust ignore
use vihaco::syntax::{BodyItem, ParsedModule, Resolve};
use vihaco::module::Module;

pub trait Resolve<I, H> {
    type Module;
    fn resolve_module(&mut self, parsed: ParsedModule<I, H>) -> eyre::Result<Self::Module>;
    // optional override:
    fn resolve_body(&mut self, items: Vec<BodyItem<I>>) -> eyre::Result<Vec<I>> { ... }
}
```

A minimal resolver:

```rust ignore
#[derive(Default)]
pub struct MyResolver {
    strings: Vec<String>,
}

impl Resolve<MyInstruction, MyHeader> for MyResolver {
    type Module = Module<MyInstruction, Value, Type, MyDeviceInfo>;

    fn resolve_module(
        &mut self,
        parsed: ParsedModule<MyInstruction, MyHeader>,
    ) -> eyre::Result<Self::Module> {
        let mut info = MyDeviceInfo::default();
        for header in parsed.headers {
            apply_header(&mut info, header)?;
        }

        let mut code = Vec::new();
        for function in parsed.functions {
            for item in function.body {
                match item {
                    BodyItem::Direct(inst) => code.push(inst),
                    BodyItem::Raw(raw) => code.extend(self.lower_raw(raw)?),
                }
            }
        }

        let mut m = Module::default();
        m.code = code;
        m.strings = std::mem::take(&mut self.strings);
        m.extra = info;
        Ok(m)
    }
}
```

Everything interesting happens in `lower_raw` and any pre/post passes you add around the body loop.

## Sugar expansion

Sugar is "one source line, many instructions". The resolver decides per-mnemonic what to emit.

```rust ignore
fn lower_raw(&mut self, raw: RawForm) -> eyre::Result<Vec<MyInstruction>> {
    match raw.mnemonic.as_str() {
        // const.str "..." — needs the interner, so it can't be canonical.
        "const.str" => {
            let s = expect_string_lit(&raw)?;
            let idx = self.intern(s);
            Ok(vec![cpu::Instruction::Const(Value::String(idx)).into()])
        }

        // play 5 → const.u64 5; signal::Play
        "play" | "signal::Play" => {
            let cycles = expect_u64(&raw.operands[0])?;
            Ok(vec![
                cpu::Instruction::Const(Value::U64(cycles)).into(),
                signal::Instruction::Play.into(),
            ])
        }

        // poly addr c0 c1 c2 c3 → 4× const.f64 then signal::Poly addr
        "poly" | "signal::Poly" => expand_poly(&raw),

        other => Err(eyre!("unhandled raw form `{other}`")),
    }
}
```

The canonical form (`signal::Play`) still parses cleanly into `BodyItem::Direct`. The sugar form (`play 5`) falls through to `BodyItem::Raw` because the derive parser, given just `play`, fails on the trailing `5`.

## String interning

Strings can't go in `BodyItem::Direct(cpu::Instruction::Const(Value::String(idx)))` directly because the index is allocated by the resolver, not known at parse time. The pattern:

```rust ignore
impl MyResolver {
    fn intern(&mut self, s: &str) -> u32 {
        if let Some(idx) = self.strings.iter().position(|x| x == s) {
            return idx as u32;
        }
        let idx = self.strings.len() as u32;
        self.strings.push(s.to_string());
        idx
    }
}
```

Then `const.str "hello"` parses as a `RawForm { mnemonic: "const.str", operands: [StringLit("hello")] }` and the resolver interns the string before producing `cpu::Instruction::Const(Value::String(idx))`.

## Symbolic operands: labels and branches

Labels (`@entry:`) and symbolic targets (`br @body`, `br @body, @exit`) require a two-pass within `resolve_module`:

1. Walk body items. A label declaration records `labels[name] = code.len() as u32` and emits nothing. A branch emits a placeholder and records a patch.
2. After the body is fully lowered, replay each patch — look up the resolved index and overwrite the placeholder.

```rust ignore
enum BranchPatch {
    Unconditional(String),
    Conditional(String, String),
}

fn raw_as_label(raw: &RawForm) -> Option<String> {
    if !raw.operands.is_empty() { return None; }
    let stripped = raw.mnemonic.strip_prefix('@')?.strip_suffix(':')?;
    (!stripped.is_empty()).then(|| stripped.to_string())
}

fn raw_as_branch(raw: &RawForm) -> Option<BranchPatch> {
    let syms: Vec<&str> = raw.operands.iter().map(|op| match op {
        RawOperand::Symbol(s) => Some(s.as_str()),
        _ => None,
    }).collect::<Option<_>>()?;
    match (raw.mnemonic.as_str(), syms.as_slice()) {
        ("br", [t]) => Some(BranchPatch::Unconditional((*t).into())),
        ("br", [t, f]) | ("cond_br", [t, f]) => {
            Some(BranchPatch::Conditional((*t).into(), (*f).into()))
        }
        _ => None,
    }
}
```

The body loop becomes:

```rust ignore
let mut labels: HashMap<String, u32> = HashMap::new();
let mut patches: Vec<(usize, BranchPatch)> = Vec::new();

for item in function.body {
    match item {
        BodyItem::Direct(inst) => code.push(inst),
        BodyItem::Raw(raw) => {
            if let Some(name) = raw_as_label(&raw) {
                if labels.insert(name.clone(), code.len() as u32).is_some() {
                    return Err(eyre!("duplicate label `@{name}`"));
                }
                continue;
            }
            if let Some(patch) = raw_as_branch(&raw) {
                patches.push((code.len(), patch));
                code.push(patch.placeholder());
                continue;
            }
            code.extend(self.lower_raw(raw)?);
        }
    }
}

for (idx, patch) in patches {
    patch.apply(&mut code, idx, &labels)?;
}
```

This is the pattern a root resolver uses for a machine that mixes a CPU with device instructions. Keep it next to your `Resolve` impl.

## Hand-writing `Parse` for composite enums

`#[derive(Parse)]` works on enums you control. The [`#[vihaco::composite]`](/guide/composites) attribute generates an outer enum (e.g. `MachineInstruction { Cpu(...), Signal(...) }`) that you can't put `#[derive(Parse)]` on at source.

Hand-roll the same dispatch the derive's `#[delegate]` would emit:

```rust ignore
use chumsky::prelude::*;
use vihaco_parser_core::Parse;

impl<'src> Parse<'src> for MachineInstruction {
    fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
        let cpu = <cpu::Instruction as Parse>::parser().map(MachineInstruction::Cpu);
        let signal = <SignalInst as Parse>::parser().map(MachineInstruction::Signal);
        choice((cpu, signal))
    }
}
```

The order matters: variants with overlapping prefixes (rare across devices, but still possible) must be tried longest-first.

## The full pipeline at a call site

```rust ignore
use chumsky::Parser as _;
use vihaco::syntax::{ParsedModule, Resolve};
use vihaco_parser_core::Parse;

let source = std::fs::read_to_string(path)?;

let parsed = ParsedModule::<MachineInstruction, DeviceHeader>::parser()
    .parse(&source)
    .into_result()
    .map_err(|errs| eyre::eyre!("parse failed: {errs:?}"))?;

let module = MyResolver::new().resolve_module(parsed)?;

let mut machine = Machine::default();
machine.load(&module)?;
machine.run()?;
```

This is the shape a CLI `run` command uses: read the source, parse it into a `ParsedModule`, resolve it into a runtime `Module`, then load and run it.

## When you'd need a fully custom parser

Two cases the derive can't model directly:

1. **Struct types** — `ParsedFunction` is a struct, not an enum, so its `Parse` impl is hand-written. If you find yourself wanting `#[derive(Parse)]` on a struct, write the chumsky combinators directly instead.
2. **Multi-segment overloads of the same mnemonic** — for example, `br @t` (1 operand) vs `br @t, @f` (2 operands). These are emitted as a single `Raw` form and disambiguated in the resolver; you don't need a custom parser, you need a smarter `lower_raw` arm.

If you reach a third case, prefer adding a `#[parse_with]` helper or a tiny hand-written combinator over a bespoke `Parse` impl. The two-pass design keeps the parser side narrow on purpose.
