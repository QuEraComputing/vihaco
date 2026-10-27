---
layout: ../../layouts/Guide.astro
title: Parsing source
slug: parser
description: "Parse typed instructions and lower source operands into runtime values."
---

# Parsing source

A parser recognizes how an instruction is written. Lowering turns the parsed
operands into values you can execute. You need both when the source contains
names, literals, or references that parsing alone can't resolve.

## Parse a leaf instruction

Here's a parser for a single instruction:

```rust
// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{Parse, chumsky::Parser as _, dialect};

#[dialect]
pub mod counter {
    #[instruction]
    pub struct Add(pub u64);
}

fn main() {
    let instruction = counter::Add::parser()
        .parse("add 5")
        .into_result()
        .expect("valid add instruction");
    assert_eq!(instruction.0, 5);
}
```

A leaf parser accepts the local mnemonic. A composite's syntax parser adds the
device route and dialect name, as in `calculator::arithmetic::add`.
Use `<Machine as HasInstructionSet>::Syntax` for that combined parser.

## Define a source pattern

`#[pattern = "'name $0"]` gives an instruction its mnemonic and captures a value
for a field. Without an annotation, the binding uses the field's parser. Add an
annotation when you want to keep a different type during parsing:

```rust
use vihaco::{Lower, Parse, Syntax, chumsky::Parser as _, dialect};

#[dialect]
mod control {
    #[instruction]
    #[pattern = "'jump $0:u64"]
    pub struct Jump(pub u32);
}

impl<C: ?Sized> Lower<C> for control::Jump<Syntax> {
    type Runtime = control::Jump;
    fn lower(self, _: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(control::Jump(self.0.try_into()?))
    }
}

let source = control::Jump::<Syntax>::parser()
    .parse("jump 4294967296").into_result().unwrap();
assert!(source.lower(&mut ()).is_err());
```

Here the syntax keeps a `u64` until lowering checks whether it fits the runtime
`u32`. Vihaco won't insert a Rust conversion implicitly. Write your lowering
impl outside the dialect module, in the same crate.

## Resolve symbolic input

For a named branch target, keep the name in the syntax operand and ask the
context for label lookup during lowering. For a quoted string, ask it for
string interning. A local literal wrapper can also parse a readable number and
encode it as a word while preserving its bits.

The generated aggregate requires the lowering capabilities of all its included
instructions. You'll need to supply those even if one program only uses numeric
operands.

Each parsed instruction lowers to one runtime instruction. Function-local source
labels become metadata, so they don't add instructions. If you want instruction
expansion, do that in a producer before this pipeline.

The [pattern parser reference](/guide/parser-patterns) covers dialect instruction
patterns and parsers for metadata, values, and types, with delimiter rules and
diagnostics.
[Loading SST programs](/guide/parser-advanced) uses these parsers with complete
programs.
