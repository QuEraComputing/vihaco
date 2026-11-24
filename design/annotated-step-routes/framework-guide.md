# Static route framework

A route declares the operations and payload dependencies of one instruction
step. Its zero-sized Rust type can be inspected before the machine exists.
Each invocation supplies the instruction and machine state. The execution
path can finish inline or hand a continuation to a driver.

## Warehouse label printing walkthrough

The Warehouse program contains `PrintLabel(42)` followed by `VerifyLabel(42)`.
The example's simulated clock starts at tick 5.
[Warehouse route](../../crates/vihaco-runtime/examples/static_graph_routes/machines/warehouse/mod.rs) · [Program loop](../../crates/vihaco-runtime/examples/static_graph_routes/machines/warehouse/program_driver.rs)

### 1. Declare the route

```rust,ignore
step_graph::<ops::PrintLabel>()
    .begin(build_message_at::<PrinterField>())
    .then_execute(instruction_at::<PrinterField>().annotate::<MinExecution<12>>())
    .then(receive_effect_at::<ParcelField>())
    .finish()
```

`begin` sets the entry. `then_execute` selects the designated instruction
site and changes the builder state so `finish` becomes available. The
resulting path is a sequence of three statically typed nodes.

| Node | Input | Output |
| --- | --- | --- |
| Prepare at `PrinterField` | `PrintLabel(42)` | Instruction and `NoMessage` |
| Execute at `PrinterField` | Prepared instruction and message | `LabelPrinted` |
| Receive at `ParcelField` | `LabelPrinted` | `()` |

### 2. Inspect the topology

`route.topology()` walks the path via `Describe`. It emits three node
occurrences and edges `0 → 1` and `1 → 2`. Nodes 0 and 1 both refer to the
printer field. Node 2 refers to the parcel records field. The timing tag
belongs to node 1. This walk uses each operation's diagnostic label and
annotation types. Starting the program requires a compatible `Run`
implementation. Inspection can happen earlier.

### 3. Dispatch at PC 0 and issue the print

The Warehouse program loader decodes the bytecode into generated machine
instructions. At PC 0, generated dispatch calls the printer's `Step`.
Its `ProgramContext` starts the route with the owned print instruction and
a terminal callback that writes to the current instruction's completion
slot. The route starts with message preparation. The next node passes
its metadata to the simulator, which reads the `ExecutionTicks` value of
`MinExecution<12>` and computes an earliest delivery tick of 17.

The simulator calls the printer's `Execute` at tick 5. The printer records
the submission and returns its `LabelPrinted` effect. The simulator
queues the effect and the continuation to the receiver for tick 17.
The parcel records are still empty. The program loop keeps PC 0 in flight
and asks the simulation driver to advance pending work.

### 4. Deliver the label and advance the program

At tick 17, the simulator removes the queued event and calls its
continuation with a fresh mutable borrow of Warehouse. The receiver
stores the label and returns `()`. The terminal callback writes
`Ok(())` to the completion slot. The program loop consumes that result,
advances to PC 1, and dispatches `VerifyLabel(42)`. Verification sees the
stored label at tick 17. A route error puts the program loop in its fault
state at the current PC.

With the channel backend, the annotation hook takes its default inline
path. The same route writes its terminal result during the initial
dispatch, so the loop can advance without asking the backend for work.
The Warehouse loop is example code. Owner-selected work, retirement
gates, and debugger authorization are covered in the driver design.
[Route drivers](route-drivers.md)

## Where to read next

| Part | Responsibility |
| --- | --- |
| [`graph/builder.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/builder.rs) | Path composition and the single-instruction builder state |
| [`graph/inspection.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/inspection.rs) and [`graph/annotations.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/annotations.rs) | Node occurrences, dependency edges, and typed annotation lookup |
| [`graph/execution.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/execution.rs) | Inline or parked continuations, sequence errors, and the join |
| [`operations.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/operations.rs) | Payloads entering and leaving each kind of node |
| [`runtime.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/runtime.rs) | Driver contracts, channel delivery, and local future polling |
| [`async_execution.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/async_execution.rs) | Exclusive component leases during async execution |
| [`synchronous.rs`](../../crates/vihaco-runtime/examples/static_graph_routes/framework/synchronous.rs) | Inline execution with a borrowed instruction and program context |

The device-completion example parks **after** the instruction returns a
handle. The async digitizer parks **inside** the instruction's future
while it owns a component lease. The synchronous CPU runs to completion
with borrowed inputs. Waveform uses a split and join and projects
metadata on the DAC transfer.
