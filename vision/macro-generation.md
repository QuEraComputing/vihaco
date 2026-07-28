# Macro Responsibilities

Macros materialize the relationships declared by instruction, component, and composite authors.
They validate and generate repetitive dispatch, but do not decide which operations a machine
contains or where data flows.

## Instruction Declaration

Surface and runtime declarations carry different responsibilities:

| Declaration | Responsibilities |
|---|---|
| Surface instruction | Pattern, dialect head, parsed source fields |
| Runtime instruction | Fully resolved fields and execution identity |
| Author surface value/type | Pattern and parsed source fields |
| Author runtime value/type | Resolved semantics and, when persisted, its byte codec |

For example:

```rust
#[derive(vihaco_parser::Parse)]
#[syntax_class(instruction, head = "control")]
#[pattern = "'branch `@` $target"]
pub struct SurfaceBranch {
    pub target: String,
}

#[derive(Instruction)]
pub struct Branch {
    pub target: InstructionIndex,
}
```

The runtime instruction derive does not introduce source syntax. Parsing remains exclusively a
surface-instruction concern.

Deriving `Parse` for a value or type product does not make it part of a vihaco-wide data model. The
author selects that product in instruction fields or as the parsed module's surface type. Scalar
field parsers come from parser core. Surface instruction derives may generate the
surface-instruction marker, but do not generate runtime opcode or bytecode implementations.

## Component Declaration

The component macro:

- Associates a component with the runtime instructions it can execute.
- Validates per-runtime-instruction execution signatures.
- Does not require one component-wide instruction enum.
- Does not generate a component-wide dispatch match.

Ordinary `impl Execute<I> for C` remains the underlying API. The initial implementation establishes
that relationship directly before adding shorthand for repeated declarations.

## Composite Declaration

The composite/machine macro:

- Collects explicitly selected surface instructions and runtime instruction routes as separate
  sets.
- Rejects duplicate public variant names.
- Verifies that each target field implements `Execute<I>`.
- Generates the surface instruction sum and its pattern parser.
- Requires every selected surface instruction to implement `vihaco_parser_core::Parse<'src>`.
- Uses the author-selected module surface type for function signatures and declarations.
- Generates the runtime instruction sum.
- Requires a `Resolve<MachineSurfaceInstruction, MachineSurfaceType, Header>` implementation whose
  output module uses the machine runtime instruction sum and author-defined constant/type products.
- Generates the outer execution match.
- Generates or calls route-specific message resolvers.
- Generates or calls route-specific effect handlers.
- Applies machine fault conversions.
- Attaches optional route metadata that a configured driver may consume.
- Preserves component and source-symbol metadata needed by loaders.

## Effect Wiring

Effect wiring supports:

- One effect sent to one handler.
- One effect sent through a deterministic chain.
- One effect broadcast to multiple handlers.
- A route-local handler method.
- A default handler when the effect is `NoEffect`.

Wiring remains type-checked. Macro input may contain strings for field names or source aliases, but
generated execution never performs string-based runtime routing.

Wiring also never inserts a value conversion. The producer and handler types must match, or the
route must name an author-defined converter or handler with explicit semantics.

## Bytecode Generation

Future bytecode derives and composite generation:

- Implement portable primitive codecs in vihaco core.
- Compose codecs implemented by the authors of runtime instruction, constant, and type products.
- Encode each component or composite's local section with its own header, payload, and data model.
- Assign explicit stable section-local route opcodes to each generated machine runtime sum.
- Recursively forward child section encoding and loading through named loadable fields.
- Preserve the file-wide global context and parent-relative child section table.
- Never derive persistent identifiers from Rust variant order or layout.
- Keep bytecode traits off surface instruction, value, and type products unless an author
  independently chooses to persist one.

The complete codec ownership model is defined in
[`types-and-values.md`](./types-and-values.md).
