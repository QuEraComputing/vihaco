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

The caller supplies a `Loadable` that pairs the machine with its decoded
image, then lets the checked driver run the program to completion.

```rust,ignore
let mut vm = CheckedDriver::new(loaded_warehouse()?, SimDriver::starting_at(5));
vm.execute(&[])?;
```

The walkthrough follows the checked issue and delivery operations
inside `execute`. The pending-state test exercises that lower-level
interface directly so it can inspect the machine between those operations.

### 1. Declare the print route

```rust,ignore
step_graph::<ops::PrintLabel>()
    .begin(build_message_at::<PrinterField>())
    .then_execute(instruction_at::<PrinterField>().annotate::<MinExecution<12>>())
    .then(apply_effect_at::<ParcelField>())
    .finish()
```

`PrinterField` and `ParcelField` identify fields of the same `Warehouse`
machine. The printer implements `Execute<ops::PrintLabel>`. The parcel
records implement `Apply<LabelPrinted>` and, separately,
`Execute<records::VerifyLabel>`. Receipt is the last operation of the
print route. Verification is a later instruction with its own route.
[Warehouse components and routes](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/mod.rs)

| Print route node | Input | Output |
| --- | --- | --- |
| Prepare at `PrinterField` | `PrintLabel(42)` | `Prepared { instruction: PrintLabel(42), message: NoMessage }` |
| Execute at `PrinterField` | The prepared instruction and message | `LabelPrinted { parcel: 42, barcode: "BX-0042" }` |
| Apply at `ParcelField` | The owned `LabelPrinted` effect | `()` after storing the barcode |

`begin` installs the preparation node in a step builder whose state is
`Before`. `then_execute` replaces the field selector with an execution
node, preserves its annotation, and moves the builder to `After`.
`then` appends the receiver. `finish` is available only in `After`, so
the resulting `Route<I, P>` has one designated instruction site.
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
  2  apply effect parcels []
  0 → 1  []
  1 → 2  []
```

Nodes 0 and 1 have the same field marker and separate occurrence indices.
The timing annotation belongs to node 1. The two edges describe payload
dependencies and have empty annotation lists. During execution, the
simulator gets timing metadata through the annotated node's driver hook.
[Topology projection](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/inspection.rs)

### 3. Decode the program and dispatch PC 0

The example encodes `PrintLabel(42)`, `VerifyLabel(42)`, and `Halt`.
The loader decodes the bytecode, converts each instruction to the
generated Warehouse instruction enum, and constructs a module with an
entry function at PC 0. `Loadable::new` pairs that image with the Warehouse
machine. The driver owns the `Loadable` and borrows its image and machine
separately for checked dispatch. The PC comes from the machine's
`#[program]` field, an `ExecutionState<Pause>` exposed through `HasProgram`.
The simulator is configured to start at tick 5. Inside `execute`, the
driver calls `start((), &[])` to enter the module's entry function.
Machine ID `()` selects the single machine in this driver.
[Bytecode conversion](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/program.rs) · [Program driver](../../crates/vihaco-stdlib/examples/program_drivers/checked.rs)

`run((), RunCommand::Continue)` enters the guarded execution loop.
At PC 0, checked fetch reads the instruction and advances the PC to 1.
It passes a `StepContext` containing the module and the route backend to
generated dispatch. The selected `Step` hands an owned print instruction
to `context.start(self, &print_route(), ...)`. The context gives
`Route::start_with` a one-shot terminal callback that owns an `Rc` to the
completion slot. That slot can outlive the borrowed step context.
[Warehouse step](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/mod.rs) · [Completion slot](../../crates/vihaco-stdlib/examples/program_drivers/checked.rs)

An inline completion is returned by `context.start` as the step result.
When the route parks, `context.start` saves its completion slot and
continuation PC, suspends `ExecutionState`, and returns `Ok(())`.
Checked fetch then restores the originating PC. The inner `run` call
reports a typed `Pending` result, which `execute` handles internally.

### 4. Start the operation at tick 5 and park its result

`Route::start_with` calls the path's `Run` implementation. A sequence
runs its left path with a callback that starts the right path on success
or forwards the error. For this route, message preparation runs inline
and calls `BuildMessage` with unit context. It passes the prepared
instruction and `NoMessage` to the annotated execution node.
[Continuation execution](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/execution.rs)

The execution node invokes `SimDriver::run_annotated`. The tag
`MinExecution<12>` supplies the value `ExecutionTicks(12)`. The driver
looks up that value type and computes `ready_at = 5 + 12 = 17`.
It calls the printer's `Execute` immediately. The printer increments
`submitted` and returns the barcode. A successful result before
`ready_at` goes into a `Scheduled<LabelPrinted>` along with the tick.
The simulator's `Await` implementation stores a one-shot callback in its
event queue, ordered by tick and then by insertion order. An error passes
through immediately, as does a success reported at or after `ready_at`.
[Simulation driver](../../crates/vihaco-runtime/examples/static_graph_routes/simulation.rs)

The event owns the effect and the continuation that will invoke the
receiver. The mutable machine borrow ends when dispatch returns. At
that point, the printer has one submission, the parcel records are
empty, and the completion slot is empty. The checked driver returns
`ProgramStop::Pending { pc: 0, work }` with zero retirements and saves the
original `Continue` command. The pending-state test checks that PC 0
is still visible and that verification has yet to run.

### 5. Deliver at tick 17 and fetch PC 1

The `execute` loop calls `deliver((), work, ())`. The checked driver
validates the work ID, restores the saved continuation PC of 1 under
the reentrancy guard, and asks the simulator to deliver. `advance_next`
removes the event for tick 17, sets the simulated clock, and invokes
the stored callback with a fresh mutable borrow of Warehouse.
The continuation starts `apply_effect_at::<ParcelField>()`.
`ParcelRecords::apply` inserts the barcode under parcel 42 and returns
`Ok(())`. This completes the route, so its terminal callback writes
`Some(Ok(()))` into the program driver's slot.
[Scheduled delivery](../../crates/vihaco-runtime/examples/static_graph_routes/simulation.rs) · [Parcel receiver](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/mod.rs)

The checked driver takes the terminal result and counts one retirement
at the saved originating PC of 0. It resumes the saved `Continue`
command and fetches `VerifyLabel(42)` at PC 1. The verify route completes
inline, finds the stored barcode, and records parcel 42 as verified.
The next fetch executes `Halt` at PC 2. The driver returns
`ProgramStop::Halted { pc: 2 }` at tick 17. [Program loop](../../crates/vihaco-stdlib/examples/program_drivers/checked.rs)

| Point in the run | Tick | PC | Observable state |
| --- | ---: | ---: | --- |
| Printer returns the barcode during dispatch | 5 | 1 | One submission, no stored label |
| Checked fetch returns with the route parked | 5 | 0 | No retirement, verification still waiting |
| Driver restores continuation and delivers the label | 17 | 1 | Barcode stored, print completion available |
| Driver retires print and fetches verify | 17 | 1 | Verification sees the barcode |
| Driver executes halt | 17 | 2 | Parcel 42 verified, program halted |

### 6. Compare inline delivery and failure

The demo runs the same decoded bytecode on a fresh machine with
`ChannelDriver`. That driver uses the default annotation hook, which
starts the node and passes its result straight to the continuation.
The parcel records receive the label and the print route's terminal
result is available before dispatch returns. Inside `execute`, the
first `run` call reaches `Halted { pc: 2 }` on the channel backend. Both
backends use the same `StepContext` and checked program loop.
[Channel comparison](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/mod.rs) · [Default annotation hook](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/execution.rs)

If receipt rejects a duplicate label, the route reports the error through
the same slot. The deferred failure test prints twice and checks that
the second print faults at PC 1. The first stored label and both print
submissions remain, and verification never runs. A separate test runs
verification first and checks that its missing-label error leaves the PC
at 0. The checked driver owns one in-flight instruction per machine.
[Failure test](../../crates/vihaco-stdlib/examples/program_drivers/warehouse/mod.rs) · [Route drivers](route-drivers.md)

## Async capture on two backends

The portable digitizer declares its capture dialect with `#[component]`
and implements `AsyncExecute<Capture>`. The generated dispatch forwards
the capture instruction to its `Step`. That step starts the same route
with either scheduler. Read the three nodes in order.

1. `async_build_message_at` takes the input channel handle from the
   composite and submits an owned preparation future. This stage can
   wait to receive the sample receiver while the digitizer stays available.
2. `async_instruction_at` takes the digitizer out of its slot and moves
   the component lease, instruction, and receiver into an owned future.
   The async method borrows only those owned values when it waits for
   the sample. The program driver keeps the machine and parked route.
3. On completion, the lease returns the digitizer before the
   continuation borrows the machine again. `apply_effect_at` calls
   `Apply` on that field and stores the sample. The runtime can then
   schedule the next fetch for this machine.

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

The device-completion example parks **after** the instruction returns a
handle. The async digitizer parks **inside** the instruction's future
while it owns a component lease. The synchronous CPU runs to completion
with borrowed inputs. Waveform uses a split and join and projects
metadata on the DAC transfer.
