# Static route framework

A route declares the operations and payload dependencies of one instruction
step. Its zero-sized Rust type can be inspected before the machine exists.
Each invocation supplies the instruction and machine state. The execution
path can finish inline or hand a continuation to a driver.

Preparation, execution, and effect delivery identify their destination
fields. A receiver can return a new effect to the graph, as the waveform
clamp does. In the sample program, all backends turn the completed sample
into a typed `PushToStack` effect and the program field applies it before
the next fetch. [Waveform handlers](../../crates/vihaco-runtime/examples/static_graph_routes/machines/waveform/mod.rs) · [Sample route](../../crates/vihaco-stdlib/examples/program_drivers/machine.rs)

This walkthrough follows the checked Warehouse example. Its simulator
stores a delivery callback in the event queue. The proposed shared
scheduler interface uses a typed ticket and leaves the continuation
with the program driver. The one-call `execute` entry point can wrap
either arrangement. [Task submission and completion](route-drivers.md#task-submission-and-completion)

## Warehouse label printing walkthrough

The program prints a barcode for parcel 42, then looks up that parcel to
verify the label was recorded. The printer accepts the submission at tick 5.
The parcel records receive the label at tick 17, and only then does the
program dispatch verification. Run the walkthrough with
`mise run program-drivers`. [Warehouse example](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/mod.rs)

The caller loads the SST image once and pairs it with a fresh Warehouse for
each run. A host call loads, schedules, and executes the program to halt
on the selected backend.

```rust,ignore
let image = program::load()?;
let mut simulated = Runtime::new(Sim::starting_at(5));
let simulated_id = simulated.execute(new_warehouse(image.clone()), vec![])?;

let mut realtime = Runtime::new(TokioWorker::new()?);
let realtime_id = realtime.execute(new_warehouse(image), vec![])?;
```

The runtime owns the loaded machines. Its program driver borrows a
machine for one entry, issue, or resume turn. The pending-state tests
advance one turn at a time to show what the driver leaves for the
scheduler. Warehouse and the digitizer share this structure.
[Canonical runtime structure](route-drivers.md#canonical-runtime-structure)

### 1. Declare the print route

```rust,ignore
step_graph::<ops::PrintLabel>()
    .begin(build_message_at::<PrinterField, NoMessage>())
    .then(execute_instruction_at::<PrinterField>().annotate::<MinExecution<12>>())
    .then(handle_effect_at::<ParcelField>())
    .finish()
```

`PrinterField` and `ParcelField` identify fields of the same `Warehouse`
machine. The printer implements `Execute<ops::PrintLabel>`. The parcel
records implement `Apply<LabelPrinted>` and
`Execute<records::VerifyLabel>`. The last print node calls Warehouse's
`HandleEffect<PrinterField, ParcelField, LabelPrinted>`. The route
infers the printer as the source. This composite impl delegates delivery
to the records' `Apply`. Verification is a later instruction with its own route.
[Warehouse components and routes](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/mod.rs)

| Print route node | Input | Output |
| --- | --- | --- |
| Prepare at `PrinterField` | `PrintLabel(42)` | `Prepared { instruction: PrintLabel(42), message: NoMessage }` |
| Execute at `PrinterField` | The prepared instruction and message | `LabelPrinted { parcel: 42, barcode: "BX-0042" }` |
| Handle from `PrinterField` to `ParcelField` | The owned `LabelPrinted` effect | `()` after storing the barcode |

`begin` installs the preparation node. Each `then` connects the next
stage. `execute_instruction_at` marks the printer node as the execution
site and carries its timing annotation. `finish()` checks that the route
contains exactly one execution site and produces a `Route<I, P>`.
`I` is `ops::PrintLabel`, and `P` is the nested sequence of node and edge
types. The execution bounds check that each node accepts its predecessor's
payload. [Builder](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/builder.rs) · [Operations](../../crates/vihaco-runtime/examples/static_graph_routes/framework/operations.rs)

### 2. Inspect the route before creating the machine

The demo constructs the route, checks that it occupies zero bytes, and
prints the topology. `Describe` walks the path types, reading operation
labels, field markers, and annotation types. It produces this output.

```text
Warehouse / Print Label
  0  build message printer []
  1  execute instruction printer ["MinExecution<12>"]
  2  handle composite effect printer → parcels []
  0 → 1  []
  1 → 2  []
```

Nodes 0 and 1 have the same field marker and separate occurrence indices.
The timing annotation belongs to node 1. The two edges describe payload
dependencies and have empty annotation lists. During execution, the
route driver reads timing metadata through the annotated node's hook.
[Topology projection](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/inspection.rs)

### 3. Load the SST section and dispatch PC 0

The `warehouse` section declares `@main` as its entry and uses the
qualified dialect instructions for printing, verifying, and halting.
The shared loader uses `vihaco-sst` to check that section against
Warehouse's generated syntax and lower it into an image. The runtime
stores the image and machine together as a `Loadable`. It queues an entry
turn for the machine. When the simulator selects that turn, the driver
enters `@main` at PC 0, and the runtime queues the first issue.
[Warehouse SST](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/warehouse.sst) · [Shared SST loader](../../crates/vihaco-stdlib/examples/program_drivers/sst.rs) · [Runtime loop](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/runtime.rs)

On the issue turn, the runtime lends the `Loadable` to the driver.
Checked fetch reads the PC through `HasProgram` on Warehouse's
`#[program]` field. It fetches the instruction and advances the PC to 1
before calling generated dispatch. The selected `Step` passes its route
and owned print instruction to `context.start`. The step context also
carries the borrowed program module.
[Warehouse step](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/mod.rs) · [Program driver](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/driver.rs)

### 4. Issue the print at tick 5

The route prepares `NoMessage` inline and passes the instruction and
message to its annotated execution node. The driver reads
`MinExecution<12>` and asks the simulator for the deadline, tick 17.
The printer executes immediately, increments `submitted`, and produces
the owned `LabelPrinted` effect. The driver submits it for timed delivery
and records the resulting ticket and the route continuation. The
simulator owns the timer and the value it will release.
[Route execution](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/execution.rs) · [Shared task interface](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/tasks.rs)

The driver saves continuation PC 1 and parks the stream. Checked fetch
restores the visible originating PC of 0. The machine borrow ends when
the issue turn returns. The barcode is still absent from the records,
so the next fetch for this stream stays suspended.

### 5. Deliver at tick 17, then schedule verification

The simulator advances to tick 17, makes the typed result available, and
reports the task ID. The runtime uses that ID to find the waiting machine
and lends it to the driver for a resume turn. The driver restores PC 1,
takes the result from the ticket, and runs the saved continuation.
`handle_effect_at::<ParcelField>()` calls the composite hook with the
inferred printer source. The hook stores the label through
`ParcelRecords::apply`, and the print route completes.
[Simulator](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/backends/sim.rs) · [Driver resumption](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/driver.rs)

The resume turn returns with the machine runnable at PC 1. The runtime
queues an issue turn for verification. That next turn sees the saved
barcode and records parcel 42 as verified. The following turn executes
`Halt` at PC 2.

| Turn | Tick | PC after turn | Observable state |
| --- | ---: | ---: | --- |
| Enter `@main` | 5 | 0 | First issue queued |
| Issue print | 5 | 0 | One submission, no stored label |
| Resume the print | 17 | 1 | Barcode stored, verification queued |
| Issue verification | 17 | 2 | Parcel 42 verified |
| Issue halt | 17 | 2 | Program halted |

### 6. Swap the backend

The Tokio run uses the same SST image, generated dispatch, and annotated
route. Tokio maps each timing tick to one millisecond. The printer still
issues synchronously, and a local timer holds the successful result until
its deadline. The Tokio worker reports a ready task ID, and the shared
driver resumes the continuation with a fresh machine borrow. Both runs
record `BX-0042`, verify parcel 42, submit once, and halt at PC 2.
The topology and the virtual clock's tick 17 are checked separately.
[Backend comparison](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/mod.rs) · [Tokio route backend](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/backends/threaded.rs)

If receipt rejects a duplicate label, the route reports the error through
the same slot. The deferred failure SST section prints twice and checks that
the second print faults at PC 1. The first stored label and both print
submissions remain, and verification never runs. A separate test runs
verification first and checks that its missing-label error leaves the PC
at 0. The program driver tracks one in-flight instruction per stream.
[Failure test](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/mod.rs) · [Route drivers](route-drivers.md)

## Async capture on two backends

The portable digitizer declares its capture dialect with `#[component]`
and implements `AsyncExecute<Capture>`. The generated dispatch forwards
the capture instruction to its `Step`. That step starts the same route
with either scheduler. The route runs three nodes.

1. `async_build_message_at` takes the input channel handle from the
   composite and submits an owned preparation future. This stage can
   wait to receive the sample receiver while the digitizer stays available.
2. `execute_async_instruction_at` takes the digitizer out of its slot and moves
   the component lease, instruction, and receiver into an owned future.
   The async method borrows only those owned values when it waits for
   the sample. The runtime owns the machine and the driver keeps the
   parked route.
3. The lease returns the digitizer before the continuation borrows the
   composite again. `async_handle_effect_at::<DigitizerField>()` selects the machine's
   policy for a sample going back to the digitizer. That hook validates
   the sample, applies it to the digitizer, and returns a ready future
   with the outcome. Once the route completes, the runtime schedules
   the next fetch.

The simulation backend chooses between runnable turns and timed input.
The Tokio backend polls the same futures on the worker that owns the
machine. The driver's ticket and continuation connect those backend
events back to the correct capture. The route and effect handler stay
the same when the host switches backends.
[Digitizer route](../../crates/vihaco-stdlib/examples/program_drivers/portable/digitizer.rs) · [Program driver and scheduler](route-drivers.md#portable-digitizers)

## Where to read next

| Part | Responsibility |
| --- | --- |
| [`graph/builder.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/builder.rs) | Path composition and the single-instruction builder state |
| [`graph/inspection.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/inspection.rs) and [`graph/annotations.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/annotations.rs) | Node occurrences, dependency edges, and typed annotation lookup |
| [`graph/execution.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/execution.rs) | Inline or parked continuations, sequence errors, and the join |
| [`operations.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/operations.rs) | Typed payloads, field-targeted effect delivery, and pure duplication for a fork |
| [`runtime.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/runtime.rs) | Driver contracts, channel delivery, and local future polling |
| [`async_execution.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/async_execution.rs) | Field-targeted message preparation and exclusive component leases during async execution |
| [`synchronous.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/synchronous.rs) | Inline execution with a borrowed instruction and program context |

The device-completion example parks after the instruction returns a
handle. Once the sample arrives, `async_apply_effect_at` calls the
digitizer's `AsyncApply`. The handler waits on a validation receiver and
stores the accepted sample through an owned handle. The async digitizer parks **inside** the instruction's future
while it owns a component lease. The synchronous CPU runs to completion
with borrowed inputs. Waveform builds its copy and original branches with
`duplicate().then(...).then(...)`. The join supplies both results to the
delivery log, and the DAC transfer keeps its edge metadata. A route
can append a `split_with(...).then(...).then(...)` fork directly when its
branches handle delivery and finish independently.
[Fluent fork syntax](annotated-step-topologies.md#declaring-and-inspecting-a-route)
