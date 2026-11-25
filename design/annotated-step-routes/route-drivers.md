# Drivers for annotated step routes

A program driver selects a machine, fetches using its program counter,
and dispatches one instruction at a time. When an instruction's route
parks, the driver keeps that instruction in flight and resumes it when
the backend can deliver its result. The route describes the operations
and their payload dependencies. [Annotated step topologies](annotated-step-topologies.md)

## Program state stays in the machine

A composite marks its `ExecutionState` field with `#[program]`. The
checked driver reaches it through `HasProgram`. `ExecutionState` owns
the PC, call frames, locals, operands, and run state. The program image
supplies the instruction vector indexed by the PC. `Loadable` pairs the
machine and image. The checked driver owns that pair, the backend, and
the program loop. It borrows the image and mutates the machine through
`Loadable::parts_mut`.
[HasProgram](../../crates/vihaco-runtime/src/execution.rs) · [ExecutionState](../../crates/vihaco-stdlib/src/execution_state.rs) · [Loadable](../../crates/vihaco-stdlib/src/loadable.rs)

The PC is an inline field. Branches store the selected target directly,
calls save the already-advanced PC in the frame, and returns restore the
frame's return address. Keeping those accesses local also lets an
execution state be initialized or cloned without borrowing its driver.
The driver saves a continuation PC when a particular instruction parks.
[Control flow](../../crates/vihaco-stdlib/src/execution_state/execution.rs)

Checked fetch requires a runnable invocation with an active frame. It
checks the instruction index and the sequential PC increment before
dispatch. The guarded driver also checks the selected work ID before
resuming a parked route. These checks happen at runtime. Bytecode
verification before execution is a separate layer.
[Checked fetch](../../crates/vihaco-stdlib/src/execution_state/runner.rs#L129) · [Checked delivery](../../crates/vihaco-stdlib/examples/program_drivers/checked.rs)

## Program driver and route backend

`ProgramDriver` is the host interface. The checked example implements
it once and composes it with different route backends. A single-machine
driver accepts machine ID `()`. The Tokio wrapper uses machine IDs to
send commands to the thread that owns each composite.
[ProgramDriver](../../crates/vihaco-stdlib/src/program_driver.rs) · [Checked driver](../../crates/vihaco-stdlib/examples/program_drivers/checked.rs)

| Host method | What it authorizes |
| --- | --- |
| `start(machine, arguments)` | Enters the loaded program at its entry point and creates the first frame. |
| `run(machine, command)` | Fetches and dispatches under a `Continue` or `SingleStep` command. |
| `deliver(machine, work, delivery)` | Advances the selected pending instruction under its saved command. |

For backends that can advance on their own, the checked example also
offers `execute(arguments)`. It enters the program, runs until a stop,
and drives pending work until halt. Warehouse uses this entry point.
If it encounters a breakpoint, it returns an error at that PC so a
caller can choose the next debugger command.
The explicit `run` and `deliver` methods support debugger commands and
host-supplied samples.

The driver reports a typed stop. `Pending` includes the originating PC
and work ID. `Retired` reports the instruction completed by a
single-step. `Breakpoint` and `Halted` identify their stop positions.
Errors travel through `eyre::Result`. This keeps a normal wait distinct
from a fault and avoids the private-error downcast described in #204.
[Checked execution and debugger suspension](https://github.com/QuEraComputing/vihaco/issues/204)

`Step` and generated `Dispatch` keep returning `eyre::Result<()>`.
Dispatch selects one step and forwards its result. The program driver
reports pending work and retirement at the host boundary after it
inspects the machine and the route's completion. Existing inline steps
keep the same return type and error handling.
[Step](../../crates/vihaco-runtime/src/step.rs) · [Dispatch](../../crates/vihaco-runtime/src/dispatch.rs)

### Public route driver API

`Await`, `FutureDriver`, and `AnnotatedDriver` will be part of the public
framework API. Downstream crates can implement the capabilities their
routes use on their own driver types. The framework will also expose the
route entry points and supporting types needed to use these traits.

| Trait | What the driver provides |
| --- | --- |
| `Await<M, H>` | Accepts a typed work handle from `await_work()` and delivers its result to a one-shot continuation. |
| `FutureDriver<M>` | Drives a Rust future, such as async execution that holds a component lease, and delivers its result. |
| `AnnotatedDriver<M>` | Interprets route annotations, for example by delaying delivery until a simulated tick. |

A parked continuation owns its captures and receives a fresh mutable
machine borrow when it runs. The current implementations of these contracts
live in the RFC examples. [Await and FutureDriver prototype](../../crates/vihaco-runtime/examples/static_graph_routes/framework/runtime.rs) · [AnnotatedDriver prototype](../../crates/vihaco-runtime/examples/static_graph_routes/framework/graph/execution.rs)

When a work handle resolves, its output continues along the route to the
selected field's effect handler. In the sample route, `Await` produces a
typed `PushToStack` effect and `apply_effect_at::<ProgramField>()` calls
`Apply` on the program field. The route completes after that field receives
the result. [Sample route](../../crates/vihaco-stdlib/examples/program_drivers/machine.rs)

## Checked issue, resumption, and retirement

1. **Issue.** Checked fetch remembers the originating PC and advances
   to the sequential continuation before dispatch. The fetch helper
   borrows instructions from the module and takes a separate dispatch
   context. The prototype context carries the module and a route backend,
   so an authored `Step` can simply return
   `context.start(self, &sample_route(), ops::Sample(instruction.0))`.
2. **Park.** An inline route returns its result as the `Step` result.
   If a route is still pending, the context saves the continuation PC
   and completion slot, then suspends `ExecutionState` before returning
   `Ok(())`. Checked fetch restores the originating PC. The driver returns
   `Pending` and retains the run command with the pending route.
3. **Resume.** The driver validates the work ID and restores the saved
   continuation PC under the reentrancy guard. The backend resumes the
   existing route. If it parks again, the driver returns a fresh work ID
   at the original PC and preserves the command.
4. **Retire.** The route completes after delivering all results required
   by the next instruction. The driver counts one retirement at the
   originating PC. For `SingleStep`, it returns before the next fetch.
   For `Continue`, it can fetch again under the saved command.
5. **Fault.** A deferred route error leaves the visible PC at the
   originating instruction and retains effects already applied.

The module-only `execute_instruction` entry point wraps the same checked
fetch helper. Existing `Loadable` callers and the image-patching debugger
keep using that API. The prototype driver owns one in-flight route per
machine. Its debugger checks breakpoints before fetch and preserves
single-step across two waits. Integrating async suspension into the
image-patching debugger is follow-up work.
[Checked fetch](../../crates/vihaco-stdlib/src/execution_state/runner.rs) · [Existing debugger](../../crates/vihaco-stdlib/src/debugger/runner.rs) · [Sample Step](../../crates/vihaco-stdlib/examples/program_drivers/machine.rs)

## Runnable examples

Run the five scenarios with `mise run program-drivers`.

| Runner | Behavior |
| --- | --- |
| [Inline CPU](../../crates/vihaco-stdlib/examples/static_execution_state/machine.rs) | Uses the checked driver and borrowed synchronous routes. A branch skips an instruction and the CPU leaves its arithmetic result on the stack before the next fetch. |
| [Debugger](../../crates/vihaco-stdlib/examples/program_drivers/scenarios.rs) | Preserves a single-step across two waits, retires the sample instruction once, and stops independently at the next breakpoint. |
| [Channel](../../crates/vihaco-stdlib/examples/program_drivers/scenarios.rs) | Delivers an external sample to a parked route. A stale work ID is rejected and the sensor issues once. |
| [Simulation and Warehouse](framework-guide.md#warehouse-label-printing-walkthrough) | Runs real bytecode through one `execute` call. The print issues at tick 5 and its label arrives at tick 17. Verification fetches after delivery. The same routes complete inline with the channel backend. |
| [Tokio threads](../../crates/vihaco-stdlib/examples/program_drivers/tokio_threads.rs) | Owns a composite and a Tokio `LocalSet` on each worker thread. A rendezvous shows both workers driving scheduled futures concurrently. Commands and results cross threads while a machine and its local continuations stay on their owner thread. |

The waveform, device completion, and async digitizer demos remain small
examples of route execution in isolation. They use the same route
capabilities without a loaded program. Run them with
`mise run static-graph-routes`. [Route-only examples](annotated-step-topologies.md#runnable-examples)

## Open integration work

- **Owner scheduling.** A machine owner may need to match several
  pending waits against incoming events. The
  prototype identifies the current work for a machine. A scheduler
  also needs the stream or invocation identity, a typed wait reason,
  and the earliest eligible time when available.
- **Drain gates.** A barrier may retire before the next instruction is
  eligible to fetch. Clearing the drain gate must add no retirement.
  This differs from a pending receive, which retires after delivery.
- **Reset and cancellation.** Invalidate pending work on reset, image
  replacement, and cancellation. A late event must never resume a
  replacement invocation. Validate this with a deferred fault after
  a visible effect and a late completion after reset.
- **Integration boundaries.** Generate coverage for sync and async
  execution modes and define how deferred routes can retain any program
  context they still need. The prototype prepares owned messages before
  entering async execution. Message preparation that itself suspends
  remains open.
  Edge latency and arbitration across streams also remain open.

[Issue #204](https://github.com/QuEraComputing/vihaco/issues/204)
