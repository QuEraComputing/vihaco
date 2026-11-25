# Annotated step topologies

An instruction step can prepare a message, execute one instruction, and
deliver its result through a sequence or a fork/join. A **route** declares
these operations and their payload dependencies. A route can be inspected
before a machine exists, then reused across invocations with fresh inputs
and completion handles.

The canonical program runtime keeps machine ownership, driver
bookkeeping, and task scheduling separate. Warehouse and the portable
digitizer use the same implementation of that split.
[Canonical runtime structure](route-drivers.md#canonical-runtime-structure)

## Where behavior lives

Routes describe where payloads go and when execution waits. A field-targeted
node calls the selected component's `Execute`, `AsyncExecute`, or `Apply`
implementation. `Apply<E>` can return a typed effect for
the next node or `()` for final delivery. For example, the waveform clamp
receives source samples and returns the clamped samples that travel over
the annotated DAC edge. For composite-level policy, `HandleEffect` and
`AsyncHandleEffect` select a hook by source field, destination field,
and incoming effect type.
[Component and composite authoring hooks](foundational-work.md#component-and-composite-authoring-hooks)

Component authors implement reusable message recipes on their message
types and `Apply` on their receiving components. Composite authors use
`BuildMessage` for machine-specific input and `HandleEffect` for
machine-specific delivery. The async counterparts follow the same
authoring boundary.

The route API has ten nodes for instruction preparation, execution,
and effect handling.

| Stage | Route constructor | Hook |
| --- | --- | --- |
| Prepare | `prepare_message_at::<F, Message>()` | `ReusableMessageRecipe` on the component author's message type |
| Prepare async | `async_prepare_message_at::<F, Message>()` | `AsyncReusableMessageRecipe` on the component author's message type |
| Build | `build_message_at::<F, Message>()` | `BuildMessage` on the composite |
| Build async | `async_build_message_at::<F>()` | `AsyncBuildMessage` on the composite |
| Execute | `execute_instruction_at::<F>()` | `Execute` on the destination field |
| Execute async | `execute_async_instruction_at::<F>()` | `AsyncExecute` on the destination field |
| Deliver effect | `apply_effect_at::<F>()` | `Apply` on the receiving component |
| Deliver effect async | `async_apply_effect_at::<F>()` | `AsyncApply` on the receiving component |
| Handle effect with composite policy | `handle_effect_at::<Destination>()` | `HandleEffect` on the composite |
| Handle effect with async composite policy | `async_handle_effect_at::<Destination>()` | `AsyncHandleEffect` on the composite |

Each message node yields a prepared instruction and message. The execution
node checks that its destination accepts that message, so preparation and
execution can use different modes. Choose one preparation node per
instruction and one effect handler per handoff. A composite hook can
delegate to the receiving field within its implementation.

The builder resolves the source of a composite handler at `finish()`.
`Execute`, `AsyncExecute`, `Apply`, and `AsyncApply` attribute their output
to the field they call. `HandleEffect` and `AsyncHandleEffect` attribute
their output to the destination field. For example, a handoff from `A`
to `B` followed by `handle_effect_at::<C>()` selects the hook from `B`
to `C`. `await_work()` preserves the source of its input. Each split
branch inherits the source of the tuple being distributed. A join
combines several producers and requires explicit
source attribution before its result can enter a source-sensitive hook.
Topology inspection displays the resolved source and selected destination.
The prototype still needs destination attribution for composite hook
outputs and an API for explicit attribution after a join.

In a loaded program, an initial or resumed stage can borrow the current
run's image and the runtime services its hook requires. Pending work owns
the inputs it needs across suspension. The callback prototype supplies
unit context today, and the borrowed prototype supplies program context
to synchronous preparation.
[Borrowing across suspension](foundational-work.md#borrowing-across-suspension)

`duplicate()` builds a fork that clones the incoming payload once.
`await_work()` waits on a handle returned by a preceding node.
[Field-targeted operations](../../crates/vihaco-runtime/examples/static_graph_routes/framework/operations.rs) · [Async message preparation](../../crates/vihaco-runtime/examples/static_graph_routes/framework/async_execution.rs)

## Using the same machine with different backends

The machine author declares the dialect, device fields, preparation
and effect handlers, and any routes. The route describes the operations,
their typed inputs and outputs, and their dependencies. `await_work()`
expresses that the next stage needs the result of a work handle. An async
node selects a hook whose result may become available later. The driver
and backend decide whether a result can be delivered in the current turn,
when to park and resume, and where eligible work runs. The logical
machine and program can stay the same when the backend changes.

The runtime author places each section on its machine, queues entries,
and chooses how to coordinate independent instruction streams and
device events. That includes virtual time and tie-breaks for an
emulator, or worker assignment and completion routing for a threaded
host. Compare machine outcomes when switching backends. Add timing
and ordering assertions wherever the workload requires them.
[Canonical runtime structure](route-drivers.md#canonical-runtime-structure)

### Routes describe dependencies

A route answers what must happen and which values each operation needs.
The host configures timing for the driver and backend it uses. A handoff
from the printer to the parcel records has the same meaning in an
immediate runner, a virtual-time simulation, and a wall-clock runner.
Those hosts can give the handoff different release policies while using
the same machine, route, and instruction image.

Use `.named("print")` to identify a node and
`edge().named("label_to_records")` to identify an individual handoff.
Names describe roles in the route and give host configuration and
inspection a shared vocabulary. Typed annotations can describe logical
properties such as the bus a transfer uses. The host supplies that bus's
timing model and any release delays. A delay on `label_to_records` applies
to that dependency even if the printer has other outgoing handoffs.
[Backend delivery policy](route-drivers.md#delivery-policy-at-the-backend)

### Using Sim as a machine author

- Name the handoff whose delivery the simulation needs to control. In
  Warehouse, `label_to_records` names the edge carrying the completed
  label. The `print` node name appears in inspection. This example binds
  delays to edges. Keep names used by host policy unique within the
  route, and update that policy when renaming a site.
- Use annotations for logical properties of the operation or handoff.
  For example, `DacBus` identifies a shared bus so a host can choose a
  model for it. Warehouse needs only an edge name for its per-edge delay.
  A machine author who also supplies the simulation host sets the delay
  in the `Sim` configuration, after registering the route's topology.
- `delay_edge` in this example takes virtual ticks from the moment the
  payload reaches the edge. The printer issues at tick 5, and the records
  receive the owned label at tick 17 with a 12-tick delay. A synchronous
  `Execute` and `Apply` work for this route because the driver can park
  between them. Choose an async hook when the operation itself can
  suspend, or `await_work()` when a preceding operation hands off a
  work handle whose result the next stage needs.
- Put ordering requirements in the route. Warehouse retires the print
  instruction after the records apply the label, so the following
  instruction can rely on it under immediate and timed delivery. Assert
  exact ticks in the simulation test that supplies the timing policy.

[Warehouse machine](../../crates/vihaco-stdlib/examples/logical_routes/machine.rs) · [Sim host and timing test](../../crates/vihaco-stdlib/examples/logical_routes.rs)

## Foundational work

The runtime changes that can land ahead of the route API are tracked in
[Foundational work](foundational-work.md). This includes automatic
[composite field projections](foundational-work.md#composite-field-projections)
that respect Rust field visibility and leave `#[device]` responsible for
bytecode lifting.

## Runnable examples

| Example | What to read for |
| --- | --- |
| [Synchronous CPU](../../crates/vihaco-stdlib/examples/static_execution_state/machine.rs) | Borrow the current instruction and module to prepare a message. Run borrowed routes inline through the checked program driver and apply CPU effects before the next fetch. |
| [Waveform fan-out](../../crates/vihaco-runtime/examples/static_graph_routes/machines/waveform/mod.rs) | Inspect the annotated DAC transfer. Follow the clamped and original samples down separate paths and join after both deliveries. |
| [Device completion](../../crates/vihaco-runtime/examples/static_graph_routes/machines/device_completion.rs) | Wait on a handle from synchronous `Execute`. Then use `async_apply_effect_at` to validate and store the sample at the receiving field. |
| [Async digitizer](../../crates/vihaco-runtime/examples/static_graph_routes/machines/async_capture/mod.rs) | Prepare the receiver, then let the component's `AsyncExecute` impl await the sample. Return the leased component before applying the result at that field. |
| [Warehouse SST](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/warehouse.sst) | Follow annotated print delivery and verification on simulation and Tokio. |
| [Logical Warehouse routes](../../crates/vihaco-stdlib/examples/logical_routes.rs) | Start the same print and async scan routes with borrowed instructions. Compare immediate delivery, delayed delivery configured by the simulator, and Tokio. |

Run the route-only examples with `mise run static-graph-routes`. Run the
CPU and Warehouse programs plus the debugger, channel, and Tokio
scenarios with `mise run program-drivers`. The CPU program is also
available on its own through `mise run static-execution-state`.
Run the logical-route example with `cargo run -p vihaco-stdlib
--features program-image-execution --example logical_routes`.

## Declaring and inspecting a route

Warehouse's logical print route has three nodes. The parcel-records
handler depends on the label produced by the printer.

```rust,ignore
step_graph::<ops::PrintLabel>()
    .begin(prepare_message_at::<PrinterField, NoMessage>())
    .then(execute_instruction_at::<PrinterField>().named("print"))
    .then_via(
        edge().named("label_to_records"),
        apply_effect_at::<ParcelField>(),
    )
    .finish()
```

Its `Step` calls `context.start(self, &print_route().finish(), instruction)` with
the borrowed instruction. The same route can deliver the label in the
issue turn or let a simulator release it 12 ticks later. The delay is
part of the simulator's policy configuration. The earlier Warehouse
walkthrough uses a timing annotation and explicitly selects its interpreter.
[Logical print route](../../crates/vihaco-stdlib/examples/logical_routes/machine.rs) · [Warehouse walkthrough](framework-guide.md#warehouse-label-printing-walkthrough) · [Backend delivery policy](route-drivers.md#delivery-policy-at-the-backend)

On the main path, `then` connects one node's output to the next
node's input. `then_via` also attaches edge metadata. The callback
interpreter can defer a named edge's owned payload. On a fork builder
started by `duplicate()` or `split_with(...)`, each `then` adds a branch.
Any `then` inside that branch still connects its own nodes in sequence.

The waveform example uses `duplicate()` to route a copy to the clamp
and DAC, and to move the original to the monitor.

```rust,ignore
.then(fork_join(
    duplicate()
        .then(apply_effect_at::<ClampField>()
            .then_via(edge().annotate::<DacBus>(), apply_effect_at::<Dac>()))
        .then(apply_effect_at::<Monitor>()),
))
.then(apply_effect_at::<DeliveryField>())
```

`split_with` takes a node that produces a tuple and moves one element
into each branch. Use `fork_join` when a later node needs the results.
Its output tuple follows branch declaration order. Directly appending
the fork gives terminal delivery paths without a downstream join node.

```rust,ignore
// SplitterField returns (Samples, CaptureStats, Trigger).
.then(
    split_with(apply_effect_at::<SplitterField>())
        .then(async_apply_effect_at::<ArchiveField>())
        .then(apply_effect_at::<TelemetryField>())
        .then(handle_effect_at::<ControlField>()),
)
.finish()
```

The prototype generates tuple support for two through eight branches.
The interpreter checks tuple arity and element types against the branch
list. Each terminal delivery branch returns `()`, and the instruction
completes once all deliveries succeed or one fails. The inspected
topology shows the individual paths without a synthetic join.
[Waveform example](../../crates/vihaco-runtime/examples/static_graph_routes/machines/waveform/mod.rs)

A path can be written inline or returned by an ordinary Rust function.
Use `execute_instruction_at` or `execute_async_instruction_at` to declare
the execution node. The same `then` connects preparation, execution,
and effect handling. At compile time, `finish()` requires exactly one
execution node in the route, outside any fork/join path.

The zero-sized `Route<I, P>` supports inspection and execution.
`Describe` projects the nodes, field markers, directed edges, and
metadata. Repeated uses of the same operation get distinct occurrence
indices within the projection.
Execution checks that each operation accepts its predecessor's output and
that the driver supplies the capabilities it needs. The proposed entry
point starts every route with a borrowed instruction and program image.
When a stage needs to wait, its adapter retains the values it needs across
suspension. The `logical_routes` example uses `finish()` for both the
print and scan routes and exercises that borrowed entry point with
immediate simulation, timed simulation, and Tokio. The earlier program
driver examples use `finish_inline()` and `finish_parkable()`.
[Framework guide and Warehouse walkthrough](framework-guide.md)

Inspection can look up an annotation by tag or by its value type. The
registered topology exposes names on nodes and edges. The named edge
delivery hook is prototyped in `logical_routes`. Node-specific timing,
resource occupancy, and occurrence IDs stable across route edits remain
open.

## From route to invocation

`Route::start_with` supplies the machine, driver, instruction, and a
one-shot terminal callback. A wait owns the values needed to resume.
The driver delivers readiness with a fresh machine borrow. A join releases
its result after all arms succeed. If one arm fails, the terminal callback
reports the error once and prior mutations and effects are retained.
Already-started sibling work can still finish. Its late completion is
consumed for cleanup without applying route effects.

The prototype framework lives under `examples/`. The public submission
and readiness contracts let downstream runtimes supply their own
scheduling backends. A checked program driver uses `HasProgram` to
fetch and dispatch and resumes a parked route when its result is ready.
The backend can serve more than one machine.
[Task submission and completion](route-drivers.md#task-submission-and-completion)
