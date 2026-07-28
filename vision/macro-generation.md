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
    pub target: usize,
}
```

The runtime instruction derive does not introduce source syntax. Parsing remains exclusively a
surface-instruction concern.

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
- Generates the runtime instruction sum.
- Requires a `Resolve<MachineSurfaceInstruction, Header>` implementation whose output module uses
  the machine runtime instruction sum.
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

