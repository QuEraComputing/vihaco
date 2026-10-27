---
layout: ../../layouts/Guide.astro
title: Dialects and instructions
slug: instructions
description: "Define and consume a typed instruction vocabulary independently of its implementation."
---

# Dialects and instructions

A dialect is a named vocabulary of instructions. It says what each operation
means, and components implement those operations. That gives a simulator, a
recorder, and a device connected to hardware a vocabulary they can share.

## Declare the vocabulary

```rust
use vihaco::dialect;

#[dialect]
pub mod counter {
    #[instruction]
    pub struct Add(pub u64);

    #[instruction]
    pub struct Clear;
}
```

Mark each operation type with `#[instruction]`. You can keep helper items in the
same module; they won't become instructions unless you mark them too.

When you document an operation, explain its payload fields, runtime inputs, and
results, along with any behavior callers need to agree on. For arithmetic, for
example, say whether overflow wraps or returns an error.

The macro gives components and composites the instruction identities and parsing
support they need. You work with the public instruction types; vihaco handles
the generated inventory and descriptions.

## Decide what an operand represents

Some inputs belong in the program itself. `counter::Add(5)`, for instance,
encodes the increment as its payload. Other inputs arrive when the instruction
runs. An addition with no payload can receive two numbers through
`PopFromStack<(u64, u64)>`. Which choice fits depends on what the operation means.

A composite chooses the destination device. In source,
`controller::counter::counter::add 5` names the device fields first, then the
dialect and mnemonic. A parser for just the instruction accepts the local
spelling, `add 5`.

## Syntax and runtime operands

Sometimes you want to keep a source name or readable literal around until the
loader can resolve it. The dialect supplies the parsed and executable forms
for you. Annotate a pattern binding when its source type differs from the
value execution needs.

You write a `Lower<C>` implementation for fields that need conversion, using
the lookup or other capabilities provided by the loading context. The impl
names the parsed input and executable output; generated lowering carries
unchanged values through for you.
[Parsing source](/guide/parser) has a checked example.

## Consume or evolve a dialect

A component declares support for whole dialects and implements every instruction
in each one. If you add an instruction, every component declaring that dialect
needs an implementation for it. An extra `Execute<I>` impl on its own doesn't
add the dialect to the component's declared capabilities.

It's useful to keep the different names and identities straight. A mnemonic
change affects source programs; a serialized opcode or payload change affects
binary compatibility. Rust types and device routes have their own identities,
and importing a dialect under an alias doesn't rename its canonical identity.

With the vocabulary defined, you can [write a component](/guide/components) to
execute it.
