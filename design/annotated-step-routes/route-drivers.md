# Drivers for annotated step routes

A runtime coordinates a set of machines. A program driver enters a
selected program, fetches using its program counter, and dispatches one
instruction at a time. When a route parks, the driver keeps the
instruction in flight. The scheduling backend advances the pending
work and reports when a result can be delivered.
[Annotated step topologies](annotated-step-topologies.md)

## Canonical runtime structure

| Layer | What it owns | What it decides |
| --- | --- | --- |
| **Machine** | Devices and live `ExecutionState`, including the PC, call frames, and operand storage. Its `Loadable` pairs it with a program image. | Instruction behavior, message preparation, and effect handling. The route declares the dependencies between operations. |
| **Program driver** | Per-invocation bookkeeping, including pending continuations, result tickets, the saved continuation PC, and the terminal route result. | How to enter a program, issue a checked instruction, resume its continuation, and report retirement, a breakpoint, a halt, or a fault. |
| **Scheduling backend** | Pending tasks and ready notifications, backed by queues, timers, device events, or a virtual clock. | When work becomes eligible and how to order ready turns and task completions under the runtime's policy. |
| **Runtime** | The loaded machines or the workers that own them, and the mapping from scheduled program turns to machine IDs. | Section placement, worker placement, and how to coordinate machine turns with device events. It delegates instruction execution to the program driver. |

The runtime owns each `Loadable` and lends the selected machine and image
to the program driver for one turn. The driver owns instruction and
continuation bookkeeping. The scheduling backend takes owned work and
reports ready task IDs. It can serve several machines without borrowing
any of them. The runtime maps each ready ID to a program turn or asks the
driver which parked invocation owns the result. This is the canonical
split for both single-machine and multi-machine runtimes.

An independent instruction stream has its own image and PC. Devices in
that stream can be fields of its composite. Warehouse uses one stream,
and the digitizer runs several streams. They share the same runtime,
program driver, and simulator and Tokio backends.
[Shared runtime](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/runtime.rs) · [Shared program driver](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/driver.rs)

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

Warehouse uses `Runtime::execute(loadable, args)` to load one stream,
queue entry, and run to halt in one host call. Internally it uses the
same entry queue and execution loop as the multi-stream digitizer.

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

The runtime-facing operations are `enter`, `issue_one`, and `resume`.
Issue and resume return control to the runtime before another instruction
is fetched. The shared example driver uses the machine's run state to
communicate whether the stream can be scheduled again. The RFC's driver
interface can also expose retirement and debugger stops to the runtime.
A debugger stop stays in force until the host authorizes a step or
continuation.

The inline, channel, and debugger demos use `CheckedDriver` as a
single-machine host adapter around the command-driven `ProgramDriver`
interface. It combines ownership and program driving for those demos.
Warehouse and the digitizer illustrate the canonical scheduler boundary.
[ProgramDriver](../../crates/vihaco-stdlib/src/program_driver.rs) · [Command-driven adapter](../../crates/vihaco-stdlib/examples/program_drivers/checked.rs)

| Host method | What it authorizes |
| --- | --- |
| `start(machine, arguments)` | Enters the loaded program at its entry point and creates the first frame. |
| `run(machine, command)` | Fetches and dispatches under a `Continue` or `SingleStep` command. |
| `deliver(machine, work, delivery)` | Advances the selected pending instruction under its saved command. |

The command-driven adapter reports a typed stop. `Pending` includes the originating PC
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
notification can be queued immediately. The driver records the ticket
and continuation before the runtime consumes the notification. Draining a
ready queue requires no machine borrow.

Submission and readiness are public extension points for scheduler
authors. The backend can expose native timing and readiness operations
for a virtual clock or an external event source. A device may publish
progress, such as a playback tick and its effects, while a task is still
in flight. The runtime can schedule that progress before the terminal
result arrives.

The route interpreter uses machine-aware `Await` and `FutureDriver`
callbacks. In the scheduled examples, the driver implements
`FutureDriver` by submitting an owned future and keeping the typed
ticket and continuation. Warehouse uses the same ticket path for
annotated timing. Its driver reads the node annotation, asks the
backend for a deadline at issue, and submits the owned label for
delivery at that deadline. The example's optional `TimedSubmit`
capability keeps deadlines specific to the backend. The backend
reports readiness by task ID.
[Route contracts](../../crates/vihaco-runtime/examples/static_graph_routes/framework/runtime.rs) · [Route driver](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/driver.rs) · [Task boundary](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/tasks.rs)

When a work handle resolves, its output continues along the route to the
selected field's effect handler. In the sample route, `Await` produces a
typed `PushToStack` effect and preserves the sensor as its source.
`handle_effect_at::<ProgramField>()` calls the composite's hook for that
source and destination. The hook calls `Apply` on the program field. [Sample route](../../crates/vihaco-stdlib/examples/program_drivers/machine.rs)

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
   context. The context carries the module and route startup services,
   so an authored `Step` can return
   `context.start(self, &print_route(), ops::PrintLabel(instruction.0))`.
2. **Park.** An inline route returns its result as the `Step` result.
   If a route is still pending, the context saves the continuation PC
   and terminal result slot in the driver, then suspends `ExecutionState`.
   Checked fetch restores the originating PC. The runtime leaves this
   stream parked and selects another ready turn.
3. **Resume.** The backend announces the ready task. The driver checks
   the waiting invocation, takes the typed result, restores the saved PC
   under the reentrancy guard, and resumes the route. If a stage submits
   more work, the driver keeps the same instruction in flight.
4. **Retire.** The instruction is complete once its route finishes. If
   the machine is runnable, the runtime queues the next fetch as a
   separate issue turn.
5. **Fault.** A deferred route error leaves the visible PC at the
   originating instruction and retains effects already applied.

The module-only `execute_instruction` entry point wraps the same checked
fetch helper. Existing `Loadable` callers and the image-patching debugger
keep using that API. The shared driver tracks one in-flight instruction
per machine. The command-driven debugger demo checks breakpoints before
fetch and preserves single-step across two waits. Integrating async
suspension into the image-patching debugger remains open.
[Checked fetch](../../crates/vihaco-stdlib/src/execution_state/runner.rs) · [Existing debugger](../../crates/vihaco-stdlib/src/debugger/runner.rs) · [Sample Step](../../crates/vihaco-stdlib/examples/program_drivers/machine.rs)

## Runnable examples

Run the five scenarios with `mise run program-drivers`.

| Runner | Behavior |
| --- | --- |
| [Inline CPU](../../crates/vihaco-stdlib/examples/static_execution_state/machine.rs) | Uses the checked driver and borrowed synchronous routes. A branch skips an instruction and the CPU leaves its arithmetic result on the stack before the next fetch. |
| [Debugger](../../crates/vihaco-stdlib/examples/program_drivers/scenarios.rs) | Preserves a single-step across two waits, retires the sample instruction once, and stops independently at the next breakpoint. |
| [Channel](../../crates/vihaco-stdlib/examples/program_drivers/scenarios.rs) | Delivers an external sample to a parked route. A stale work ID is rejected and the sensor issues once. |
| [Warehouse](framework-guide.md#warehouse-label-printing-walkthrough) | Runs one SST image on simulation and Tokio through the same `execute` entry point. Simulation delivers the label at tick 17 after issuing at tick 5. Tokio interprets the same annotation with a wall-clock timer. |
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
[Runtime loop](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/runtime.rs) · [Simulation](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/backends/sim.rs) · [Tokio workers](../../crates/vihaco-stdlib/examples/program_drivers/scheduled/backends/threaded.rs)

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
- **Integration boundaries.** Define how deferred routes retain program
  context. The portable example models independent streams. Timing
  constraints and resource arbitration between distinct machine types
  still need a shared-device scenario.
  [Foundational async traits](foundational-work.md#async-preparation-execution-and-effect-handling)

[Issue #204](https://github.com/QuEraComputing/vihaco/issues/204)
