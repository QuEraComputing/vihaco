# Concepts in the Demo's `vihaco` Layer

The files under [`demo/vihaco`](./demo/vihaco) contain the small
contracts used to express an instruction pipeline. They are not a complete framework API. They are
a concrete sketch of the relationships the eventual framework and its macros need to generate.

This document explains each concept independently. The examples use deliberately small domains
such as a counter, a mailbox, and a door; they are not taken from the demo machine.

## Concept status against current vihaco

The contracts described here are a design sketch, not a claim that every concept is already part
of vihaco core. The following map shows how they relate to the current implementation:

| Concept document | Current vihaco equivalent | Comparison |
|---|---|---|
| `Effects<E>` | [`effect.rs`](../../crates/vihaco/src/effect.rs) | Already exists closely. Current `Effects` supports `None`, `One`, `Many`, mapping, flattening, and iteration. |
| `NoMessage` | [`runtime/marker.rs`](../../crates/vihaco/src/runtime/marker.rs) | The demo uses a named `NoMessage` type. Current vihaco has a general `Message` marker trait and implements it for `()`, but does not provide the same named convention. |
| `Execution` | [`vihaco-cpu/src/outcome.rs`](../../crates/vihaco-cpu/src/outcome.rs) | Current `StepOutcome` models CPU outcomes such as `Continue`, `Breakpoint`, `Halt`, and `Return`. It is broader and different from the demo's `Complete`/`Parked` suspension state. |
| `StepResult<E>` | [`runtime/generated.rs`](../../crates/vihaco/src/runtime/generated.rs) | The demo groups effects and execution state in `StepResult`. Current generated components return `Result<Effects<E>>`; execution state is not paired with effects in one core type. |
| `Execute<I>` | [`#[component]`](../../crates/vihaco-derive/src/attr_component.rs) and [`GeneratedComponent`](../../crates/vihaco/src/runtime/generated.rs) | The demo uses one `Execute<I>` implementation per instruction type. Current vihaco uses one component-level `execute` method over an instruction type, message type, and effect type, then generates `GeneratedComponent`. |
| `Supply<M>` | [`StackMemory`](../../crates/vihaco/src/traits/machine.rs) and component-specific APIs | The demo has a general typed message-supply capability. Current vihaco has specialized state-access traits such as `StackMemory`, but no general `Supply<M>` trait. |
| `Absorb<E>` | [`EffectSink<E>`](../../crates/vihaco/src/traits/event_sink.rs) | Both describe effect destinations, but `EffectSink` emits into a sink and has no fault result. The demo's `Absorb` models a component actively consuming and applying an effect. |
| `Observe<E, R>` | [`runtime::Observe<E>`](../../crates/vihaco/src/runtime/observe.rs) | Current observation can return follow-up effects, but it has no `Route` type parameter. |
| `Handle<E, R>` | No direct equivalent | Current vihaco has `EffectSink` and generated dispatch, but not a route-parameterized handler with a default `Absorb` delegation path. |
| `Route` | Generated composite/device metadata | Current [`#[composite]`](../../crates/vihaco-derive/src/attr_composite.rs) and `Machine` machinery generate device and instruction routing, but the explicit per-route marker trait in this document is not currently a public core concept. |
| `Resume<C>` | No direct core equivalent | The demo models owned completion of a parked operation explicitly. Current runtime traits do not yet expose the same generic resume contract. |
| `component!` instruction expansion | [`#[component]`](../../crates/vihaco-derive/src/attr_component.rs) | These are different layers. Current `#[component]` adapts an implementation over one instruction enum into `GeneratedComponent`; it does not split an enum into individual instruction structs. |
| `machine!` effect fanout | [`#[composite]`](../../crates/vihaco-derive/src/attr_composite.rs), [`#[observe]`](../../crates/vihaco-derive/src/attr_observe.rs), and generated dispatch | Current macros generate machine/device structure and observation support, but the planned `effects { observe ...; to ...; with ...; }` syntax does not currently exist. |

The status of these relationships can be summarized as:

- **Current** — the repository already provides approximately the same concept.
- **Partial** — the repository provides a related mechanism with different ownership or type
  boundaries.
- **Proposed** — the concept is demonstrated here but is not currently part of vihaco core.
- **Planned macro surface** — the concept describes intended syntax or code generation that is not
  implemented yet.

### The important `Execute<I>` difference

The conceptual design has individually executable instruction products:

```rust
impl Execute<Add> for ArithmeticUnit {
    type Message = BinaryOperands;
    type Effect = ValueResult;
    type Fault = ArithmeticFault;

    fn execute(
        &mut self,
        instruction: &Add,
        message: BinaryOperands,
    ) -> Result<StepResult<ValueResult>, ArithmeticFault> {
        // execute one instruction product
    }
}
```

Current vihaco instead uses a component-level instruction sum:

```rust
#[component(
    instruction = RuntimeInstruction,
    message = CPUMessage,
    effect = StepOutcome,
)]
impl CPU {
    fn execute(
        &mut self,
        instruction: RuntimeInstruction,
        message: CPUMessage,
    ) -> eyre::Result<Effects<StepOutcome>> {
        match (instruction, message) {
            // current component-level dispatch
        }
    }
}
```

The current `#[component]` macro generates an implementation of `GeneratedComponent`:

```rust
trait GeneratedComponent {
    type Instruction;
    type Message;
    type Effect;

    fn execute_generated(
        &mut self,
        instruction: Self::Instruction,
        message: Self::Message,
    ) -> eyre::Result<Effects<Self::Effect>>;
}
```

The conceptual direction is therefore more granular than the current implementation. It aims to
move instruction matching and each instruction's message/effect relationship into separate
`Execute<I>` implementations.

## The pipeline at a glance

An instruction usually crosses four boundaries:

```text
component state --Supply--> message --Execute--> effects + execution state
                                                        |
                                      Observe (borrow) --+
                                      Handle (consume) --+
```

If execution cannot finish immediately, the component returns `Parked`. Later, an owned completion
is given to `Resume`, which produces another `StepResult`. The parent composite owns the sequencing
and decides what to do with the result; the instruction component owns its local invariants.

The `Effects<E>` type in the examples comes from the surrounding framework. It represents zero,
one, or many effects. The contracts in this directory specify how those effects are produced and
consumed, but do not define the collection itself.

## `NoMessage`: making “no input” explicit

`NoMessage` is a marker type for an instruction whose execution needs no value resolved from the
runtime. It is preferable to using `()` everywhere because it gives the route a named, searchable
contract and leaves room for framework-level policies around message resolution.

For example, a `ResetDisplay` instruction can state that it has no runtime input:

```rust
struct ResetDisplay;

impl Execute<ResetDisplay> for Display {
    type Message = NoMessage;
    type Effect = DisplayReset;
    type Fault = DisplayFault;

    fn execute(
        &mut self,
        _instruction: &ResetDisplay,
        _message: NoMessage,
    ) -> Result<StepResult<DisplayReset>, DisplayFault> {
        self.clear_pixels();
        Ok(StepResult {
            effects: Effects::one(DisplayReset),
            execution: Execution::Complete,
        })
    }
}
```

The important distinction is between “no message is needed” and “the message happens to be an
empty value.” A route requiring a `UserId` cannot accidentally be wired to `NoMessage`, and a
component that supplies messages can be checked against the exact instruction type.

## `Execution`: whether the instruction finished

`Execution` has two states:

```rust
enum Execution {
    Complete,
    Parked,
}
```

`Complete` means the parent may advance the instruction stream. `Parked` means the current
instruction is still the active instruction and must be resumed or otherwise resolved before the
parent advances.

This state is separate from effects. An instruction can emit an effect and still park. For
example, a `WaitForDoor` operation may emit a `WaitRegistered` fact while it waits for an external
signal:

```text
effects:   [WaitRegistered]
execution: Parked
```

Keeping these dimensions separate prevents a parent from inferring completion merely because an
effect was emitted. It also means an effect handler can schedule a wakeup without having to mutate
the child program counter.

## `StepResult<E>`: the result of starting or resuming work

`StepResult<E>` groups the effects produced by one execution attempt with its completion state:

```rust
struct StepResult<E> {
    effects: Effects<E>,
    execution: Execution,
}
```

The same shape is returned by `Execute` and `Resume`. That is useful because the parent can run
the same observation and handling pipeline after an instruction starts and after a parked
instruction wakes.

Consider a queue read. A successful read might return:

```text
StepResult {
    effects: [ItemRead(42)],
    execution: Complete,
}
```

An empty queue might return:

```text
StepResult {
    effects: [ReaderParked(reader_id)],
    execution: Parked,
}
```

The parent does not need separate “normal result” and “suspension result” plumbing. It still
processes effects, then branches on `execution`.

## `Execute<I>`: component-owned instruction behavior

`Execute<I>` says that a component can execute one particular instruction type:

```rust
trait Execute<I> {
    type Message;
    type Effect;
    type Fault;

    fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault>;
}
```

The instruction, message, effect, and fault are associated with this specific implementation.
That is more precise than giving a component one large enum and one universal message type.

For a simple `AddCredit` operation:

```rust
struct AddCredit;
struct CreditAmount(u64);
struct CreditChanged(u64);

impl Execute<AddCredit> for Wallet {
    type Message = CreditAmount;
    type Effect = CreditChanged;
    type Fault = WalletFault;

    fn execute(
        &mut self,
        _instruction: &AddCredit,
        CreditAmount(amount): CreditAmount,
    ) -> Result<StepResult<CreditChanged>, WalletFault> {
        self.balance = self
            .balance
            .checked_add(amount)
            .ok_or(WalletFault::Overflow)?;
        Ok(StepResult {
            effects: Effects::one(CreditChanged(self.balance)),
            execution: Execution::Complete,
        })
    }
}
```

This allows the same `AddCredit` instruction to be selected into multiple composites, provided
each composite supplies a compatible message and handles the declared effect. The `Wallet` owns
the balance invariant; the composite owns how the message is obtained and where the effect goes.

## `component!`: declaring a component's instruction set

The arithmetic library shows the shape that a future `component!` macro is meant to make concise.
`ArithmeticUnit` is a reusable component, and its instruction set consists of `add`, `sub`, and
`mul`. The source currently writes the important pieces out by hand. Its commented `isa!` sketch
shows the intended declaration:

```rust
isa! {
    #[namespace("arith")]
    instruction Arithmetic {
        #[pattern = "'add"]
        Add,
        #[pattern = "'sub"]
        Sub,
        #[pattern = "'mul"]
        Mul,
    }
}
```

A component-oriented macro can use that instruction set as part of a declaration such as:

```text
component! {
    ArithmeticUnit {
        instructions: Arithmetic,
    }
}
```

The macro's useful expansion is not one `Execute<Arithmetic>` implementation. It turns each
instruction-set member into an individual instruction struct and preserves an enum of those
structs for grouping, parsing, storage, or dispatch:

```rust
struct Add;
struct Sub;
struct Mul;

enum Arithmetic {
    Add(Add),
    Sub(Sub),
    Mul(Mul),
}
```

The enum is the instruction *sum*: it answers “which arithmetic operation is this value?” The
structs are the individual instruction *products*: each one can be used as the `I` in
`Execute<I>`:

```rust
impl Execute<Add> for ArithmeticUnit {
    type Message = BinaryOperands;
    type Effect = ValueResult;
    type Fault = ArithmeticFault;

    fn execute(
        &mut self,
        _instruction: &Add,
        message: BinaryOperands,
    ) -> Result<StepResult<ValueResult>, ArithmeticFault> {
        Ok(StepResult {
            effects: Effects::one(ValueResult(message.lhs + message.rhs)),
            execution: Execution::Complete,
        })
    }
}
```

`Sub` and `Mul` can have their own `Execute<Sub>` and `Execute<Mul>` implementations. They may
share `BinaryOperands` and `ValueResult`, as the arithmetic component does, or declare different
message, effect, and fault types when their semantics require it.

This split is necessary because a single enum implementation would force execution through a
large match and one broad set of associated types. Individual structs allow the type system to
record that `Add` needs `BinaryOperands`, produces `ValueResult`, and has a particular fault
model. A composite can select only `Add` without also exposing `Sub` and `Mul`, while a parser or
runtime instruction sum can still carry all three variants when it needs a single storable value.

The component macro therefore has two related jobs:

1. Declare or consume the component's instruction set and generate the individual instruction
   products plus their grouped enum.
2. Generate the repetitive component boundary and dispatch plumbing while leaving the actual
   `Execute<I>` behavior to the component author.

The component owns reusable instruction behavior. A composite later decides which individual
instructions are admitted, which component instance receives each one, where messages come from,
and where effects go. This keeps instruction semantics reusable without making every component
automatically expose every operation in every machine.

## `Supply<M>`: resolving a runtime message

`Supply<M>` is a capability for obtaining a message of type `M` from component state:

```rust
trait Supply<M> {
    type Fault;

    fn supply(&mut self) -> Result<M, Self::Fault>;
}
```

For the wallet example, a route might supply an amount from a register component:

```rust
struct Register(u64);

impl Supply<CreditAmount> for Register {
    type Fault = RegisterFault;

    fn supply(&mut self) -> Result<CreditAmount, RegisterFault> {
        Ok(CreditAmount(self.0))
    }
}
```

The capability keeps message resolution outside `Execute`. `Wallet` does not need to know
whether its amount came from a register, a decoded constant, a stack, or a network adapter. A
different machine can reuse `AddCredit` with a different `Supply<CreditAmount>` implementation.

For `NoMessage`, no supplier is needed: the framework can construct `NoMessage` directly.

## `Absorb<E>`: a reusable effect destination

`Absorb<E>` describes a component that can consume an effect:

```rust
trait Absorb<E> {
    type Fault;

    fn absorb(&mut self, effect: E) -> Result<(), Self::Fault>;
}
```

A history component can absorb wallet changes without knowing how the wallet produced them:

```rust
struct AuditLog(Vec<String>);

impl Absorb<CreditChanged> for AuditLog {
    type Fault = std::convert::Infallible;

    fn absorb(&mut self, CreditChanged(balance): CreditChanged) -> Result<(), Self::Fault> {
        self.0.push(format!("balance is now {balance}"));
        Ok(())
    }
}
```

This enables reuse and composition. The same `CreditChanged` can be handled by a balance display,
an audit log, or a quota checker, each with its own state and fault type. `Absorb` is intentionally
machine-agnostic: it says what a component can consume, not which instruction route selected it.

The component author implements `Absorb<E>` as part of the component's reusable behavior. The
composite author, or generated composite code, supplies the route-specific `Handle<E, R>` wiring
that decides when and where the capability is used. This is why `Absorb` does not need to know the
route that produced the effect.

## `Observe<E, R>`: non-consuming instrumentation

`Observe<Effect, Route>` receives a borrowed effect before the semantic handler consumes it:

```rust
trait Observe<Effect, Route> {
    type Error;

    fn observe(&mut self, effect: &Effect) -> Result<(), Self::Error>;
}
```

The route parameter matters because the same effect type may be produced by several routes. A
simple observer can count events without taking ownership:

```rust
struct CreditRoute;
struct Metrics { credit_events: usize }

impl Observe<CreditChanged, CreditRoute> for Metrics {
    type Error = std::convert::Infallible;

    fn observe(&mut self, _effect: &CreditChanged) -> Result<(), Self::Error> {
        self.credit_events += 1;
        Ok(())
    }
}
```

Observation is separate from handling for two reasons. First, logging and metrics should not
become the semantic owner of an effect. Second, multiple observers can borrow the same effect in a
deterministic order before one handler consumes it. Enabling an observer should add visibility,
not change the destination or ownership of the effect.

## `Handle<E, R>`: route-specific effect handling

`Handle<Effect, Route>` consumes an effect for one statically identified route:

```rust
trait Handle<Effect, Route> {
    type Error;

    fn handle(&mut self, effect: Effect) -> Result<(), Self::Error>;
}
```

The route parameter prevents ambiguous handling when one composite selects the same effect or
instruction more than once. For example, a machine could route `MessageSent` from two different
ports to one transport type while keeping their destinations distinct:

```rust
struct LeftPort;
struct RightPort;
struct Transport;
struct MessageSent(Vec<u8>);

impl Handle<MessageSent, LeftPort> for Transport {
    type Error = TransportFault;

    fn handle(&mut self, effect: MessageSent) -> Result<(), TransportFault> {
        self.send_from_left(effect.0)
    }
}

impl Handle<MessageSent, RightPort> for Transport {
    type Error = TransportFault;

    fn handle(&mut self, effect: MessageSent) -> Result<(), TransportFault> {
        self.send_from_right(effect.0)
    }
}
```

Without the route marker, the two implementations would collide because Rust sees the same
`Transport` target and `MessageSent` effect. More importantly, the generated composite would lose
the identity needed to route each operation correctly.

In the usual case, `Handle` is the route-aware adapter and `Absorb` is the reusable destination
capability. The generated or hand-written `Handle` implementation commonly delegates directly to
`Absorb`:

```rust
impl Handle<CreditChanged, CreditRoute> for AuditLog {
    type Error = <Self as Absorb<CreditChanged>>::Fault;

    fn handle(&mut self, effect: CreditChanged) -> Result<(), Self::Error> {
        self.absorb(effect)
    }
}
```

This preserves both roles: `Absorb<CreditChanged>` says that `AuditLog` can consume this effect in
any suitable context, while `Handle<CreditChanged, CreditRoute>` says that this particular machine
route sends its effect to that destination. `Handle` can instead contain route-specific behavior
when the default delegation is not sufficient.

## `Route`: static identity for one selected path

`Route` is a marker trait with associated `Effect` and `Error` types:

```rust
trait Route {
    type Effect;
    type Error;
}
```

A route is not a runtime event and not a program-counter state. It is the compile-time identity of
one path through a composite. A generated composite might create markers like these:

```rust
struct ReadConfig;
struct ReadSecret;

impl Route for ReadConfig {
    type Effect = ConfigRead;
    type Error = MachineFault;
}

impl Route for ReadSecret {
    type Effect = SecretRead;
    type Error = MachineFault;
}
```

Route identity allows generation to associate each path with its own message supplier, component,
effect observers, handler, timing policy, and diagnostics. It also makes it possible to route the
same instruction type to two component instances without merging their wiring.

The associated `Effect` lets generated code name the route once and derive the effect type from
it. The associated `Error` is the containing machine's normalized error boundary: lower-level
component, supplier, observer, and handler faults can be converted into it at the route boundary.

## `Resume<C>`: completing a parked operation

`Resume<C>` handles a completion for an operation that previously returned `Parked`:

```rust
trait Resume<C> {
    type Effect;
    type Fault;

    fn resume(&mut self, completion: C) -> Result<StepResult<Self::Effect>, Self::Fault>;
}
```

The completion must be owned. It cannot contain a borrow into the parent or into a temporary
message because the parent may process it much later.

For a door controller:

```rust
struct OpenDoor;
struct DoorOpened;
struct OpenCompletion { request_id: u64 };

impl Resume<OpenCompletion> for DoorController {
    type Effect = DoorOpened;
    type Fault = DoorFault;

    fn resume(
        &mut self,
        completion: OpenCompletion,
    ) -> Result<StepResult<DoorOpened>, DoorFault> {
        self.finish_request(completion.request_id)?;
        Ok(StepResult {
            effects: Effects::one(DoorOpened),
            execution: Execution::Complete,
        })
    }
}
```

The parent stores or schedules `OpenCompletion`; it does not need to understand the controller's
internal state. When resumed, the controller can emit ordinary effects and use the same handling
pipeline as a newly started instruction.

## `machine_macro.rs`: planned effect fanout

This file is currently a design note, not an implementation. It sketches a future `machine!`
surface for declaring effect fanout:

```text
effects {
    observe metrics, trace;
    to audit_log;
}
```

The intended expansion is:

```text
for each effect:
    metrics.observe(&effect)
    trace.observe(&effect)
    audit_log.handle(effect)
```

The observers borrow the effect, so both can inspect it. The handler receives ownership exactly
once. `to audit_log;` selects the default behavior: the generated `Handle` implementation routes
the effect to `audit_log`, normally by calling its `Absorb` implementation.

When the machine needs custom effect-handling behavior, the destination can eventually be
overridden with `with record_credit;`:

```text
effects {
    observe metrics, trace;
    with record_credit;
}
```

`with record_credit;` names a handler function supplied by the machine author. The generated route
uses that function instead of the default `Handle`/`Absorb` path. For example, the machine author
could provide:

```rust
fn record_credit(
    machine: &mut AccountMachine,
    effect: CreditChanged,
) -> Result<(), AccountFault> {
    machine.audit.push(effect.0);
    Ok(())
}
```

This syntax is necessary because effect routing is repetitive but semantically important:
the generated code must preserve observer order, handler ownership, route identity, and error
conversion.

The macro should generate wiring, not invent behavior. The author still defines the component's
`Execute` implementation, the observer logic, and the handler logic. The declaration merely makes
the selected connections visible and checks that the types fit.

## How the concepts fit together

Here is a complete small route for `AddCredit`:

```text
Register::supply
    -> CreditAmount
    -> Wallet::execute(AddCredit, CreditAmount)
    -> StepResult<CreditChanged>
    -> Metrics::observe(&CreditChanged)
    -> AuditLog::handle(CreditChanged)
    -> Execution::Complete
```

The same route with a waiting instruction has a different control state but the same effect
pipeline:

```text
Mailbox::execute(ReadNext, NoMessage)
    -> StepResult<ReaderParked>
    -> Execution::Parked
    -> later ReadCompletion
    -> Mailbox::resume(ReadCompletion)
    -> StepResult<ItemRead>
    -> observers and handler
    -> Execution::Complete
```

Together, these contracts provide the useful separation:

- `Supply` determines where runtime input comes from.
- `Execute` owns the instruction's local state transition.
- `Effects` communicates consequences without exposing component internals.
- `Observe` adds non-owning diagnostics and instrumentation.
- `Absorb` provides the reusable effect-consuming capability, while `Handle` normally delegates to
  it and adds route identity; a `machine!` `with handler;` clause can eventually override that
  default.
- `Execution` tells the parent whether instruction-stream progress is allowed.
- `Resume` gives suspension a typed, owned completion path.
- `Route` keeps repeated or identical-looking paths distinct.
- `NoMessage` makes the absence of runtime input explicit.
- The planned macro makes the wiring concise while preserving those boundaries.

That separation is what lets a single instruction behavior be reused in different machines, lets a
component retain ownership of its invariants, and lets a parent composite coordinate dataflow and
suspension without reaching into child-private state.
