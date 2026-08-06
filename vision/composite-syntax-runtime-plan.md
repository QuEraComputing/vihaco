# Composite Syntax and Runtime Instruction Plan

## Status

Design plan for the SST-only instruction pipeline. This document defines how a
composite declares source syntax, lowers parsed instructions into runtime
instructions, and executes those instructions through typed component routes.

Components provide reusable runtime products and `Execute<I>` implementations.
Composites provide the machine-specific SST vocabulary, lowering policy, route
selection, message resolution, and effect handling.

## Pipeline

```text
SST section
    -> generated composite surface parser
    -> ParsedModule<SurfaceInstruction, SurfaceType, Header>
    -> composite syntax-resolver trait
    -> Vec<RuntimeInstruction>
    -> program-container module installation
    -> program-counter execution
    -> runtime route selection
    -> message resolution
    -> Execute<I>
    -> effect observation and handling
```

The surface and runtime instruction types are distinct:

```text
surface instruction
    -> source/module resolution
    -> runtime instruction
```

Parsing never executes instructions. Runtime execution never performs source
resolution.

## Composite declaration

An executable composite has three relevant parts:

1. A `#[program]` field that owns the loaded program and program counter.
2. A `syntax` block that defines the composite's public SST vocabulary.
3. A `runtime` block that defines executable routes.

Illustrative shape:

```rust
vihaco::composite! {
    pub composite ControlMachine {
        error = ControlMachineFault;

        #[program]
        pub program: ControlProgram;

        #[device(0x01, alias = "processor")]
        pub processor: Processor;

        #[device(0x02, alias = "waveform")]
        pub waveform: WaveformDevice;

        #[device(0x03, alias = "logic")]
        pub logic: LogicDevice;

        #[device(0x04, alias = "sensor")]
        pub sensor: SensorDevice;

        #[device(0x05, alias = "optical")]
        pub optical: OpticalDevice;

        pub clock: Clock;
        pub stdout: StdoutObserver;
    }

    syntax {
        #[pattern = "'processor::step $0"]
        Step(StepSyntax) => lower_step;

        #[pattern = "'waveform::play $0"]
        Play(PlaySyntax) => lower_play;

        #[pattern = "'optical::clear"]
        Clear => runtime Clear;
    }

    runtime {
        Step(processor::instruction::Step) => processor {
            message with resolve_step;
            effects {
                handle with handle_step;
            }
        }

        Play(waveform::instruction::Play) => waveform {
            message with resolve_play;
            effects {
                observe stdout;
                handle with handle_waveform;
            }
        }

        Clear(optical::instruction::Clear) => optical {
            message none;
            effects {
                handle with handle_optical;
            }
        }
    }
}
```

The exact field and route names are user-defined. The important distinction is
that syntax names and runtime route names are allowed to differ.

## Generated modules

The composite macro generates namespaced modules rather than placing all
products in the composite's parent namespace:

```rust
pub mod control_machine {
    pub mod syntax {
        pub enum Instruction {
            Step(StepSyntax),
            Play(PlaySyntax),
            Clear,
        }

        pub trait Resolver {
            fn lower_step(
                &mut self,
                instruction: StepSyntax,
            ) -> Result<Vec<super::runtime::Instruction>, ControlMachineFault>;

            fn lower_play(
                &mut self,
                instruction: PlaySyntax,
            ) -> Result<Vec<super::runtime::Instruction>, ControlMachineFault>;
        }
    }

    pub mod runtime {
        pub enum Instruction {
            Step(processor::instruction::Step),
            Play(waveform::instruction::Play),
            Clear(optical::instruction::Clear),
        }

        pub trait MessageResolver {
            // Methods are generated for `message with ...` routes.
        }
    }

    pub mod routes {
        // Generated route markers and route-specific implementations.
    }
}

pub use control_machine::syntax::Instruction as SurfaceInstruction;
pub use control_machine::runtime::Instruction as RuntimeInstruction;
pub use control_machine::syntax::Resolver as ControlMachineSyntaxResolver;
pub use control_machine::runtime::MessageResolver as ControlMachineMessageResolver;
```

The generated syntax enum implements the parser's surface-instruction marker
and parser interface. The runtime enum is the execution boundary and does not
implement source parsing by default.

## Syntax declarations

Composite syntax patterns use complete public spellings. The new pattern
grammar does not require an instruction `head`:

```rust
syntax {
    #[pattern = "'waveform::play $0"]
    Play(PlaySyntax) => lower_play;
}
```

Instruction tokens accept namespaced identifiers:

```text
instruction-token = identifier, { "::", identifier } ;
```

The composite syntax block establishes the instruction syntax class, so
composite-generated instruction enums do not need an explicit
`#[syntax_class(...)]` attribute. User-defined payload types continue to use
the parser derive and syntax classes appropriate to their role.

### User-defined payload syntax

The composite owns the instruction prefix. A payload type owns the grammar of
its operands:

```rust
#[derive(vihaco_parser_derive::Parse)]
#[syntax_class(value)]
#[pattern = "$duration `,` $mode"]
pub struct PlaySyntax {
    pub duration: u64,
    pub mode: PlayMode,
}

vihaco::composite! {
    // ...
    syntax {
        #[pattern = "'waveform::play $0"]
        Play(PlaySyntax) => lower_play;
    }
}
```

`$0` invokes `PlaySyntax::parser()`. This keeps nested operand syntax
composable and prevents the composite macro from becoming a second struct
pattern parser.

### Direct mappings

Direct mappings are limited initially to unit instructions:

```rust
syntax {
    #[pattern = "'optical::clear"]
    Clear => runtime Clear;
}
```

The macro constructs the runtime route directly. Argument-bearing instructions
use named lowerers because procedural macros cannot inspect arbitrary external
runtime product definitions and infer safe conversions.

### Delegated syntax

Components do not provide parsers in the initial design. A composite may,
however, explicitly delegate an existing syntax vocabulary in the future or
where a reusable parser type already exists:

```rust
syntax {
    #[delegate(host_vm::Instruction, prefix = "processor")]
    Processor(host_vm::Instruction) => runtime Processor;
}
```

Delegation imports syntax; it does not make the component's instruction enum
the composite execution boundary.

## Syntax resolution

The macro generates a public syntax-resolver trait for named lowerers. The
trait is implemented directly by the composite:

```rust
impl ControlMachineSyntaxResolver for ControlMachine {
    fn lower_play(
        &mut self,
        instruction: PlaySyntax,
    ) -> Result<Vec<RuntimeInstruction>, ControlMachineFault> {
        let duration_ns = instruction.duration.try_into()?;

        Ok(vec![RuntimeInstruction::Play(
            waveform::instruction::Play { duration_ns },
        )])
    }
}
```

Lowerers receive only the parsed syntax value. They access module-resolution
state through `self.program` and may use other composite fields when the
machine explicitly permits it. The program object owns the resolution context;
the composite owns the machine-specific lowering policy.

Every named lowerer returns an owned sequence:

```rust
Result<Vec<RuntimeInstruction>, CompositeFault>
```

This supports one-to-one lowering, source sugar, and one-to-many expansion.
Module-level resolution assigns final instruction addresses after expansion so
labels and source symbols refer to the runtime program rather than the surface
instruction sequence.

The generated resolver trait contains only named lowerers. Direct mappings do
not create user methods.

## Multiple runtime routes

One surface instruction may select different runtime routes based on source
arguments or resolved module information:

```rust
syntax {
    #[pattern = "'arithmetic::add $0"]
    Add(AddSyntax) => lower_add;
}

runtime {
    IntegerAdd(arithmetic::instruction::Add) => integer_stack {
        message from integer_stack;
        effects {
            absorb with integer_stack;
        }
    }

    AddressAdd(arithmetic::instruction::Add) => address_stack {
        message from address_stack;
        effects {
            absorb with address_stack;
        }
    }
}
```

```rust
impl ControlMachineSyntaxResolver for ControlMachine {
    fn lower_add(
        &mut self,
        instruction: AddSyntax,
    ) -> Result<Vec<RuntimeInstruction>, ControlMachineFault> {
        let route = match instruction.ty {
            AddType::Integer => RuntimeInstruction::IntegerAdd(
                arithmetic::instruction::Add,
            ),
            AddType::Address => RuntimeInstruction::AddressAdd(
                arithmetic::instruction::Add,
            ),
        };

        Ok(vec![route])
    }
}
```

The outer runtime variant carries route identity. It selects the target field,
message resolver, effect policy, fault conversion, and any route-specific
timing or scheduling behavior. The inner runtime product describes the
operation executed by the selected component.

Runtime route selection based on source/module information happens during
syntax resolution. Decisions based on live machine state remain in runtime
message resolution or component execution.

## Runtime message resolution

Message resolution is a separate generated public trait. It runs after a
runtime route has been selected:

```rust
pub trait ControlMachineMessageResolver {
    fn resolve_play(
        &mut self,
        instruction: &waveform::instruction::Play,
    ) -> Result<PlayMessage, ControlMachineFault>;
}
```

The implementation may read both the loaded program and live composite state:

```rust
impl ControlMachineMessageResolver for ControlMachine {
    fn resolve_play(
        &mut self,
        instruction: &waveform::instruction::Play,
    ) -> Result<PlayMessage, ControlMachineFault> {
        let template = self.program.lookup_template(instruction.template)?;
        let snapshot = self.sensor.snapshot()?;

        Ok(PlayMessage {
            template,
            snapshot,
        })
    }
}
```

The resolver receives the inner runtime product, not the outer route variant.
The generated dispatch already knows which route selected it.

`message none` and `message from field` do not create user trait methods.
Only `message with resolver_method` creates a required message-resolver method.

Messages should be owned values. A resolver must not return a borrow into the
program image when the operation may park or otherwise outlive the immediate
dispatch call.

## Program containers

`#[program]` is a capability marker, not a concrete type requirement. An author
may provide any program container that owns the program image, PC, module
context, and whatever lookup or resolution state the machine needs:

```rust
pub struct ControlProgram {
    pub module: RuntimeModule,
    pub pc: u32,
    pub context: ProgramContext,
    pub strings: StringTable,
}
```

The framework keeps program behavior split across focused traits:

```rust
ProgramCounter
GetProgramInfo
LoadOwnSstSection
LoadSstSection
InstallProgramModule
```

The exact combination is validated by generated call sites. A program type is
not required to expose strings, constants, bytecode, or any other capability it
does not use.

The installation capability is intentionally small:

```rust
pub trait InstallProgramModule {
    type Instruction;
    type Module;
    type Context;

    fn install_module(
        &mut self,
        module: Self::Module,
        context: ContextHandle<Self::Context>,
    ) -> eyre::Result<()>;
}
```

Installation replaces the runtime module, context, and PC as one operation.
Program-specific lookup APIs remain author-defined. The framework does not
require a universal `resolve_string` or `resolve_constant` method.

## SST loading

The generated root loading path uses the existing multi-section loading model:

```text
LoadSstSection(root)
    -> generated composite LoadOwnSstSection
        -> parse root syntax
        -> lower through ControlMachineSyntaxResolver
        -> build temporary runtime module
        -> InstallProgramModule on #[program]
    -> forward direct child sections to #[loadable] fields
```

The root program is resolved independently of arbitrary live child-device
state. Child sections may provide explicit load metadata through the program's
resolution context, but syntax lowering does not inspect arbitrary device
fields.

The load is transactional. Parsing, lowering, expansion, label assignment, and
module construction complete before the program container replaces its current
module. A failure leaves the previously loaded program intact.

Generated composite methods should include:

```rust
fn load_source(&mut self, source: &str) -> Result<(), ControlMachineFault>;

fn load_parsed(
    &mut self,
    parsed: ParsedModule<SurfaceInstruction, SurfaceType, Header>,
) -> Result<(), ControlMachineFault>;
```

`load_parsed` is the primary unit-testing boundary for lowering. It avoids
coupling resolver tests to text parsing or section-container construction.

SST is the only loading format covered by this design. Bytecode loading is
outside the scope of the new pipeline.

## Errors and diagnostics

Named syntax lowerers and message resolvers use the composite's declared error
type:

```rust
Result<T, ControlMachineFault>
```

Section-loading traits continue to use `eyre::Result` so they can attach
section, function, and source-location context. Generated loading converts and
enriches composite faults at that boundary.

Generated resolution should identify the current function and instruction when
propagating a lowerer failure. A new framework-wide structured error type is
not required initially.

## Effect handling

Runtime routes continue to define effect observation and handling:

```rust
runtime {
    Play(waveform::instruction::Play) => waveform {
        message with resolve_play;
        effects {
            observe stdout;
            handle with handle_waveform;
        }
    }
}
```

The composite macro generates route-specific dispatch. Custom effect handlers
remain ordinary user methods; no generated effect-handler trait is required in
the initial design.

## Parser changes

The parser derive and shared parser machinery need the following changes for
the new composite model:

- Remove the requirement for instruction `head`.
- Support complete namespaced instruction tokens in patterns.
- Keep `syntax_class` for standalone user-defined payload types.
- Allow composite-generated instruction enums to receive parser metadata
  without requiring user-written `#[derive(Parse)]` declarations.
- Keep payload parsing compositional through each payload type's `Parse`
  implementation.
- Do not require reusable components to provide parsers.

The parser derive's existing pattern validation remains valuable: field
bindings must be complete, unambiguous, and type-directed.

## Implementation phases

1. Audit `ProgramCounter`, `GetProgramInfo`, `LoadOwnSstSection`,
   `LoadSstSection`, and the generated multi-section loading paths.
2. Define and implement `InstallProgramModule` with transactional module,
   context, and PC installation.
3. Add composite-generated `syntax` and `runtime` modules.
4. Generate surface instruction enums and parser implementations from
   composite syntax entries.
5. Remove instruction `head` requirements and add namespaced pattern tokens.
6. Generate public syntax-resolver and message-resolver traits.
7. Generate SST root loading and `load_parsed` paths for composites with
   `#[program]` and `syntax`.
8. Generate runtime route dispatch from the `runtime` block.
9. Migrate a small composite with unit direct mappings and argument-bearing
   named lowerers.
10. Migrate a composite with one surface instruction selecting multiple
    runtime routes.
11. Add transactional-load, interning, one-to-many expansion, source-location,
    and message-resolution tests.

## Non-goals

This design does not initially provide:

- component-owned parsers or default component syntax;
- bytecode loading;
- declarative field-by-field runtime constructors;
- a universal program lookup API for strings or constants;
- generated effect-handler traits;
- runtime route selection based on arbitrary live device state during module
  resolution;
- automatic compatibility with the old `head`-based parser declarations.
