# SST Parsing and Resolution

The pattern parser is the single SST syntax frontend. It constructs surface instructions and never
runtime instructions:

```rust
#[derive(vihaco_parser::Parse)]
#[syntax_class(instruction)]
#[pattern = "'conditional_branch `@` $when_true `,` `@` $when_false"]
pub struct SurfaceConditionalBranch {
    pub when_true: String,
    pub when_false: String,
}
```

The generated parser accepts:

```text
control::conditional_branch @then, @otherwise
```

The surface instruction owns its `head`, written without a trailing `::`, and its first pattern
atom is the instruction token.

Pattern compilation validates the complete mapping from syntax to the Rust product:

- Tuple instructions use numeric bindings such as `$0`.
- Named instruction structs use field bindings such as `$value`.
- Every field is bound exactly once.
- Bindings may be reordered without changing constructor field order.
- Unit instructions contain no bindings.
- Instruction patterns begin with one mnemonic token.
- Leading, trailing, repeated, and tab-separated pattern whitespace is rejected.
- Literal keywords are written as backtick atoms.
- Comma and `@` have punctuation-aware literal forms.

Every bound field delegates to that field type's
`vihaco_parser_core::Parse::parser()`. Specialized syntax belongs in a surface type with its own
pattern-derived parser:

1. Use a local type with an appropriate `Parse` implementation.
2. Use a local newtype around a foreign type.
3. Parse through a richer surface instruction and lower it during `Resolve`.

A missing language construct is addressed by extending the pattern generator, not by introducing a
second parser or compatibility attribute system.

`#[syntax_class(instruction, ...)]` identifies a surface instruction and may generate the
framework's surface-instruction marker. That marker is independent of the runtime bytecode
`Instruction` contract: parsing a source product must not require an opcode, byte width, or decoder.

## Value and Type Operands

The `value` and `type` syntax classes let instruction fields delegate grammar to domain types:

```rust
#[derive(vihaco_parser::Parse)]
#[syntax_class(type)]
#[pattern = "`i64`"]
pub struct I64Type;

#[derive(vihaco_parser::Parse)]
#[syntax_class(value)]
#[pattern = "$value"]
pub struct ImmediateI64 {
    pub value: i64,
}
```

Value and type patterns cannot contain instruction tokens. Types always declare an explicit
pattern. Values receive defaults only for unambiguous unit or single-field forms; multi-field
values declare their pattern explicitly.

The instruction pattern consequently remains structural: it binds fields, while each field type
owns its grammar.

These products are author-defined. Vihaco does not supply a semantic `SurfaceValue`,
`SurfaceType`, runtime `Value`, or runtime `Type` that every machine must use. Parser core instead
provides fallible scalar parsers and distinct lexical helpers that authors compose into their own
products. Identifiers, symbols, quoted strings, and unresolved literal text remain different
shapes rather than aliases for one catch-all `String`.

A parsed function's parameter and return types likewise use an author-selected surface type:

```text
ParsedModule<MachineModuleSyntax>
```

The complete ownership and runtime relationship is defined in
[`types-and-values.md`](./types-and-values.md).

## Canonical Syntax Ownership

A reusable surface instruction owns its canonical dialect head and pattern. The composite decides
whether to admit that parser, but does not normally rewrite its mnemonic or head.

The composite may still provide:

- Source-symbol aliases for a child section or device.
- Pattern-derived sugar forms that lower to one or more runtime instructions.
- Compatibility aliases represented by explicit surface instruction types.
- Machine-local wrappers when a genuinely different public syntax is required.

These are source-orchestration concerns rather than mutations of a canonical pattern. Two unrelated
spellings use two surface instruction types, or an explicit source-level enum, even when both lower
to the same runtime operation.

## Composite Parser Generation

An executable composite's surface parser is the sum of exactly the selected surface products:

```text
Parse<stack::surface::Push>
    + Parse<arithmetic::surface::Add>
    + Parse<control::surface::ConditionalBranch>
    = Parse<MyMachineSurfaceInstruction>
```

Omitted surface instructions are absent from the parser choice by construction. The composite does
not parse every component catalog and reject unsupported forms afterward; unsupported SST is
unrecognizable at the parser boundary.

The pattern parser composes the selected alternatives, including overlapping mnemonic prefixes and
large instruction sets. The composite supplies types and does not implement a separate parsing
algorithm.

The non-executing `HeterogeneousMachine` runtime root selects no local surface or runtime
instructions and therefore has no local instruction parser. Its two CPU children parse and resolve
their own programs. A future structural root section may describe child sections and wiring without
creating an empty executable instruction sum.

## Parsing Versus Resolution

Pattern parsing and module resolution are consecutive but distinct boundaries. Parsing always
constructs a surface instruction and author-defined module type products.
`Resolve<ModuleSyntax>` then uses module-wide context to construct
runtime instructions, constants, and runtime type metadata:

- Labels and symbolic branch targets require symbol resolution.
- Interned strings require a module interner.
- Sugar may expand one surface instruction into several runtime instructions.
- Overloaded forms can be separate surface instruction types.
- Machine-specific validation may depend on headers or other section metadata.
- Surface literals require author-defined range and invariant checks.
- Source-language coercions must lower to explicit conversions.

The full distinction is:

```text
pattern parsing:
    source text -> surface instruction

module resolution:
    ParsedModule<ModuleSyntax>
        -> Resolve<ModuleSyntax>
        -> Module<RuntimeInstruction, Constant, RuntimeType, Info>

runtime message resolution:
    runtime instruction + machine state -> Execute<I>::Message
```

For `ConditionalBranch`, parsing preserves `@then` and `@otherwise` as source names. Module
resolution replaces them with fixed-width `InstructionIndex` values. Runtime message resolution
may later obtain the condition from the operand stack, but never resolves the labels again.

## Naming the Three Instruction Concepts

The API needs distinct names for three different concepts:

- A surface instruction.
- A runtime operation.
- A generated machine runtime-instruction sum.

A consistent naming direction is:

- `SurfaceInstruction` for the types constructed by the pattern parser.
- `Instruction` for an individual runtime operation.
- `MachineInstruction` or `InstructionSet` for the generated runtime sum.
- `Resolve<ModuleSyntax>` for module lowering.

The exact identifiers remain an API decision; the three roles must remain visible.
