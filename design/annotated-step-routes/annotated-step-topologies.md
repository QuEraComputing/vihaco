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
[Foundational effect handling](foundational-work.md#composite-and-receiving-field-effect-handling)

The route API has ten nodes for instruction preparation, execution,
and effect handling.

| Stage | Route constructor | Hook |
| --- | --- | --- |
| Prepare | `prepare_message_at::<F, Message>()` | `ReusableMessageRecipe` on the message type |
| Prepare async | `async_prepare_message_at::<F, Message>()` | `AsyncReusableMessageRecipe` on the message type |
| Build | `build_message_at::<F, Message>()` | `BuildMessage` on the composite |
| Build async | `async_build_message_at::<F>()` | `AsyncBuildMessage` on the composite |
| Execute | `execute_instruction_at::<F>()` | `Execute` on the destination field |
| Execute async | `execute_async_instruction_at::<F>()` | `AsyncExecute` on the destination field |
| Deliver effect | `apply_effect_at::<F>()` | `Apply` on the receiving field |
| Deliver effect async | `async_apply_effect_at::<F>()` | `AsyncApply` on the receiving field |
| Handle effect with composite policy | `handle_effect_at::<Destination>()` | `HandleEffect` on the composite |
| Handle effect with async composite policy | `async_handle_effect_at::<Destination>()` | `AsyncHandleEffect` on the composite |

Each message node yields a prepared instruction and message. The execution
node checks that its destination accepts that message, so preparation and
execution can use different modes. Choose one preparation node per
instruction and one effect handler per handoff. A composite hook can
delegate to the receiving field within its implementation.

The builder resolves the source of a composite handler at `finish()`.
`Execute`, `AsyncExecute`, `Apply`, and `AsyncApply` attribute their output
to the field they call. Transparent nodes such as `await_work()` and
`duplicate()` preserve the source. The prototype requires a unique field
source for composite handlers. A join result or an effect emitted by a
composite hook has no single field source. Topology inspection displays
the inferred source and the selected destination.

The callback interpreter supplies unit context to these hooks. The
borrowed interpreter supplies program context to synchronous preparation
and uses unit context for composite effect handling.
[Borrowing across suspension](foundational-work.md#borrowing-across-suspension)

`duplicate()` builds a fork that clones the incoming payload once.
`await_work()` waits on a handle returned by a preceding node.
[Field-targeted operations](../../crates/vihaco-runtime/examples/static_graph_routes/framework/operations.rs) · [Async message preparation](../../crates/vihaco-runtime/examples/static_graph_routes/framework/async_execution.rs)

## Using the same machine with different backends

The machine author declares the dialect, device fields, preparation
and effect handlers, and any routes. A capture step can start the same
route with the backend supplied through its `StepContext`. The logical
program and route stay the same as long as the backend supplies the
required work and effect types.

The runtime author places each section on its machine, queues entries,
and chooses how to coordinate independent instruction streams and
device events. That includes virtual time and tie-breaks for an
emulator, or worker assignment and completion routing for a threaded
host. Compare machine outcomes when switching backends. Add timing
and ordering assertions wherever the workload requires them.
[Canonical runtime structure](route-drivers.md#canonical-runtime-structure)

## Foundational work

The runtime changes that can land ahead of the route API are tracked in
[Foundational work](foundational-work.md).

## Runnable examples

| Example | What to read for |
| --- | --- |
| [Synchronous CPU](../../crates/vihaco-stdlib/examples/static_execution_state/machine.rs) | Borrow the current instruction and module to prepare a message. Run borrowed routes inline through the checked program driver and apply CPU effects before the next fetch. |
| [Waveform fan-out](../../crates/vihaco-runtime/examples/static_graph_routes/machines/waveform/mod.rs) | Inspect the annotated DAC transfer. Follow the clamped and original samples down separate paths and join after both deliveries. |
| [Device completion](../../crates/vihaco-runtime/examples/static_graph_routes/machines/device_completion.rs) | Wait on a handle from synchronous `Execute`. Then use `async_apply_effect_at` to validate and store the sample at the receiving field. |
| [Async digitizer](../../crates/vihaco-runtime/examples/static_graph_routes/machines/async_capture/mod.rs) | Prepare the receiver, then let the component's `AsyncExecute` impl await the sample. Return the leased component before applying the result at that field. |
| [Warehouse SST](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/warehouse.sst) | Print a label and verify it through the same route on simulation and Tokio. Simulation delays delivery by 12 ticks, and Tokio uses a wall-clock timer. |

Run the route-only examples with `mise run static-graph-routes`. Run the
CPU and Warehouse programs plus the debugger, channel, and Tokio
scenarios with `mise run program-drivers`. The CPU program is also
available on its own through `mise run static-execution-state`.

## Declaring and inspecting a route

Warehouse's print route has three nodes. The execution annotation
sets the earliest tick at which simulation can deliver a successful result
to the parcel records.

```rust,ignore
step_graph::<ops::PrintLabel>()
    .begin(build_message_at::<PrinterField, NoMessage>())
    .then(execute_instruction_at::<PrinterField>().annotate::<MinExecution<12>>())
    .then(handle_effect_at::<ParcelField>())
    .finish()
```

On the main path, `then` connects one node's output to the next
node's input. `then_via` also attaches edge metadata. On a fork builder
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

The zero-sized `Route<I, P>` supports inspection and two execution paths.
`Describe` projects the nodes, field markers, directed edges, and
metadata. Repeated uses of the same operation get distinct occurrence
indices within the projection.
`Run` checks that each operation accepts its predecessor's output and that
the driver supplies the capabilities it needs. The synchronous
interpreter, `RunSync`, runs its supported nodes and sequences inline with
borrowed program context.
[Framework guide and Warehouse walkthrough](framework-guide.md)

Applications attach typed static annotations to nodes and edges. Inspection
can look up a value by tag or by its value type. Annotated operation nodes
pass their metadata to the driver when they run. Edge annotations are
available for inspection. An execution hook for edges, resource occupancy,
and stable occurrence IDs for source mappings remain open.

## From route to invocation

`Route::start_with` supplies the machine, driver, instruction, and a
one-shot terminal callback. A wait owns the values needed to resume.
The driver delivers readiness with a fresh machine borrow. A join releases
its result after both arms succeed. If one arm fails, the terminal callback
reports the error once. Any already-started work can still complete, and
prior mutations and effects are retained.

The prototype framework lives under `examples/`. The public submission
and readiness contracts let downstream runtimes supply their own
scheduling backends. A checked program driver uses `HasProgram` to
fetch and dispatch and resumes a parked route when its result is ready.
The backend can serve more than one machine.
[Task submission and completion](route-drivers.md#task-submission-and-completion)
