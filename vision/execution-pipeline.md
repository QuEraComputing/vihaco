# Instruction Pipeline

Loading and execution are separate pipelines joined by the runtime program. The first translates
source into resolved instructions; the second executes one of those instructions against live
machine state.

## Surface Resolution Pipeline

SST loading follows this path:

```text
SST text
    -> pattern parser
    -> ParsedModule<ModuleSyntax>
    -> Resolve<ModuleSyntax>
    -> Module<RuntimeInstruction, Constant, RuntimeType, Info>
    -> runtime program image
```

An executable `composite!` can now own the surface-to-runtime boundary. Its
`syntax` block generates the surface instruction parser, its named resolver
methods lower payload-bearing instructions, and a `#[program]` field receives
the resulting module through `BuildProgramModule` and
`InstallProgramModule`:

```text
SST section
    -> composite::syntax::Instruction parser
    -> ParsedModule<ModuleSyntax>
    -> generated composite lowering
    -> BuildProgramModule
    -> load_parsed(parsed, ContextHandle)
    -> installed runtime program
```

The generic `Resolve` trait remains available for standalone or more
application-specific module construction. The generated composite path is a
convenience for executable composites and does not require the runtime
instruction enum to implement bytecode encoding.

`SurfaceType`, `Constant`, and `RuntimeType` are author-defined products rather than vihaco enums.
`Resolve<ModuleSyntax>` owns every transformation that requires
module-wide source context:

- Building and consulting label tables.
- Turning `@label` references into fixed-width `InstructionIndex` values.
- Interning strings.
- Expanding surface sugar into one or more runtime instructions.
- Validating source-level types and declarations.
- Interpreting typed or unresolved author-defined literals.
- Selecting explicit conversions required by the source language.

At the trait boundary, resolution consumes a parsed surface module and produces a runtime module:

```rust
pub trait Resolve<S> {
    type Module;

    fn resolve_module(
        &mut self,
        parsed: ParsedModule<S>,
    ) -> eyre::Result<Self::Module>;
}
```

The resolver may delegate individual variants to ordinary helper methods, but the trait remains
module-oriented. That wider view allows it to collect labels before lowering branches, expand one
surface instruction into several runtime instructions, and assign labels to final runtime indices
after expansion. Resolution finishes while constructing the program image; execution never
performs source resolution.

## Runtime Execution Pipeline

One-instruction execution starts with a runtime instruction supplied by a containing runtime root
or direct caller:

```text
supplied runtime instruction
    -> select machine route
    -> resolve runtime message
    -> execute against the route's component
    -> handle immediate internal effects
    -> return the step outcome and any root-facing work
```

The composite macro generates one outer `step` dispatcher from the selected runtime routes. Users
do not hand-write this match. A representative expansion is:

```rust
fn step(
    &mut self,
    instruction: &MyMachineInstruction,
) -> Result<Execution, MachineFault> {
    match instruction {
        MyMachineInstruction::Push(instruction) => {
            let message = NoMessage;
            let effects = self.operand_stack.execute(instruction, message)?;
            self.handle_push_effects(effects)
        }
        MyMachineInstruction::Add(instruction) => {
            let message = self.resolve_add_message(instruction)?;
            let effects = self.arithmetic.execute(instruction, message)?;
            self.handle_add_effects(effects)
        }
        MyMachineInstruction::Allocate(instruction) => {
            let message = self.resolve_allocate_message(instruction)?;
            let effects = self.heap.execute(instruction, message)?;
            self.handle_allocate_effects(effects)
        }
        MyMachineInstruction::ConditionalBranch(instruction) => {
            let message = self.resolve_conditional_branch_message(instruction)?;
            let effects = self.program.execute(instruction, message)?;
            self.handle_conditional_branch_effects(effects)
        }
        MyMachineInstruction::Send(instruction) => {
            let message = self.resolve_send_message(instruction)?;
            let effects = self.channels.execute(instruction, message)?;
            self.handle_send_effects(effects)
        }
    }
}
```

Every generated match arm performs the same three runtime stages: message resolution, component
execution, and effect handling. The concrete resolver, target field, instruction type, effect
handler, and fault conversions come from that route's composite declaration. `NoMessage` and
`NoEffect` routes reduce to their trivial forms. Directly generated match arms are the initial
representation; further dispatch abstractions are justified only by demonstrated repetition or
compiler constraints.

`step` does not inherently fetch an instruction, iterate a program, define what happens next, or
advance modeled time. In the reference runtime, each CPU owns its program counter and route
handling mutates that modeled component during `step`. That is an explicit machine configuration
rather than universal step behavior.

### Stage 1: Message Resolution

Runtime message resolution supplies the owned, execution-time information that is intentionally
absent from the instruction. It is distinct from
`Resolve<ModuleSyntax>`: module resolution transforms parsed source
into a runtime program, while message resolution reads live machine state for an instruction that
is already fully resolved.

The composite route owns this stage because only the composite knows:

- Which stack supplies operands.
- Which program image owns interned strings and function metadata.
- Which register file is active.
- Which frame is current.
- Which privilege or validation policy applies.
- Which clock, resource, or external input belongs to this machine.

For a stack-machine `Add`:

```text
resolve:
    pop rhs from operand_stack
    pop lhs from operand_stack
    construct Operands { lhs, rhs }
```

For `Print`:

```text
resolve:
    obtain or consume the value selected by the machine's print policy
    resolve any interned string data
    construct PrintMessage
```

For `Send`:

```text
resolve:
    consume the value from the operand stack
    retain the channel identifier stored in the instruction
    construct SendMessage
```

Any route that may park requires an owned message. A synchronous route may borrow data when the
borrow is guaranteed to end before `step` returns.

#### `NoMessage`

Instructions with no live input use `NoMessage`, allowing generation to omit a user-written
resolver. A route's documentation still states whether its nontrivial resolution reads, copies, or
consumes machine state.

#### Resolution Sources

Message resolution has three declaration forms, ordered by how much the composite macro can
generate on the author's behalf:

- A route with no live input uses `NoMessage`, and generation omits the resolver entirely.
- A route whose entire message comes from one component uses `message from <field>`. The component
  supplies the message through a reusable capability, and generation emits a forwarding resolver.
- A route whose message is assembled from several components, or in an order the grammar does not
  imply, names a route-local resolver method with `message with <method>`.

The single-source form is backed by a component capability that is the input dual of effect
absorption:

```rust
pub trait Supply<M> {
    type Fault;

    fn supply(&mut self) -> Result<M, Self::Fault>;
}
```

A stack implements `Supply<Operands<V>>` once, and every `message from <stack>` route reuses it.
The generated resolver is then a forwarding call:

```rust
fn resolve_integer_add_message(
    &mut self,
    _instruction: &Add,
) -> Result<Operands<MachineValue>, MachineFault> {
    Ok(self.operand_stack.supply()?)
}
```

Resolution is expressed as a route method rather than a route-parameterized trait. Effect handling
earns its trait from two properties that message resolution does not share: component execution
returns an `Effects<E>` stream that a generic drainer folds over, and the generated route marker's
locality permits a component to carry an effect handler directly. A resolved message is instead a
single owned value, and resolution reads across several composite fields, so it can neither be
folded nor relocated onto a component. The reusable part of resolution therefore lives in
`Supply<M>`, while selection between same-typed messages is expressed by distinct resolver methods
rather than by a marker type.

### Stage 2: Component Execution

Component execution applies the resolved operation to its single selected state owner. The call is
synchronous and may:

- Validate the instruction against component state.
- Mutate the selected component.
- Produce zero, one, or many typed effects.
- Fault.

The call does not:

- Borrow arbitrary fields from the composite.
- Retain a borrow into the composite after returning.
- Block on wall-clock I/O.
- Suspend internally.
- Directly schedule future machine steps.
- Choose another component instance by name.

#### Owner-Local Mutation

Direct mutation is appropriate when the selected component owns the affected invariant. Typical
examples include:

- `stack::Push` pushes onto its selected stack.
- `stack::Drop` pops and discards from its selected stack.
- `stack::Dup` duplicates through the stack's invariant-preserving method.
- `heap::Deallocate` deallocates a reference already delivered in its message.
- `counter::Increment` mutates its selected counter.

Turning these operations into effects that immediately return to the same component adds
indirection without making ownership clearer.

#### Cross-Component Mutation

State owned by any other component crosses the composite boundary:

- Input from another component is resolved into the message.
- Output intended for another component is emitted as an effect.

This keeps cross-component dataflow visible in the machine definition.

#### Pure Instructions

A semantic operation with no mutable state still fits the same relationship:

```rust
pub struct Add;

pub struct Operands<V> {
    pub lhs: V,
    pub rhs: V,
}

pub struct ArithmeticUnit<V> {
    marker: std::marker::PhantomData<fn() -> V>,
}

impl<V> Execute<Add> for ArithmeticUnit<V>
where
    V: TryAdd,
{
    type Message = Operands<V>;
    type Effect = ValueResult<V>;
    type Fault = V::Error;

    fn execute(
        &mut self,
        _instruction: &Add,
        message: Operands<V>,
    ) -> Result<Effects<ValueResult<V>>, V::Error> {
        let value = message.lhs.try_add(&message.rhs)?;
        Ok(Effects::one(ValueResult(value)))
    }
}
```

A zero-sized executor gives the operation an execution target without teaching arithmetic about
stack layout. Pure operations remain a deliberate special case rather than a second execution
pipeline.

### Stage 3: Effect Handling

Effect handling gives machine-local meaning to the owned values produced by component execution.
An effect may represent:

- A result to place in another component.
- A control-flow request.
- A resource request.
- A diagnostic-facing event.
- A scheduling request.
- A request that may park the machine.

Examples include:

```rust
pub struct ValueResult<V>(pub V);
pub struct JumpTo(pub JumpTarget);
pub struct Invoke<I>(pub I);
pub struct SendValue<V> {
    pub channel: ChannelId,
    pub value: V,
}
pub struct Receive {
    pub channel: ChannelId,
}
```

#### Route-Specific Handling

The runtime route that produced an effect selects its handling policy. The Rust effect type
describes semantic meaning, but it does not by itself identify a destination within one composite.

##### Route Identity

Each entry in a composite's `runtime` declaration defines one route. Its
composite-local name identifies the complete execution path:

```text
machine instruction variant
    -> runtime instruction type
    -> target component field
    -> message resolver
    -> effect handler
    -> route outcome and root-facing work
```

The generated machine instruction variant is the canonical route identity during dispatch. The
`step` match selects the message resolver, target component, effect handler, and route outcome
associated with that variant. Effect handling therefore remains within the route selected by the
outer machine instruction; it is not resolved globally from `Effect`.

##### Resolution Selects the Runtime Route

The composite declaration defines the available runtime routes, and the composite macro gives each
one a machine instruction variant.
`Resolve<MachineModuleSyntax>` selects among those variants
while lowering surface instructions into the runtime module.

This separation allows one SST operation to select a machine-specific execution path after its
source operands are resolved. A typed addition illustrates the distinction:

```rust
#[derive(vihaco_parser::Parse)]
#[syntax_class(instruction)]
#[pattern = "'add $ty"]
pub struct SurfaceAdd {
    pub ty: ArithmeticSurfaceType,
}
```

`ArithmeticSurfaceType` is supplied by the arithmetic or machine data-model author. It is not a
vihaco core type.

The same surface product parses both of these forms:

```text
arithmetic::add integer
arithmetic::add address
```

The composite can provide distinct runtime routes for the supported resolved types:

```rust
runtime {
    IntegerAdd => arithmetic::runtime::Add on integer_arithmetic {
        message from operand_stack;
        effects to operand_stack;
    }

    AddressAdd => arithmetic::runtime::Add on address_arithmetic {
        message from address_stack;
        effects to address_stack;
    }
}
```

The composite macro generates the available runtime variants:

```rust
pub enum MyMachineInstruction {
    IntegerAdd(arithmetic::runtime::Add),
    AddressAdd(arithmetic::runtime::Add),
}
```

The resolver chooses the variant that enters the runtime module. An instruction-specific helper
called by the module-level `Resolve` implementation may take this shape:

```rust
fn resolve_add(
    &mut self,
    instruction: SurfaceAdd,
) -> eyre::Result<MyMachineInstruction> {
    match self.resolve_type(instruction.ty)? {
        ArithmeticType::Integer => Ok(MyMachineInstruction::IntegerAdd(
            arithmetic::runtime::Add,
        )),
        ArithmeticType::Address => Ok(MyMachineInstruction::AddressAdd(
            arithmetic::runtime::Add,
        )),
        ty => Err(eyre::eyre!("addition is not supported for {ty}")),
    }
}
```

The complete transition is:

```text
surface Add { ty }
    -> Resolve validates and resolves ty
    -> IntegerAdd(Add) or AddressAdd(Add)
    -> step dispatches to the selected arithmetic component
    -> route-specific handling returns the result to the selected stack
```

The parser does not select a component, and `Execute<Add>` does not inspect the composite to choose
one. Resolution makes that architectural decision once, while it has source and type context. The
resulting runtime route then carries the decision through execution and effect handling.

##### Type Compatibility and Conversion

Route wiring preserves Rust boundary types. Moving an effect or message from one component to
another does not trigger a cast. A route whose producer emits `i64` cannot target a handler that
requires `f64` unless the author selects a conversion instruction, component, or named handler.

When the source language defines an implicit coercion, the module resolver makes it explicit in the
runtime program or converts a source constant during resolution. Checked, saturating, wrapping,
lossy, and bitwise conversions remain distinct operations. Generated dispatch and effect forwarding
never choose conversion semantics.

##### Same Effect Type, Different Machine Semantics

The same `Add` runtime instruction can therefore appear through two routes:

```rust
runtime {
    IntegerAdd => arithmetic::runtime::Add on integer_arithmetic {
        message from operand_stack;
        effects to operand_stack;
    }

    AddressAdd => arithmetic::runtime::Add on address_arithmetic {
        message from address_stack;
        effects to address_stack;
    }
}
```

Both routes contain the same runtime instruction type and produce
`ValueResult<MachineValue>`, where `MachineValue` is an illustrative author-defined carrier. They
execute on different component instances and apply their effects to different stacks:

```text
IntegerAdd -> ValueResult<MachineValue> -> operand_stack
AddressAdd -> ValueResult<MachineValue> -> address_stack
```

A single `Handle<ValueResult<MachineValue>> for MyMachine` implementation cannot distinguish these
policies. The effect type intentionally describes the semantic result—an operation produced a
value—without naming a destination in the composite. Adding the destination to `ValueResult` would
couple the arithmetic component to a particular machine layout. Replacing it with a machine-wide
effect enum would reintroduce broad, composite-specific effect types.

Route-specific handling preserves both abstractions: components emit semantic effects, while the
composite assigns destinations and machine-local behavior.

##### Generated Route Representation

`IntegerAdd` and `AddressAdd` originate in the composite's `runtime` declaration.
Generation turns them into variants of the machine runtime sum:

```rust
pub enum MyMachineInstruction {
    IntegerAdd(arithmetic::runtime::Add),
    AddressAdd(arithmetic::runtime::Add),
}
```

The outer variant provides route identity inside the generated `step` match. Direct match-arm
generation can inline the corresponding effect handling without introducing another public type.

Code generation may factor effect handling through `HandleEffects<R>`. In that representation, the
composite macro emits zero-sized internal marker types derived from the route names:

```rust
#[doc(hidden)]
struct IntegerAddRoute;

#[doc(hidden)]
struct AddressAddRoute;
```

`IntegerAddRoute` and `AddressAddRoute` are generated from machine-local route declarations; they
are not provided by the arithmetic component or the `Add` instruction. They carry no runtime data
and are not part of the component API. Their purpose is to preserve the distinction between
otherwise identical instruction and effect types when handling is expressed through a generic
trait.

Route identity is required; marker types are not. They are one private representation and disappear
when direct match arms already retain the distinction.

##### Effect Handling Contract

Trait-based factoring uses a framework contract such as:

```rust
trait HandleEffects<R> {
    type Effect;
    type Error;

    fn handle_effects(
        &mut self,
        effects: Effects<Self::Effect>,
    ) -> Result<Execution, Self::Error>;
}
```

The composite macro implements this trait for each declaratively wired route. The generated
implementations for `IntegerAdd` and `AddressAdd` are equivalent to:

```rust
impl HandleEffects<IntegerAddRoute> for MyMachine {
    type Effect = ValueResult<MachineValue>;
    type Error = MachineFault;

    fn handle_effects(
        &mut self,
        effects: Effects<Self::Effect>,
    ) -> Result<Execution, Self::Error> {
        for ValueResult(value) in effects {
            self.operand_stack.push(value)?;
        }
        Ok(Execution::Complete)
    }
}

impl HandleEffects<AddressAddRoute> for MyMachine {
    type Effect = ValueResult<MachineValue>;
    type Error = MachineFault;

    fn handle_effects(
        &mut self,
        effects: Effects<Self::Effect>,
    ) -> Result<Execution, Self::Error> {
        for ValueResult(value) in effects {
            self.address_stack.push(value)?;
        }
        Ok(Execution::Complete)
    }
}
```

The generated `step` arm invokes the implementation associated with its route:

```rust
MyMachineInstruction::IntegerAdd(instruction) => {
    let message = self.resolve_integer_add_message(instruction)?;
    let effects = self.integer_arithmetic.execute(instruction, message)?;
    <Self as HandleEffects<IntegerAddRoute>>::handle_effects(self, effects)
}
```

`resolve_integer_add_message` performs runtime message resolution after the runtime route has
already been selected. It supplies operands from `operand_stack`; it is distinct from the
module-level `Resolve` pass that selected `IntegerAdd`.

The generated implementation may use a fully qualified trait call, a private method, or an inline
match-arm body. All three preserve the same public model: the current machine instruction variant
selects exactly one effect-handling policy.

##### Component Absorb and Observe Capabilities

Declarative effect forwarding is backed by reusable component capabilities, so a generated route
handler names a destination without carrying handler behavior. A component that consumes an effect
into its own state implements `Absorb`; a component that only reads an effect implements `Observe`:

```rust
pub trait Absorb<E> {
    type Fault;

    fn absorb(&mut self, effect: E) -> Result<(), Self::Fault>;
}

pub trait Observe<E> {
    fn observe(&mut self, effect: &E);
}
```

`Absorb` is the effect-side dual of `Supply`. Both are written once per component, are machine
agnostic, and preserve the component's invariants exactly as its ordinary methods do. A generated
`effects to <field>` handler forwards through `Absorb`, so the composite still owns the routing
decision while the component still owns how its state changes.

Effect handling then has the same three declaration forms as message resolution:

- `effects to <field>` forwards the effect into one component through `Absorb`.
- `effects to <field>, observed by <field>...` delivers the effect to one consuming component and
  any number of read-only observers.
- `effects with <method>` names a route-local handler for anything the forwarding forms cannot
  express.

Because the generated route marker is a type local to the machine crate, it may also appear in the
trait reference of a handler implemented directly on a component. That locality is what permits a
component to own the effect handler for a specific route without violating the orphan rule, while
two routes that produce the same effect type remain distinct implementations. Handling on the
composite remains the default; component-side handling is available where a component should own its
own effect policy.

##### Multiple Handlers

One operation may need to reach more than one component. This resolves into one of three shapes, and
only the composite route decides which applies:

- Distinct effects. When the destinations need different information, component execution emits one
  effect that carries the whole outcome, and the route handler distributes its fields to each
  component through their own `Absorb` implementations. The field-to-component mapping is semantic,
  so this uses `effects with <method>`.
- Observation. When several components need the same value but only one consumes it, the consuming
  component uses `Absorb` and the rest use `Observe`. Observers run before the consumer takes
  ownership, so no clone is required.
- Broadcast. When several components must each consume the same value, the effect implements `Clone`
  and generation clones it for every destination but the last. Generation admits `effects to a, b`
  only when the effect is `Clone`; otherwise it directs the author to observation or an explicit
  handler.

Delivery order is deterministic: observers precede the consumer, and multiple consumers receive the
effect in declaration order. Routing selects a destination from the effect's type and the route, not
from a runtime value. A route that produces heterogeneous outputs carries them in a carrier product
or a sum the handler matches, rather than distributing by inspecting effect contents.

##### Code-Generation Boundary

Code generation supports the ownership model without becoming part of it:

| Owner | Responsibility |
|---|---|
| Framework | Defines `Effects`, `Execution`, and, if useful, the generic `HandleEffects<R>` contract |
| User | Defines component state, `Execute<I>` implementations, named routes, the `Resolve` implementation that selects runtime route variants, and custom machine policy |
| Composite macro | Generates the machine instruction variants, exhaustive `step` dispatch, internal route identities when needed, declarative effect forwarding, and fault conversions |
| User, for custom handling | Writes a named handler method when the route cannot be expressed as simple forwarding |
| Composite macro, for custom handling | Generates the route-specific dispatch that calls the user's named method |

A declaration such as `effects to operand_stack` contains enough information to generate ordinary
forwarding; it does not require the user to write `HandleEffects<IntegerAddRoute>`.

A route with custom semantics names a user-defined handler:

```rust
effects with handle_special_result;
```

The macro verifies the handler's effect and outcome types and generates its route-specific call.
A manually implemented composite can implement `HandleEffects<R>` directly, while a generated
composite avoids repetitive route implementations.

##### Route Provenance

Route provenance matters whenever two identical effect types receive different machine semantics:

- The destination component instance.
- Whether a value is pushed, observed, discarded, or transformed.
- Whether handling completes immediately or parks the machine.
- Whether a scheduling request remains internal or crosses a child-to-root boundary.
- Which fault conversion and diagnostic context are attached.
- Which handlers receive the effect.

Effects therefore do not enter an unlabelled machine-wide queue before route handling. Deferred
work retains either equivalent route provenance or an already-resolved continuation. Once route
handling converts the effect into a resource command, diagnostic event, or root scheduling request,
ordinary typed handlers can continue it without the original route marker.

#### Effect Ordering

Effect continuation is deterministic:

- `Effects::Many` is handled left-to-right.
- Follow-up effects are continued depth-first.
- Route handling produces one final `Execution` outcome.
- A suspending operation occurs only in tail position.

If one effect parks the machine, the handler must already own or register everything needed to
resume. No borrowed data from resolution or component execution may survive.

#### Commands and Events

Commands and events share the same typed transport but carry different meanings:

- A command effect asking another component or resource to do something.
- A fact/event describing something that already happened during direct mutation.

Naming and documentation must preserve that distinction. A fact records something that already
happened; it is not a deferred mutation merely because it travels through `Effects`. This matters
for tracing, diagnostic handlers, replay, and future event-sourced runtimes.

#### No Effects

Owner-local mutation may complete with an empty `Effects<NoEffect>`. The route still returns a step
outcome, and a containing runtime can still account for time or select more work. Neither effect
production nor a clock is required by `step`.
