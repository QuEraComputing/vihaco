# Annotated step topologies

An instruction step can prepare a message, execute one instruction, and
deliver its result through a sequence or a fork/join. A **route** declares
these operations and their payload dependencies. A route can be inspected
before a machine exists, then reused across invocations with fresh inputs
and completion handles.

## Where behavior lives

Routes describe where payloads go and when execution waits. A field-targeted
node calls the selected component's `Execute`, `AsyncExecute`, `Receive`,
or `Apply` implementation. `Receive<E>` can return a new typed effect for
the next node or `()` for final delivery. For example, the waveform clamp
receives the source samples and returns the clamped samples that travel over
the annotated DAC edge.

Message preparation uses `BuildMessage` for synchronous instructions and
`BuildAsyncMessage` for async instructions. Both identify the field whose
message they prepare. `build_async_message_at` runs before the async
execution node leases the destination component.

The route API omits `Compute`. Payload plumbing such as `duplicate()`
operates on the payload alone. State changes go through the destination
field's contract and appear as such in the inspected topology.
[Field-targeted operations](../../crates/vihaco-runtime/examples/static_graph_routes/framework/operations.rs) · [Async message preparation](../../crates/vihaco-runtime/examples/static_graph_routes/framework/async_execution.rs)

## Runnable examples

| Example | What to read for |
| --- | --- |
| [Synchronous CPU](../../crates/vihaco-stdlib/examples/static_execution_state/machine.rs) | Borrow the current instruction and module to prepare a message. Run borrowed routes inline through the checked program driver and apply CPU effects before the next fetch. |
| [Waveform fan-out](../../crates/vihaco-runtime/examples/static_graph_routes/machines/waveform/mod.rs) | Inspect the annotated DAC transfer. Follow the clamped and original samples down separate paths and join after both deliveries. |
| [Device completion](../../crates/vihaco-runtime/examples/static_graph_routes/machines/device_completion.rs) | Start an acquisition with synchronous `Execute`. Park at a separate node on the handle the instruction returned. |
| [Async digitizer](../../crates/vihaco-runtime/examples/static_graph_routes/machines/async_capture/mod.rs) | Let the component's own `async fn execute` wait for a sample. Return the leased component before delivering the result back to that field. |
| [Warehouse bytecode](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/mod.rs) | Print a label and then verify it. Simulation delays the label's delivery by 12 ticks. The same bytecode completes inline with the channel driver. |

Run the route-only examples with `mise run static-graph-routes`. Run the
CPU and Warehouse programs plus the debugger, channel, and Tokio
scenarios with `mise run program-drivers`. The CPU program is also
available on its own through `mise run static-execution-state`.

## Declaring and inspecting a route

The warehouse's print instruction has three nodes. The execution annotation
sets the earliest tick at which simulation can deliver a successful result
to the parcel records.

```rust,ignore
step_graph::<ops::PrintLabel>()
    .begin(build_message_at::<PrinterField>())
    .then_execute(instruction_at::<PrinterField>().annotate::<MinExecution<12>>())
    .then(receive_effect_at::<ParcelField>())
    .finish()
```

`then` connects the output of one path to the input of the next.
`then_via` attaches metadata to that connection. A fork takes a split
such as `duplicate()`, which clones a payload once and moves the original
to the other arm. The join produces a pair of the arms' outputs.
A path can be written inline or returned by an
ordinary Rust function. The builder requires exactly one designated
instruction node, selected with `then_execute` or `then_execute_async`.

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

The prototype framework lives under `examples/`. The RFC specifies
`Await`, `FutureDriver`, and `AnnotatedDriver` as public extension traits
that downstream crates can implement. Program examples put a checked
program driver above the route backend. That driver uses
`HasProgram` to read the machine's `ExecutionState`, fetches and
dispatches instructions, and stops fetching while a route is pending.
The companion design explains the driver API, retirement, and debugger
commands. [Route drivers](route-drivers.md)
