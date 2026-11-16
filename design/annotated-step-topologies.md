# Annotated Step Topologies

Status: initial design proposal. A runnable prototype lives in
[`annotated_route`](../crates/vihaco-runtime/examples/annotated_route.rs).
The routing APIs remain local to that example; they are not library APIs yet.

## Purpose

Introduce a declarative route builder for the execution behavior supplied by
`Step<F, I, Context>`. A route describes where a message is built, where an
instruction executes, and where its effects propagate. Its structure must be
statically inspectable, with explicit destinations and operation kinds.

Individual stages carry arbitrary annotations. The context passed to `Step`
interprets those annotations and determines how work proceeds. A scheduler can
interpret a delivery latency as a request to apply an effect at `now + latency`;
another context can execute the same topology immediately.

The earlier `../vihaco-static-top-experiment` explored typed route builders and
owned continuations. This proposal adapts that direction to the current `Step`,
`Field`, `BuildMessage`, and `Execute` interfaces.

## Step boundary

A step has three phases:

```text
Build the instruction's message
    -> Execute the instruction
    -> Propagate the resulting effects
```

Each declared step contains exactly one instruction execution node. Failures or
deferred work can prevent that node from running, but the topology cannot declare
a second execution. `Execute` refers exclusively to instruction execution.

Effect delivery uses a separate receiving contract. Receiving an effect does
not implicitly invoke another instruction. If a delivery leads to subsequent
instruction execution, that execution belongs to another step. How such steps
are initiated remains outside the initial prototype.

The builder encodes these phases in its types. Only the prepared-message phase
offers `.execute()`. Execution configuration can transition into propagation;
propagation builders have no `.execute()` method.

## Authoring a topology

The proposed fixed-latency route has this shape:

```rust,ignore
route_through::<field!(Machine::source)>()
    .build_message::<commands::Fire>()
    .execute()
    .annotate::<FixedLatencyNs<3>>()
    .fan_out((
        deliver_to::<field!(Machine::dac)>()
            .annotate::<FixedLatencyNs<7>>(),
        deliver_to::<field!(Machine::monitor)>()
            .annotate_with(TraceLabel("monitor delivery")),
    ))
    .start(self, context, instruction)
```

Construction produces a typed description; `.start(...)` initiates its
interpretation. The selected source field identifies the component and its
`Execute<I>` implementation. `BuildMessage` determines that execution's input.
Delivery fields identify receivers of the resulting effect.

`route_through::<F>()` selects the instruction destination directly, replacing
the separate `route::<Machine>().at::<F>()` selection. The concrete machine type
is inferred from `self` at `.start(...)`, where its field and execution bounds
are checked. Existing field markers identify declaration fields and can be
shared by generic instantiations; the builder must not assume that it can
recover a concrete parent type from the marker alone.

`deliver_to::<F>()` constructs a propagation branch directly. Simple deliveries
do not require a separate `branch()` constructor. Delivery builders have no
instruction-execution method, preserving the single-execution step boundary.

The runnable prototype still uses `route::<Machine>().at::<F>()` and
`branch().deliver_to::<F>()`; adopting the constructors above is pending.
It also uses explicit `DacField` and `MonitorField` implementations because the
current composite macro generates field routes for instruction devices, while
these receivers have no instruction inventory. Generated field markers for
effect-only receivers are needed for the full spelling shown above.

### Message preparation and execution

The example models a waveform controller. `Fire` carries an output channel and
an amplitude multiplier. Its `BuildMessage` implementation resolves waveform
samples and calibration from machine state and creates an owned `PulseMessage`.
Source execution computes each output sample as:

```text
sample * instruction_amplitude * calibration_gain + calibration_offset
```

The result is an owned waveform effect. The DAC receiver validates the output
range and records the waveform; the monitor receiver records peak amplitude and
sample count. Their receiving behavior is distinct from instruction execution.

Message preparation currently happens when the step is submitted. It snapshots
the selected samples and calibration, even if execution is deferred. The
prototype calls `BuildMessage` with `()` as its preparation context. Forwarding
additional preparation context and annotating preparation itself require further
design; they are not implemented by the current builder.

### Effect propagation

The source and instruction are fixed by the route prefix. Each propagation
branch declares a destination field in the topology, rather than hiding that
destination inside an arbitrary callback. The field's child must implement the
receiving contract for the effect type:

```rust,ignore
trait Receive<E> {
    fn receive(&mut self, effect: E) -> eyre::Result<()>;
}
```

The prototype fans out one effect into two branches, cloning the effect for the
first branch and moving it into the second. This ownership policy and the
two-branch arity are prototype choices. Routing multiple effects, selecting
effect variants, transformations, and additional branching forms remain open.

## Static topology and inspection

The route must expose enough structure for a reader or tool to identify:

- The instruction destination and instruction type.
- Its message-preparation stage and associated message type.
- Its single instruction execution.
- Propagation connections, branch structure, and destination fields.
- Annotation types and whether values are static or supplied at runtime.

Component implementations still define computation and message resolution.
Static topology describes declared propagation boundaries; it does not reveal
every read or mutation inside those implementations. Future transformations must
preserve explicit source and destination information rather than concealing
connections in opaque operations.

The prototype's inspection method walks concrete route types and produces text
without building a message or executing a handler. It uses field source names
and Rust type names. This demonstrates structural inspection, not a stable
export format or a compile-time analysis engine. Nested-route identity must
retain the complete typed field path; `Field::SOURCE` alone names only the final
field and its aliases.

A runtime annotation changes attached data, not the topology's connections.
An inspector can identify a dynamically valued latency without knowing its value
until a particular route instance is constructed.

## Annotation model

Annotations are application-defined types. Latency, tracing, resource requests,
and priority are possible meanings; the routing core does not prescribe their
interpretation.

The builder distinguishes static annotations from runtime values:

```rust,ignore
.annotate::<FixedLatencyNs<7>>()
.annotate_with(DynamicLatency(delay))
```

The static form retains `A` through a type marker such as
`PhantomData<fn() -> A>`. It constructs no `A` value and requires no `Default`
implementation. The runtime form owns its annotation value.

```rust,ignore
struct FixedLatencyNs<const NS: u64>;
struct DynamicLatency(Duration);
```

Mixed metadata is also possible: a runtime annotation's type can identify a
resource statically while its fields carry a runtime quantity. Static and
runtime annotations can appear on different stages of the same route.

An annotation applies to the immediately preceding stage. Execution latency
decorates instruction execution; delivery latency decorates that delivery.
Neither implicitly decorates descendant stages. The prototype permits one
annotation per supported stage. Accumulation, composition, and interactions
between annotations are unresolved.

Keeping annotation information in types enables trait-based validation and
specialized interpretation. Exact compile-time timing analysis would need
additional traversal and analysis machinery. Even a fixed delay produces a
runtime deadline when added to the current modeled time.

## Framework and user responsibilities

The intended ordinary user interface is the machine declaration, its component
behavior, and the route expression inside each authored `Step`. Users select
an execution context and annotations; they do not implement the builder or
continuation machinery to use routes.

The following is the proposed library boundary. Existing `Field`, `BuildMessage`,
`Execute`, and `Step` interfaces already come from vihaco. New routing and
receiving interfaces are currently example-local and would become library APIs
as this design is implemented.

| Concern | Provided by vihaco | Written or selected by the machine author |
| --- | --- | --- |
| Machine structure | Declaration macros, field-marker generation, and `Field` access machinery | Components, composite fields, and their state |
| Instruction execution | `Execute<I>` contract and routing to the selected field | Instruction definitions and component `Execute<I>` implementations, or existing components |
| Message preparation | `BuildMessage` contract, existing automatic preparation for supported message types, and the route node that invokes it | Application message types and custom resolvers when automatic preparation does not apply |
| Step definition | `Step` contract and builder operations enforcing the phase order | A `Step` implementation that declares its topology and starts it with the supplied context |
| Effect propagation | Delivery and fan-out nodes, field access, payload compatibility bounds, and a receiving contract | Receiver behavior and explicit destinations; application-specific transformations when needed |
| Static annotations | Type-position attachment and dispatch to the context without constructing annotation values | Selection of annotation types and constants; custom annotation definitions when needed |
| Runtime annotations | Value attachment and ownership through interpretation | Values computed from instructions, configuration, or execution input |
| Deferred work | Standard execution/delivery work types and their resumption implementations | Selection of an existing interpreter; custom boundary data only for specialized integrations |
| Inspection | Structural traversal of the declared topology | Calls to inspection APIs and any application-specific presentation |
| Interpretation | Context extension contracts and an immediate interpreter | Selection of a provided context or implementation of an application-specific interpreter |

### Ordinary machine authors

For the waveform example, the application supplies `Fire`, `PulseMessage`, the
source's `Execute` implementation, custom `BuildMessage` logic, and the DAC and
monitor's receiving behavior. Its `Step` contains the route shown above. A
consumer reusing those components would only compose them and declare the routes
and policies appropriate to its machine.

Users should not need to name or implement `ExecutionWork`, `DeliveryWork`,
`Resume`, `Policy`, annotation wrappers, or builder state types for this path.
Those types preserve the topology and ownership guarantees behind the public
builder. Generic route signatures may expose associated bounds, but authoring a
concrete route should not require manually spelling its nested topology type.

The prototype's explicit `DacField` and `MonitorField` implementations are
temporary integration scaffolding, not intended routine user work. Generated
field access for effect-only receivers should eliminate that boilerplate.

### Annotation and interpreter authors

Adding a new annotation meaning requires defining its type and arranging for
the chosen context to interpret it. A machine author using an already supported
annotation supplies its type parameter or runtime value; they do not write a
context implementation for every route or machine. For example, one generic
`StaticContext<M, FixedLatencyNs<NS>>` implementation handles all fixed values
and compatible machines.

An interpreter author implements the submission contracts, scheduling or
recording policy, and any required queue or external-service integration. The
framework constructs ordinary owned continuations; the interpreter decides when
to resume them. Only integrations requiring different ownership boundaries or
consumer environments should need custom continuation representations.

The immediate interpreter is intended to ship with the routing machinery. The
example's latency types, tracing annotation, and scheduler demonstrate reusable
policies; their eventual home in a standard-library crate or an optional runtime
package is still open. They should not be copied into every machine. The initial
proposal does not assume that vihaco's core includes a scheduler for every
application or hardware environment.

## Context interpretation

The prototype separates runtime-valued and type-only interpretation:

```rust,ignore
trait Context<M, A>: Sized {
    fn submit<W>(
        &self,
        machine: &mut M,
        annotation: A,
        work: W,
    ) -> eyre::Result<()>
    where
        W: Resume<M, Self> + 'static;
}

trait StaticContext<M, A>: Sized {
    fn submit_static<W>(
        &self,
        machine: &mut M,
        work: W,
    ) -> eyre::Result<()>
    where
        W: Resume<M, Self> + 'static;
}
```

The scheduler implements `StaticContext<M, FixedLatencyNs<NS>>` for every
supported const value. It obtains the delay directly from `NS`. Its runtime
latency implementation reads the submitted value. Its tracing implementation
records a label at submission and queues work with zero delay.

The immediate interpreter explicitly ignores both forms of annotation and
resumes work synchronously. Unannotated stages also run synchronously in the
prototype. A production API must decide whether contexts should control those
stages too.

Required context implementations appear in the step's trait bounds. Unsupported
annotations therefore fail type checking at route execution. Ignoring metadata
is an explicit interpreter policy.

`Step` continues to borrow its context through `&Context`. The example scheduler
uses `Cell` and `RefCell` to maintain its queue through that reference. These are
single-threaded implementation choices, not requirements for all interpreters.

## Ownership and deferred execution

Execution borrows its environment. At a deferred boundary, the interpreter
receives an owned description of how to continue, not a closure retaining the
step's machine borrow.

```text
Borrow machine and context
    -> prepare input and interpret the route
    -> construct owned work at a deferred boundary
    -> submit work and release borrows

Later, acquire an execution environment
    -> resume work
    -> propagate results through the remaining topology
```

Before instruction execution, owned work carries execution input and the
remaining propagation topology. After execution, a deferred delivery carries
its effect and destination. Effects do not have to exist when their downstream
topology is declared.

The prototype clones the borrowed instruction and owns the prepared message.
That is one implementation of the boundary. Other machines could retain an
owning program reference and instruction position, or construct device-specific
commands understood by another worker. The general requirement is an owned
continuation with sufficient information for its consumer to resume it.

The current continuation contract reacquires the same machine/context types:

```rust,ignore
trait Resume<M, C: ?Sized> {
    fn resume(self, machine: &mut M, context: &C) -> eyre::Result<()>;
}
```

Supporting consumers with different environments or worker ownership models
requires further design. The example queue requires `'static` work but not
`Send`; multithreaded scheduling may impose stronger bounds at its boundaries.

## Scheduler semantics and failures

The example interprets latency as delay before applying an operation. It does
not model separate start and completion events or reserve a resource for the
duration of the delay.

```text
t = 0 ns   submit source execution with fixed 3 ns latency
t = 3 ns   execute source; produce waveform
           submit DAC delivery with fixed 7 ns latency
           submit monitor delivery with zero added delay
t = 3 ns   monitor receives waveform
t = 10 ns  DAC receives waveform
```

Each delivery delay starts when that delivery is submitted after effect
production. The scheduler orders equal deadlines by submission sequence. It
erases concrete continuation types only at its heterogeneous queue.

Successful `Step::step` means the route's initial synchronous work and submission
succeeded. It does not guarantee deferred completion. Preparation failures are
returned immediately; deferred execution and delivery failures surface when the
scheduler resumes work. Existing state changes and successful submissions are
not rolled back. Fan-out can partially succeed before a later branch fails.
Cancellation, retries, and broader recovery policy remain open.

## Prototype and validation

The example is split by responsibility:

| File | Responsibility |
| --- | --- |
| [framework.rs](../crates/vihaco-runtime/examples/annotated_route/framework.rs) | Proposed builder, typed nodes, context contracts, continuation machinery, and inspection |
| [contexts.rs](../crates/vihaco-runtime/examples/annotated_route/contexts.rs) | Example latency and tracing policies, and scheduler |
| [machine.rs](../crates/vihaco-runtime/examples/annotated_route/machine.rs) | Machine, instructions, message builder, source execution, receivers, and authored steps |
| [annotated_route.rs](../crates/vihaco-runtime/examples/annotated_route.rs) | Executable scenarios and assertions |

Run all three timing variants:

```sh
cargo run -p vihaco-runtime --example annotated_route --features typed-declarations
cargo run -p vihaco-runtime --example annotated_route --features typed-declarations -- 2 4
```

The arguments supply runtime execution and delivery delays in nanoseconds.
For the second invocation:

| Route | Source execution | DAC delivery |
| --- | --- | --- |
| Fixed 3 ns execution and 7 ns delivery | 3 ns | 10 ns |
| Runtime 2 ns execution and 4 ns delivery | 2 ns | 6 ns |
| Fixed 3 ns execution and runtime 4 ns delivery | 3 ns | 7 ns |

Assertions cover calibrated waveform values, one source execution per successful
step, delivery timing, instruction ownership across suspension, topology
inspection, preparation failure, and deferred DAC failure. The default and
custom-delay runs, formatting, and targeted Clippy checks passed for the current
prototype.

## Integration questions

Before promoting this API into vihaco, resolve:

1. Adopt `route_through::<F>()` with machine inference at `.start(...)` and
   direct `deliver_to::<F>()` branches. Provide generated field routes for
   effect-only receivers and determine whether `Receive` should integrate with
   existing effect-sink interfaces.
2. Annotation support for message preparation, multiple annotations on a stage,
   and interaction between execution policies.
3. General propagation of zero, one, or many effects; effect selection;
   transformations; fan-out arity; and ownership or sharing across branches.
4. Message snapshot timing and the relationship between preparation context and
   the step's execution context.
5. Owned execution references beyond instruction cloning, and continuation
   consumers with different environments or threading requirements.
6. Structured topology inspection/export, full nested-field identities, and
   static validation or timing analysis.
7. Completion, cancellation, ordering, and failure behavior across scheduled
   work and subsequent steps.

The central constraints remain: explicit static topology, a single instruction
execution per step, generic static and runtime annotations, and owned work at
deferred boundaries.
