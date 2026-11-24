# Annotated step topologies

An instruction step can prepare a message, execute one instruction, and
deliver its result through a sequence or a fork/join. A **route** declares
these operations and their payload dependencies. A route can be inspected
before a machine exists, then reused across invocations with fresh inputs
and completion handles.

## Runnable examples

| Example | What to read for |
| --- | --- |
| [Synchronous CPU](../../crates/vihaco-stdlib/examples/static_execution_state/machine.rs) | Borrow the current instruction and module to prepare a message. Apply the CPU effect to the operand stack before the next fetch. |
| [Waveform fan-out](../../crates/vihaco-runtime/examples/static_graph_routes/machines/waveform/mod.rs) | Inspect the annotated DAC transfer. Follow the clamped and original samples down separate paths and join after both deliveries. |
| [Device completion](../../crates/vihaco-runtime/examples/static_graph_routes/machines/device_completion.rs) | Start an acquisition with synchronous `Execute`. Park at a separate node on the handle the instruction returned. |
| [Async digitizer](../../crates/vihaco-runtime/examples/static_graph_routes/machines/async_capture/mod.rs) | Let the component's own `async fn execute` wait for a sample. Return the leased component before delivering the result back to that field. |
| [Warehouse bytecode](../../crates/vihaco-runtime/examples/static_graph_routes/machines/warehouse/mod.rs) | Print a label and then verify it. Simulation delays the label's delivery by 12 ticks. The same bytecode completes inline with the channel driver. |

Run the examples with `mise run static-graph-routes` and
`mise run static-execution-state`.

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
`then_via` attaches metadata to that connection. A fork takes an explicit
split operation that supplies owned inputs to its two arms, then produces
the pair of their outputs. A path can be written inline or returned by an
ordinary Rust function. The builder requires exactly one designated
instruction node, selected with `then_execute` or `then_execute_async`.

The zero-sized `Route<I, P>` supports inspection and two execution paths.
`Describe` projects the nodes, field markers, directed edges, and metadata. Repeated uses of the
same operation get distinct occurrence indices within the projection.
`Run` checks that each operation accepts its predecessor's output and that
the driver supplies the capabilities it needs. The synchronous
interpretation, `RunSync`, returns directly and can borrow program context.
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

The example framework lives under `examples/`. The companion plan describes
how the prototype's continuation model would connect to scheduling,
retirement, and debugger commands. [Route drivers](route-drivers.md)
