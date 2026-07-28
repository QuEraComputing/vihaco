# Hybrid Component-Bound Instruction Architecture

## Status and Direction

Vihaco needs instructions to remain reusable without reducing components to passive storage. A
pure instruction model (i.e. everything is an effect) makes dataflow explicit, but forces even
owner-local state changes through the composite. A component-owned instruction-set model
preserves local invariants, but exposes every instruction carried by every selected component
(i.e. composites must support *every* instruction from each of its composites).

This architecture takes the useful boundary from each model. Instructions remain individually
selectable types, while components retain responsibility for executing the operations that mutate
their state.

The heterogeneous two-CPU machine in [`demo.md`](./demo.md) is the integration reference for these
boundaries. The instruction rewrite and demo should develop together: the general
architecture must support the demo without introducing CPU-, clock-, or communication-specific
behavior into vihaco core.

The model has the following properties:

- Instructions remain individual Rust structs so that a machine can select them independently.
- Surface instructions describe SST syntax and are parsed exclusively by the pattern parser.
- Runtime instructions contain fully resolved operands and are the only instructions executed by
  components.
- The machine's `Resolve<SurfaceInstruction, Header>` implementation lowers surface instructions
  into runtime instructions before execution.
- Components remain the owners of state and the invariant-preserving operations over that state.
- A component implements execution for each instruction it supports.
- A composite explicitly selects the instructions that are part of its public instruction set.
- The composite owns machine-level instruction dispatch, message resolution, and effect routing.
- An external driver owns program iteration and any scheduling or modeled time policy needed by
  that execution mode. Program-counter transitions have one configured owner: either the driver or
  a modeled machine component.
- An instruction may directly mutate the one component selected as its execution target.
- Cross-component inputs and outputs are represented through message resolution and effects.

The resulting ownership model is:

| Decision | Owner |
|---|---|
| What syntax is accepted from SST? | Surface instruction types and their patterns |
| How are labels, symbols, and sugar lowered? | The implementer of `Resolve<SurfaceInstruction, Header>` |
| What fully resolved data is stored for execution? | Runtime instruction types |
| Which component knows how to execute it? | The selected component's `Execute<I>` implementation |
| Is the instruction available in this machine? | The composite |
| Which component instance receives it? | A composite route |
| Where does non-inline input come from? | Composite message resolution |
| Where do results and effects go? | Composite effect handling |
| Who advances the program counter? | Either the driver or one modeled component, never both |
| How much modeled time passes? | The selected driver, using route, component, or effect data |
| Can execution park or resume? | The driver together with the resource that owns the continuation |

Components may publish a catalog of operations they can execute, but that catalog is not the
machine's instruction set. The composite selects individual instructions and gives each selection a
machine-local route.

## Goals

The architecture is intended to preserve the following properties:

1. A composite exposes only surface and runtime instructions it explicitly selects.
2. Unsupported surface instructions cannot be parsed, and unsupported runtime instructions cannot
   be dispatched by that composite.
3. Each instruction has statically paired message, effect, and fault types for a particular
   component implementation.
4. A component can preserve its own invariants without converting every local mutation into an
   effect.
5. Cross-component data movement remains explicit in the composite.
6. A reusable semantic instruction can be executed in more than one machine architecture.
7. The same instruction can be routed to multiple instances of the same component type.
8. Synchronous execution remains easy to inline and statically dispatch.
9. Suspension remains limited to instruction boundaries.
10. Nested composites can expose a selected instruction set without leaking all instructions from
    their children.
11. The generated surface remains ordinary Rust that could be written manually.
12. Surface instruction products use the checked pattern parser generator.
13. Runtime instruction products never contain unresolved labels or other source-only data.
14. A composite parser is constructed from only the selected surface instructions.
15. `Resolve<SurfaceInstruction, Header>` is the explicit, type-checked bridge from parsed surface
    modules to runtime modules.

## Non-Goals

The first implementation deliberately leaves the following capabilities outside the core model:

- Roll back state automatically when an instruction faults.
- Permit a borrowed execution context to survive a parked instruction.
- Infer modeled time from how long host execution takes.
- Dynamically discover instructions at runtime.
- Require all component state transitions to be observable effects.
- Make every instruction portable across every machine architecture.
- Decide advanced borrowing or projection ergonomics before the first implementation demonstrates
  that they are needed.

## Instruction Set Shape: Products Selected Into a Sum

Surface syntax and runtime execution require different representations. They are separate product
types because source-level names are useful during parsing, while execution requires operands that
have already been resolved.

```rust
use vihaco_parser::Parse;

// Surface syntax: appears in SST and may contain source-level names.
#[derive(Parse)]
#[syntax_class(instruction, head = "control")]
#[pattern = "'conditional_branch `@` $when_true `,` `@` $when_false"]
pub struct SurfaceConditionalBranch {
    pub when_true: String,
    pub when_false: String,
}

// Runtime instruction: stored in the program image and executed.
pub struct ConditionalBranch {
    pub when_true: usize,
    pub when_false: usize,
}
```

The pattern parser constructs `SurfaceConditionalBranch` from source such as:

```text
control::conditional_branch @then, @otherwise
```

The resolver owns the label table and lowers those names to runtime program indices. The
instruction-specific part can remain a normal helper:

```rust
impl MyResolver {
    fn resolve_conditional_branch(
        &mut self,
        instruction: SurfaceConditionalBranch,
    ) -> eyre::Result<ConditionalBranch> {
        Ok(ConditionalBranch {
            when_true: self.label_index(&instruction.when_true)?,
            when_false: self.label_index(&instruction.when_false)?,
        })
    }
}
```

A composite selects products into two related sums:

```rust
pub enum MyMachineSurfaceInstruction {
    Push(surface::Push),
    Add(surface::Add),
    ConditionalBranch(surface::ConditionalBranch),
}

pub enum MyMachineInstruction {
    Push(runtime::Push),
    Add(runtime::Add),
    ConditionalBranch(runtime::ConditionalBranch),
}
```

The surface sum is parsed from SST. An implementation of
`Resolve<MyMachineSurfaceInstruction, MyHeader>` produces a
`Module<MyMachineInstruction, ...>` containing runtime instructions for the program image. The
runtime sum is dispatched during execution.

The mapping is not necessarily one-to-one. One surface instruction may expand into several runtime
instructions, including cases where one source operation selects different execution paths for
different resolved types. A runtime instruction may also be introduced during lowering without a
direct surface form. The invariant is that only runtime instructions reach components.

A component package may publish surface and runtime instruction catalogs. Those catalogs are not
automatically inherited by a machine; the composite explicitly selects both its accepted surface
syntax and its executable runtime instruction set. The resolver defines the mapping between the two
selected sets rather than requiring every surface operation to name exactly one runtime operation.

## Core Trait Shape

A runtime instruction identifies a fully resolved operation. The fact that a particular component
can execute that operation is a separate relationship. Keeping those facts separate allows one
instruction type to participate in several component implementations without giving the
instruction global knowledge of machine state.

The `Instruction` trait is a marker for runtime operations. Execution behavior belongs to
`Execute<I>`:

```rust
pub trait Instruction {
    // Surface parsing is a separate type-level concern.
}

pub trait Execute<I>
where
    I: Instruction,
{
    type Message: Message;
    type Effect: Effect;
    type Fault;

    fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> Result<Effects<Self::Effect>, Self::Fault>;
}
```

The essential relationship is:

```text
Component implements Execute<Instruction>
```

It replaces a component-wide associated instruction set:

```text
Component has one associated InstructionSet enum
```

Each supported operation receives its own implementation:

```rust
pub struct Stack<V> {
    values: Vec<V>,
}

pub struct Push<V> {
    pub value: V,
}

pub struct Drop;

impl<V: Clone> Execute<Push<V>> for Stack<V> {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = StackFault;

    fn execute(
        &mut self,
        instruction: &Push<V>,
        _message: NoMessage,
    ) -> Result<Effects<NoEffect>, StackFault> {
        self.push(instruction.value.clone())?;
        Ok(Effects::none())
    }
}

impl<V> Execute<Drop> for Stack<V> {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = StackFault;

    fn execute(
        &mut self,
        _instruction: &Drop,
        _message: NoMessage,
    ) -> Result<Effects<NoEffect>, StackFault> {
        self.pop()?;
        Ok(Effects::none())
    }
}
```

The syntax remains illustrative. `NoEffect`, for example, may be uninhabited because
`Effects<NoEffect>` never needs to construct a value.

### Why `Execute<I> for Component`

Placing execution on `Execute<I> for Component` keeps state ownership visible in the type system:

- The component is visibly responsible for maintaining its invariants.
- An instruction does not need one globally fixed `Component` associated type.
- The same instruction can have implementations for more than one component type.
- Associated message, effect, and fault types may depend on both the instruction and component.
- A stateless or pure instruction can use a zero-sized executor component.
- Tests can replace a component with a small alternative implementation when useful.

An `Instruction<C>` trait with `execute(&self, &mut C, ...)` can express the same call
mechanically, but it places component behavior on the instruction side and encourages broad
generic state bounds. The public model instead states the ownership relationship directly:
components execute operations.

### Instruction Identity and Route Identity

Instruction identity describes an operation, but not its complete path through a machine. A
composite may route the same instruction type to two fields:

```rust
pub enum MachineInstruction {
    PushOperand(stack::Push<Value>),
    PushCall(stack::Push<Value>),
}
```

Both variants contain the same instruction type and may target the same `Stack<Value>` component
type, but they target different instances and may have different message and effect policies.

The outer variant is therefore part of the route identity. The composite uses it to determine:

- Target field selection.
- Message resolution.
- Effect handling.
- Optional metadata made available to a driver.
- Tracing and diagnostics.
- Machine-local instruction metadata.

The generated dispatch must preserve that distinction. Whether it does so with direct match arms or
private marker types remains an internal choice; route identity itself is part of the architecture.

## Shape of Surface and Runtime Instructions

A surface instruction preserves the information written in SST:

```rust
#[derive(vihaco_parser::Parse)]
#[syntax_class(instruction, head = "control")]
#[pattern = "'branch `@` $target"]
pub struct SurfaceBranch {
    pub target: String,
}

#[derive(vihaco_parser::Parse)]
#[syntax_class(instruction, head = "control")]
#[pattern = "'call $arity `,` `@` $target"]
pub struct SurfaceCall {
    pub arity: u32,
    pub target: String,
}
```

A runtime instruction contains the resolved information required by execution:

```rust
pub struct Branch {
    pub target: usize,
}

pub struct Call {
    pub arity: u32,
    pub target: usize,
}
```

Surface instruction types therefore:

- Derive `vihaco_parser::Parse`.
- Own their pattern and dialect head.
- May contain labels, symbolic names, literals, and other source-level values.
- Are inputs to `Resolve<SurfaceInstruction, Header>`.
- Are never executed by components.
- Are not stored in the runtime program image.

Runtime instruction types:

- Contain no unresolved source symbols.
- Implement the runtime instruction marker.
- Are stored in the program image.
- Are the types accepted by `Execute<I>`.
- Need not implement `Parse`.

Neither representation carries runtime ownership or orchestration state:

- A reference to its component.
- A reference to the composite.
- A clock or scheduler.
- An event queue.
- A waker.
- A borrowed execution context.
- Runtime scheduler state.

Runtime instructions may contain resolved semantic configuration such as:

- An arithmetic type.
- A local index.
- A resolved program index.
- A channel identifier.
- An immediate value.
- An operation mode.

Information that depends on live machine state belongs in the runtime message rather than either
instruction representation.

## Shape of a Component

A component owns one coherent domain of state and the operations that preserve that domain's
invariants. Its responsibilities are to:

- Store one coherent domain of state.
- Expose invariant-preserving domain methods.
- Implement `Execute<I>` for the individual runtime instructions it supports.
- Implement reset, loading, observation, or resource interfaces when those responsibilities
  actually belong to the component.
- Avoid exposing its internal fields merely so generated code can mutate them.

It does not:

- Define one enum containing all supported instructions.
- Expose one dispatch method matching every instruction.
- Contribute all its instructions to any composite that contains it.
- Know which machine-local route name a composite assigns to an instruction.
- Know which other components receive its effects.
- Know the runtime's clock or scheduling policy.

The component's public catalog describes which runtime operations have implementations for its
type. The composite separately decides which surface operations are accepted, how they resolve,
which runtime operations exist in the machine, and which component instance receives each one.

### Components That Are Also Resources

Stacks, heaps, channels, and clocks may serve both as instruction targets and as resources used by
message resolution or effect handling. Their ordinary Rust methods remain the
invariant-preserving boundary in both roles:

```rust
impl<V> Stack<V> {
    pub fn push(&mut self, value: V) -> Result<(), StackFault> {
        // Preserve capacity, frame, and ownership invariants here.
    }

    pub fn pop(&mut self) -> Result<V, StackFault> {
        // Preserve underflow and frame-boundary invariants here.
    }
}
```

Calling these methods from composite wiring does not expose `Push` or `Pop` as program
instructions. Program visibility changes only when the composite selects a route into its machine
instruction sum.

## Shape of a Composite

A composite is the architectural junction between reusable component behavior and one concrete
machine. It is:

- The product of its component fields.
- The authority that selects its instruction sum.
- The owner of machine-level routing.
- The boundary for cross-component data movement.
- The place where one-instruction route policy becomes concrete.

Program iteration, program-counter advancement, modeled time, and selection of the next runnable
machine are separate concerns. The driver owns iteration, readiness, and modeled time. Cursor
advancement belongs either to the driver or to an explicitly modeled component; it does not become
an implicit composite responsibility merely because the composite owns instruction dispatch.

For example:

```rust
pub struct MyMachine {
    operand_stack: Stack<Value>,
    call_stack: Stack<Value>,
    arithmetic: ArithmeticUnit,
    heap: Heap<Value>,
    channels: Channels<Value>,
    program: Executor,
    clock: ChildClock,
}
```

Merely placing these fields in the struct does not add surface or runtime instructions. The
composite declares its accepted surface instructions and executable runtime routes separately:

```rust
machine! {
    composite MyMachine {
        operand_stack: Stack<Value>,
        call_stack: Stack<Value>,
        arithmetic: ArithmeticUnit,
        heap: Heap<Value>,
        channels: Channels<Value>,
        program: Executor,
        clock: ChildClock,
    }

    surface_instructions {
        Push => stack::surface::Push;
        Add => arithmetic::surface::Add;
        Allocate => heap::surface::Allocate;
        ConditionalBranch => control_flow::surface::ConditionalBranch;
        Send => channel::surface::Send;
    }

    runtime_instructions {
        Push => stack::runtime::Push on operand_stack;

        Add => arithmetic::runtime::Add on arithmetic {
            message from operand_stack;
            effects to operand_stack;
        }

        Allocate => heap::runtime::Allocate on heap {
            message from operand_stack;
            effects to operand_stack;
        }

        ConditionalBranch => control_flow::runtime::ConditionalBranch on program {
            effects to program;
        }

        Send => channel::runtime::Send on channels {
            message from operand_stack;
            effects to clock;
        }
    }
}
```

The syntax is illustrative; the architecture requires the following properties:

- Each surface instruction is explicitly admitted to SST parsing.
- The machine's `Resolve` implementation may lower each surface instruction to one or more of the
  selected runtime instructions.
- Each runtime instruction has a stable machine-local name.
- Each runtime instruction selects exactly one primary execution target.
- Message and effect wiring is route-specific.

### Generated Surface and Runtime Sums

From those declarations, the composite produces one sum for each instruction level:

```rust
pub enum MyMachineSurfaceInstruction {
    Push(stack::surface::Push),
    Add(arithmetic::surface::Add),
    Allocate(heap::surface::Allocate),
    ConditionalBranch(control_flow::surface::ConditionalBranch),
    Send(channel::surface::Send),
}

pub enum MyMachineInstruction {
    Push(stack::runtime::Push),
    Add(arithmetic::runtime::Add),
    Allocate(heap::runtime::Allocate),
    ConditionalBranch(control_flow::runtime::ConditionalBranch),
    Send(channel::runtime::Send),
}
```

The pattern parser generator builds the parser for `MyMachineSurfaceInstruction` from the selected
surface patterns. The resolver builds a module containing `MyMachineInstruction` values.

The surface sum defines what the parser accepts. The runtime sum defines what `step` can dispatch.
Neither sum inherits unselected instructions from component catalogs.

### Nested Composites

A nested composite behaves like a component at its parent's boundary while retaining its own
instruction-selection boundary. It exports only the surface and runtime operations that it has
chosen to make public. The parent may:

- Route a nested instruction set as a whole when that is intentional.
- Select explicit public operations exported by the child.
- Treat the child as a resource or effect handler without exposing its runtime instructions.

Containment never implies recursive instruction inheritance.

