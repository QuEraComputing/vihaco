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
#[syntax_class(instruction)]
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
- Supports both executable composites and structural composites. A composite may be top-level,
  nested, or both executable and top-level.
- Treats a local program/instruction stream as optional. When a composite declares `#[program]` and
  executable routes, generation includes its runtime instruction sum, fetch/step boundary, and
  program-counter completion plumbing. Without `#[program]`, generation does not invent a local
  instruction stream.
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
- Generates route-specific effect fanout directly in each `execute_generated` instruction arm; a
  separate runtime `drain` function is not required.
- Requires each effect route to declare its observers and exactly one handler explicitly. The
  declaration is shaped as `effects { observe foo, bar; to foobar; }`.
- Generates observer calls in declaration order with a shared borrow of each effect, then gives
  the owned effect to the single handler.
- Type-checks every listed observer against `Observe<Effect, Route>` and the handler against
  `Handle<Effect, Route>`.
- Converts observer and handler errors into the route error through `Into<R::Error>` (or an
  equivalent framework conversion bound), allowing observers to use either the route error or
  their own error type.
- Applies machine fault conversions.
- Attaches optional route metadata that a configured driver may consume.
- Preserves component and source-symbol metadata needed by loaders.

### Proposed `machine!` surface

`machine!` is the author-facing shorthand for a composite declaration. It lowers to the composite
attribute, the runtime-instruction declaration, and the generated execution relationship. The
same surface covers a top-level executable machine such as Cursa and a structural runtime root
such as `HeterogeneousMachine`.

An executable top-level machine can own a program and child devices:

```rust
machine! {
    composite Cursa {
        #[program]
        loader: ProgramImage<Instruction, NoContext, Value, Type, DeviceInfo>,

        #[device(0x01, alias = "cpu")]
        cpu: Cpu,

        #[device(0x02, alias = "fpga")]
        fpga: Fpga,
    }

    runtime {
        device Cpu => cpu::Instruction {
            message with resolve_cpu;
            effects with continue_cpu;
        }

        device Fpga => fpga::Instruction {
            message with resolve_fpga;
            effects with continue_fpga;
        }
    }
}
```

The generated portion is equivalent in shape to:

```rust
#[composite]
#[runtime(
    Cpu => cpu::Instruction {
        message with resolve_cpu;
        effects with continue_cpu;
    },
    Fpga => fpga::Instruction {
        message with resolve_fpga;
        effects with continue_fpga;
    },
)]
struct Cursa {
    #[program]
    loader: ProgramImage<Instruction, NoContext, Value, Type, DeviceInfo>,
    #[device(0x01, alias = "cpu")]
    cpu: Cpu,
    #[device(0x02, alias = "fpga")]
    fpga: Fpga,
}
```

The attribute form is the procedural-macro expansion target; authors normally write the
`machine!` form. The macro generates the outer instruction enum, route dispatch, message resolver
calls, effect continuation, and program-counter transitions. The named resolver and handler
methods remain ordinary author code because they contain machine-specific semantics.

A structural top-level runtime can use the same declaration without a local program:

```rust
machine! {
    composite HeterogeneousMachine {
        clock: GlobalClock<MachineEvent>,
        fabric: ChannelFabric<i64, CpuId>,

        #[device(0x01, alias = "cpu_a")]
        cpu_a: Cpu,

        #[device(0x02, alias = "cpu_b")]
        cpu_b: Cpu,
    }

    runtime {}
}
```

This still gets composite metadata and child-section wiring, but its event loop is supplied by
the runtime root rather than generated as a local instruction stepper. A top-level composite may
also emit effects. In that case `effects to parent` is not valid unless the root has an explicit
outer sink; use a host/runtime boundary or a root handler instead.

## Message Wiring

Message wiring supports:

- A `NoMessage` route with no generated resolver.
- A single-component route whose message is supplied through a component `Supply` capability.
- A route-local resolver method for a message assembled from several components.

Wiring remains type-checked, and a resolved message is an owned value so that a parked route retains
no borrow into the composite. Message resolution is generated as a route method rather than a
marker-parameterized trait, because a message is a single value read across composite fields and
cannot be relocated onto a component.

## Effect Wiring

Effect wiring supports:

- One effect observed by zero or more explicitly listed observers.
- One effect consumed by exactly one explicitly listed handler.
- A route-local handler method.
- A default handler when the effect is `NoEffect`.

The preferred route syntax is:

```rust
effects {
    observe debug, stdout;
    to stack;
}
```

The generated body is equivalent in shape to:

```rust
for effect in effects {
    observer_a.observe(&effect).map_err(Into::<RouteError>::into)?;
    observer_b.observe(&effect).map_err(Into::<RouteError>::into)?;
    handler.handle(effect).map_err(Into::<RouteError>::into)?;
}
```

This is a many-readers/one-consumer boundary: observers never consume or clone the effect, and the
handler receives ownership exactly once. `Observe<Effect, Route>` is route-parameterized so the
same effect type may be observed differently on different routes. A generic observer such as
`DebugTrace` may implement the trait for every `E`/`R` pair satisfying the route bounds.

The macro cannot discover observer implementations by inspecting the crate. Observer names must
therefore be explicit in each runtime route; the compiler validates that each named field
implements the required observer trait. The macro also preserves observer declaration order.

Wiring remains type-checked. Macro input may contain strings for field names or source aliases, but
generated execution never performs string-based runtime routing.

Wiring also never inserts a value conversion. The producer and handler types must match, or the
route must name an author-defined converter or handler with explicit semantics.

### Optional debug instrumentation

Future composite debug instrumentation may be opt-in, for example with `#[vihaco::debug]`. It may
inject a private `DebugTrace` field, add that field as an observer to every effect route, and expose
an accessor for the collected records. The generated trace requires observed effects to implement
`Debug` and records the route identity with `std::any::type_name::<Route>()`. Nested-composite
scope and per-route opt-out behavior remain design decisions.

## Bytecode Generation

Future bytecode derives and composite generation:

- Implement portable primitive codecs in vihaco core.
- Compose codecs implemented by the authors of runtime instruction, constant, and type products.
- Encode each admitted section with its owner's header, payload, and data model; a structural root
  may have an empty local instruction payload.
- Assign explicit stable section-local route opcodes to each generated machine runtime sum.
- Recursively forward child section encoding and loading through named loadable fields.
- Preserve the file-wide global context and parent-relative child section table.
- Never derive persistent identifiers from Rust variant order or layout.
- Keep bytecode traits off surface instruction, value, and type products unless an author
  independently chooses to persist one.

The complete codec ownership model is defined in
[`types-and-values.md`](./types-and-values.md).
