# Drivers for annotated step routes

A runtime coordinates a set of machines. A program driver enters a
selected program, fetches using its program counter, and dispatches one
instruction at a time. When a route parks, the driver keeps the
instruction in flight. The scheduling backend advances the pending
work and reports when a result can be delivered.
[Annotated step topologies](annotated-step-topologies.md)

## Execution layers

| Layer | What it owns | What it decides |
| --- | --- | --- |
| **Machine** | Devices and live `ExecutionState`, including the PC, call frames, and operand storage. Its `Loadable` pairs it with a program image. | Instruction behavior, message preparation, and effect handling. The route declares the dependencies between operations. |
| **Program driver** | Per-invocation bookkeeping, including pending continuations, result tickets, the saved continuation PC, and the terminal route result. | How to enter a program, issue a checked instruction, resume its continuation, and report retirement, a breakpoint, a halt, or a fault. |
| **Scheduling backend** | Pending tasks and ready notifications, backed by queues, timers, device events, or a virtual clock. | When work becomes eligible and how to order ready turns and task completions under the runtime's policy. |
| **Runtime** | The loaded machines or the workers that own them, and the mapping from scheduled program turns to machine IDs. | Section placement, worker placement, and how to coordinate machine turns with device events. It delegates instruction execution to the program driver. |

An independent instruction stream has its own `Loadable` and PC. Devices
that participate in one stream can be fields of its composite. A runtime
can coordinate different machine types alongside multiple instances of
one machine type. The owner lends the driver the selected image and
machine for a turn. The one-machine checked example combines the owner
and program driver in one struct.

## Starting and placing machines

The host loads each section into its corresponding `Loadable`, queues
entry for each section, and starts the runtime. Entry happens when its
turn is selected.

```rust,ignore
for loadable in loadables {
    let machine = runtime.load(loadable);
    runtime.schedule_entry(machine, Vec::new())?;
}
runtime.run()?;
```

The runtime maps the selected turn to program entry, instruction issue,
or task delivery. Whenever an action leaves a machine runnable, the
runtime can queue its next instruction. The host wrapper can focus on
loading, device connections, and inspecting results.

A single-timeline emulator can coordinate eligible instructions and
timed device events in one loop, with an explicit order for equal
timestamps. A threaded runtime can place several machines on each
owner worker, run local tasks there, and use a shared executor for
eligible `Send` work. Completions from shared work return to the owner.
Machine count, worker count, and queue count are independent choices.

## Program state stays in the machine

A composite marks its `ExecutionState` field with `#[program]`. The
checked driver reaches it through `HasProgram`. `ExecutionState` owns
the PC, call frames, locals, operands, and run state. The program image
supplies the instruction vector indexed by the PC. `Loadable` pairs the
machine and image. The checked driver borrows the image and mutates
the machine through `Loadable::parts_mut`. Inline instructions can
borrow their instructions and context for the duration of dispatch.
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

## Program driver operations

The scheduler-facing operations are `enter`, `issue_one`, and
`resume_one`. Issue and resume return control before another instruction
is fetched. Their outcome identifies the originating PC and whether the
instruction retired, parked, halted, or hit a breakpoint. Ordinary
scheduler turns preserve a debugger stop. The host explicitly authorizes
a debugger step or continuation.

The checked examples expose a higher-level `ProgramDriver` interface.
A single-machine driver accepts machine ID `()`. The portable digitizer
example exercises the scheduler-facing operations on several machines.
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
Each arm generated by `#[composite]` calls the selected
`Step<Field, Instruction, Context>` with the instruction and context,
then returns its result. A route-backed `Step` passes its route to
`context.start`. The context handles route startup and suspension, and
the program driver reports pending work and retirement at the host
boundary. Inline steps use the same generated dispatch.
[Step](../../crates/vihaco-runtime/src/step.rs) · [Dispatch generation](../../crates/vihaco-runtime-derive/src/inventory.rs)

## Task submission and completion

The public scheduler boundary accepts owned work, such as a future or
a typed device request. Submission returns a ticket. The scheduler
drives the work and reports when its result is ready. The driver
records which invocation and continuation are waiting for that ticket.

| Term | Meaning |
| --- | --- |
| **Work** | An owned request with a typed output. The backend advances it according to its timing and thread policy. |
| **Task ID** | An opaque key scoped to a completion queue. A readiness notification identifies a result that can be taken. |
| **Ticket** | The driver's handle for taking the typed terminal result once. The backend publishes readiness after the result is available. |
| **Continuation** | The driver's saved next route stage. It consumes a ready result under a fresh machine borrow. |

A possible Rust interface leaves ticket storage to the backend.

```rust,ignore
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

pub trait Work {
    type Output: 'static;
}

pub enum Completion<T> {
    Finished(eyre::Result<T>),
    Cancelled,
}

pub trait Ticket<T> {
    fn id(&self) -> TaskId;
    fn try_take(&mut self) -> Option<Completion<T>>;
}

pub trait Submit<W: Work> {
    type Ticket: Ticket<W::Output>;

    fn submit(&self, work: W) -> eyre::Result<Self::Ticket>;
}

pub trait ReadyQueue {
    fn try_ready(&mut self) -> Option<TaskId>;
}
```

An error from `submit` means the backend declined the work. Once it
accepts the work, its ticket carries the terminal result. A ready
notification can be queued immediately. The owner records the ticket
and continuation before it consumes the notification. Draining a ready
queue requires no machine borrow.

Submission and readiness are public extension points for scheduler
authors. The backend can expose native timing and readiness operations
for a virtual clock or an external event source. A device may publish
progress, such as a playback tick and its effects, while a task is still
in flight. The runtime can schedule that progress before the terminal
result arrives.

The route interpreter uses machine-aware `Await` and `FutureDriver`
callbacks. The portable example implements the latter on the program
owner's side of the boundary. It submits an owned future, keeps the
typed ticket and continuation, and receives an opaque task ID from
the backend when the result is ready. In the Warehouse example,
`await_work()` registers interest in a work handle and the annotation
hook can select the earliest simulated delivery tick.
[Route contracts](../../crates/vihaco-runtime/examples/static_graph_routes/framework/runtime.rs) · [Portable driver](../../crates/vihaco-stdlib/examples/program_drivers/portable/owner.rs) · [Task boundary](../../crates/vihaco-stdlib/examples/program_drivers/portable/tasks.rs)

When a work handle resolves, its output continues along the route to the
selected field's effect handler. In the sample route, `Await` produces a
typed `PushToStack` effect and `apply_effect_at::<ProgramField>()` calls
`Apply` on the program field. The route completes after that field receives
the result. [Sample route](../../crates/vihaco-stdlib/examples/program_drivers/machine.rs)

### Ownership of parked routes

The driver keeps the parked route's continuation, ticket, saved PC,
and terminal result. The machine retains its visible PC and run state.
While a field is leased, the owner can still inspect the PC and access
other available fields. The invocation stays parked until its route
can proceed.

The backend drives work that owns its inputs and any lease. When a
result is ready, the program driver validates which invocation is
waiting and resumes its continuation with a fresh machine borrow.
One task can advance part of a route. Retirement happens when the
route completes.

Futures holding thread-local leases stay on their owner thread. A
backend that moves work across threads requires `Send` for that work
and returns its result to the machine owner. Cancelling work already
issued to external devices remains the backend's responsibility.
[Borrowing across suspension](foundational-work.md#borrowing-across-suspension)

## Checked issue, resumption, and retirement

1. **Issue.** Checked fetch remembers the originating PC and advances
   to the sequential continuation before dispatch. The fetch helper
   borrows instructions from the module and takes a separate dispatch
   context. The prototype context carries the module and a route backend,
   so an authored `Step` can simply return
   `context.start(self, &sample_route(), ops::Sample(instruction.0))`.
2. **Park.** An inline route returns its result as the `Step` result.
   If a route is still pending, the context saves the continuation PC
   and terminal result slot in the driver, then suspends `ExecutionState`.
   Checked fetch restores the originating PC. The driver returns
   `Pending` and retains the run command with the pending route.
3. **Resume.** The backend announces the ready task. The driver checks
   the waiting invocation, takes the typed result, restores the saved PC
   under the reentrancy guard, and resumes the route. If a stage submits
   more work, the driver keeps the instruction in flight and preserves
   its debugger authorization.
4. **Retire.** The driver counts one retirement at the originating PC
   once the route finishes. The next fetch is a separate turn. A
   higher-level `Continue` can schedule that turn under the saved command.
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
| [Portable digitizers](../../crates/vihaco-stdlib/examples/program_drivers/portable/mod.rs) | Runs two named SST sections through the same machine and route on a shared simulation timeline and on Tokio owner workers. |

The waveform, device completion, and async digitizer demos remain small
examples of route execution in isolation. They use the same route
capabilities without a loaded program. Run them with
`mise run static-graph-routes`. [Route-only examples](annotated-step-topologies.md#runnable-examples)

### Portable digitizers

The example uses one dialect, one composite, and one capture route.
Each SST section runs on its own `Loadable`. One starts with a capture.
The other branches around a halt before reaching the same capture
instruction. The wrapper loads the sections, wires scripted samples,
queues entry, and calls `run`.
[Machine and route](../../crates/vihaco-stdlib/examples/program_drivers/portable/digitizer.rs) · [SST program](../../crates/vihaco-stdlib/examples/program_drivers/portable/digitizers.sst)

The runtime uses the same execution loop on both backends. The simulator
places both machines on one clock and delivers the second section's
sample first at tick 4, followed by the first section's sample at tick
20. The Tokio backend assigns sections to configurable OS workers,
each with a `LocalSet` and wall-clock timers. The owner drives several
machines when they share a worker. The tests compare route nodes and
edges, sample values, capture counts, and halt PCs. They also drive
multiple captures in one invocation.
[Runtime loop](../../crates/vihaco-stdlib/examples/program_drivers/portable/runtime.rs) · [Simulation](../../crates/vihaco-stdlib/examples/program_drivers/portable/backends/sim.rs) · [Tokio workers](../../crates/vihaco-stdlib/examples/program_drivers/portable/backends/threaded.rs)

Use `cargo run -p vihaco-stdlib --features program-image-execution
--example program_drivers -- 1` to put both digitizers on one Tokio
worker. The default uses two workers.

## Open integration work

- **Owner scheduling.** Match several pending waits against incoming
  events and report progress before a task completes. Provide the
  invocation identity, typed wait reason, and earliest eligible time
  when available.
- **Drain gates.** A barrier may retire before the next instruction is
  eligible to fetch. Clearing the drain gate must add no retirement.
  An instruction waiting for input retires after its route finishes.
- **Reset and cancellation.** Invalidate pending work on reset, image
  replacement, and cancellation. A late event must never resume a
  replacement invocation. Validate this with a deferred fault after
  a visible effect and a late completion after reset.
- **Integration boundaries.** Connect suspending effect handling to
  the route interpreter and define how deferred routes retain program
  context. The portable example models independent streams. Timing
  constraints and resource arbitration between distinct machine types
  still need a shared-device scenario.
  [Foundational async traits](foundational-work.md#async-preparation-execution-and-effect-handling)

[Issue #204](https://github.com/QuEraComputing/vihaco/issues/204)
