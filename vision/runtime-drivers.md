# Execution Outcomes and Runtime Drivers

This document defines the boundary between one-instruction execution and the policies that select,
schedule, park, and resume work.

## Execution Outcomes, Suspension, and Time

After immediate effect handling, the route reaches a one-instruction execution state:

```rust
pub enum Execution {
    Complete,
    Parked,
}
```

This enum is the minimal status. A machine with a driver-owned program counter or external
scheduler uses a richer step outcome carrying owned control-flow and scheduling requests beside the
status. Faults remain errors unless a runtime specifically models traps as first-class state.

`Complete` means:

- The instruction and all immediate effect handling finished.
- The machine has reached a boundary at which its driver may select more work.
- It does not select the next instruction or imply that another instruction executes at the same
  modeled time.

`Parked` means:

- The handler atomically registered the work needed to resume.
- No borrow from instruction execution is needed to resume.
- The driver must not treat this execution context as runnable until the corresponding owned
  continuation becomes ready.

Instruction execution remains synchronous. `send`, `recv`, external I/O, and delayed hardware
completion express suspension through effects handled after `execute` returns.

Timing remains driver policy. The same instruction may have different modeled duration under
different drivers or configurations. Timing data may come from:

- Driver configuration.
- Optional route metadata.
- Runtime instruction data.
- A component result.
- Resource state.
- A configured timing table.
- A child clock.
- An external completion event.

Host execution time never determines modeled duration.

## Runtime and Program Drivers

A composite can execute one supplied runtime instruction, but that ability does not make it a
running system. Something must still obtain the next instruction from the configured source,
decide when it is eligible to run, interpret the result of the step, and repeat or stop. That
orchestration role is the **driver**. The source may itself be a modeled sequencer, so the driver
need not be the authority that computes the program counter.

This document uses **runtime** for the top-level running arrangement: a composite machine, a
selected driver policy, and any resolved programs used by that policy. This is distinct from a
*runtime instruction*, which is one fully resolved operation in a program.

### The Step/Driver Boundary

The stable machine boundary is one instruction:

```text
driver obtains a runtime instruction from the configured source
    -> machine.step(instruction)
        -> resolve runtime message
        -> execute on the selected component
        -> handle immediate effects
        -> return an owned outcome
    -> driver interprets the outcome
    -> driver selects the next work, waits, or stops
```

The driver is necessary because none of the following has one correct policy for every vihaco
machine:

- Whether instructions come from a stored program, an interactive caller, a device stream, or an
  event queue.
- Whether successful completion advances a cursor.
- Whether a branch mutates a machine component or returns a control request to the caller.
- Whether another instruction runs immediately or at a later modeled time.
- Whether one machine runs to completion or several machines are interleaved.
- Whether a parked operation blocks the caller, yields to another machine, or is exposed as an
  incomplete result.
- Whether breakpoints, tracing, deterministic replay, or external hardware completions participate
  in instruction selection.

`step` must therefore remain usable without a program counter or clock. A unit test, debugger, or
host application can construct a runtime instruction and call `step` directly. A program driver
builds repetition on top of exactly the same operation.

Conceptually, the boundary may be expressed as:

```rust
pub trait Step {
    type Instruction;
    type Outcome;
    type Fault;

    fn step(
        &mut self,
        instruction: &Self::Instruction,
    ) -> Result<Self::Outcome, Self::Fault>;
}
```

The associated outcome is intentionally machine-specific. A simple machine may need only
`Complete` and `Parked`; a control-flow machine may also need to communicate advance, jump, halt,
trap, or breakpoint information. The framework should not force every machine to carry control
states that it cannot produce.

The generated dispatch is still valuable even though every match arm has the same three stages.
The runtime instruction variant selects different concrete instruction types, target fields,
message resolvers, effect handlers, and fault conversions. The driver repeats `step`; it does not
replace that route-specific dispatch.

### Runtime Ownership

A convenient top-level owner places the driver and machine beside one another:

```rust
pub trait Driver<M> {
    type Output;

    fn run(&mut self, machine: &mut M) -> eyre::Result<Self::Output>;
}

pub struct Runtime<M, D> {
    pub machine: M,
    pub driver: D,
}

impl<M, D> Runtime<M, D> {
    pub fn run(&mut self) -> eyre::Result<D::Output>
    where
        D: Driver<M>,
    {
        self.driver.run(&mut self.machine)
    }
}
```

The exact API may differ, and the first implementation may use inherent `run` methods instead of a
common `Driver<M>` trait. The important ownership rule is that the driver is external to the
composite it drives. “External” here means that it is not a field that must borrow its containing
composite; it may still be a normal vihaco type in the same process and may be owned by a
`Runtime<M, D>`.

This sibling arrangement lets the driver hold its own mutable policy state while borrowing the
whole machine for a step. Placing the driver inside the machine would require mutably borrowing the
driver field and the containing machine at the same time whenever the driver calls `step`. It would
also make a particular execution policy part of the machine's hardware shape.

The driver should interact with machine state through explicit machine operations. It may inspect a
program counter, fetch through a program-storage interface, drain driver-facing requests, or reset
the machine when those operations are part of the selected design. It should not depend on the
private layout of arbitrary component fields.

### What a Driver Holds

A driver owns the state required by its selection and progression policy. Depending on the driver,
that may include:

- A resolved program or a reference to program storage.
- One program cursor, several cursors, or no cursor.
- Entry-point and halt state.
- Breakpoints, single-step mode, and debugger bookkeeping.
- A runnable set, event queue, modeled current time, and deterministic tie-breaking order.
- Pending external operations and the owned identifiers used to resume them.
- Reset generations used to reject stale completions.
- Replay input, recorded decisions, or a source of test instructions.

A driver does not own component invariants or execute component operations itself. It supplies a
runtime instruction to the composite and responds to the resulting outcome. Component-local state
remains in components, and cross-component effects remain routed by the composite.

The normal lifecycle is:

1. Parse SST and resolve it into a runtime program.
2. Load or attach that program according to the chosen storage model.
3. Reset the machine and driver state as required.
4. Select an entry point or initial event.
5. Select a runtime instruction and call `step`.
6. Interpret completion, control flow, scheduling requests, parking, or faults.
7. Repeat, wait for a completion, or return a terminal result.

### Driver Families

Drivers are policies rather than a second kind of machine. Different use cases should be able to
reuse the same composite:

| Driver | Typical state | Selection policy |
|---|---|---|
| Sequential interpreter | Program and one cursor | Run the instruction at the cursor, then advance or apply control flow |
| Single-step/debugger | Program, cursor, breakpoints, inspection state | Stop at requested boundaries and expose machine state between steps |
| Timeline/emulation driver | Global time, event queue, runnable machines, one or more cursors | Run the earliest eligible event and schedule its follow-up work |
| Cooperative multi-program driver | Programs, cursors, runnable queue | Interleave several execution contexts according to an explicit policy |
| Externally driven adapter | Pending host or device input | Execute instructions supplied by another process or hardware controller |
| Hardware completion driver | Outstanding operations and completion identifiers | Resume work in response to device completions or interrupts |
| Replay/test driver | Recorded or generated instruction decisions | Reproduce a trace or explore instruction sequences deterministically |

These can be layered. A debugger may wrap a sequential or timeline driver. A replay facility may
record the choices of another driver. A host adapter may feed instructions to a machine that has no
stored program or program counter at all.

The initial implementation should begin with concrete drivers needed by reference machines. A
universal driver trait is useful only if those drivers demonstrate a stable shared contract. The
one-instruction `Step` boundary is more fundamental than requiring every orchestration policy to
fit one trait immediately.

### Drivers and Clocks

A clock is not intrinsically a framework-wide authority. It may occupy either of two roles:

1. An ordinary component or handler that owns local clock state, translates device ticks, records
   durations, or produces scheduling requests.
2. A driver whose notion of global time determines which instruction or event executes next.

A sequential interpreter that runs at its caller's pace may have no clock. A machine may contain a
child clock component while being driven sequentially. Conversely, a global emulation clock may be
the timeline driver and hold the event queue, current modeled time, programs, and cursors for
multiple child machines. A driver need not be a clock, and a clock need not be a driver.

This distinction also defines how effects reach a clock. Internal components continue to handle
effects through the same typed, route-specific handling model as every other destination. An effect
may be sent deterministically to several handlers—for example, a child clock that translates a
device delay and a debug component that records it. Each handler receives the same semantic effect
in declaration order, may mutate its own component, and may emit owned follow-up effects. The
shared input need not be consumed by the first handler, and no separate handling semantics are
required merely because one handler uses the effect only for diagnostics.

If an effect must influence the external driver, its scheduling meaning must survive the `step`
boundary as owned data. A route can accomplish that in either of two broad ways:

- Return a driver-facing request as part of the step outcome.
- Record the request in explicitly exposed machine state that the driver drains after the step.

Returning owned requests makes the boundary clearest, while machine-owned queues may be appropriate
when queueing is itself modeled hardware. The first implementation can choose the simpler
representation without changing the semantic rule: scheduling work intended for an external
driver cannot be consumed exclusively by an internal handler.

A typical clock hierarchy is:

```text
instruction emits device scheduling effect
    -> route sends it to child clock and diagnostic handlers
    -> child clock converts device ticks to a global scheduling request
    -> step returns that owned request
    -> global clock-driver inserts it into the event queue
    -> driver resumes the machine when the event becomes current
```

The global clock does not need to see every mutation performed directly by `Execute<I>`. It only
needs the information that affects global ordering, modeled duration, or readiness. When a direct
mutation has such consequences, the instruction result or its route must expose the relevant fact
or scheduling request. Purely component-local changes can remain local.

Any operation that may park must make that fact visible at the driver boundary. The operation may
first emit an effect that an internal resource handles, but the resulting `Parked` outcome and owned
continuation identity must reach the driver. An instruction must not silently block inside
`Execute<I>` or leave the driver believing that the execution context is still runnable.

### Program and Program-Counter Placement

Program storage, a program cursor, and the policy that advances the cursor are separate concepts.
They may be colocated for convenience, but the architecture should not require them to have the
same owner.

| Placement | Appropriate when | Consequences |
|---|---|---|
| Program and cursor in the driver | Ordinary interpretation, debugging, replay, or several cursors over shared code | The machine receives selected instructions and need not model program storage |
| Program in the driver, cursor in the machine | The program is host-owned but the program counter is visible or mutable hardware state | The driver reads the machine cursor, fetches the instruction, and lets machine policy determine the next cursor |
| Program in the machine, cursor in the driver | Program memory is modeled or device-resident but progression is host-controlled | The driver fetches through an explicit machine operation and owns advance/jump policy |
| Program and cursor in the machine | A sequencer or control-flow unit owns both fetch state and progression | The external driver obtains the next owned instruction through the sequencer and then calls `step` |
| Program supplied externally, no cursor | Interactive execution, streaming control, tests, or a hardware command source | Each instruction is supplied directly and `step` remains fully usable |

Resolved program contents are usually immutable and may be shared. A cursor is mutable execution
state and there may be several cursors for one program. The rewrite should therefore model program
data and per-execution cursor state as distinct concepts even if it retains a convenient
one-program/one-cursor wrapper. The existing `ProgramImage` shape can remain such a convenience,
but combining a module and one program counter must not make that placement a requirement for all
drivers.

When the driver owns the cursor, control-flow handling returns driver-facing control such as
advance, jump, call, return, halt, or park. The driver is the sole authority that applies those
changes. It must not increment the cursor before the step and then also apply an advance outcome.

When the machine owns the cursor, a selected control-flow component or route handler mutates it.
The machine's route policy also applies its ordinary sequential advance when no explicit
control-flow operation replaces it. The driver fetches using the current value and reads the
updated value after the step. In this arrangement the driver must not independently infer that
every completed instruction advances by one. There must be one authority for each cursor
transition.

Machine-owned cursors are important for hardware-oriented models. A sequencer, branch unit,
interrupt controller, direct-memory-access engine, or external device may drive the program
counter. Treating the cursor as a component permits those modeled hardware operations to mutate it,
while the external driver remains responsible for deciding when the machine is allowed to perform
work. A timeline driver can therefore schedule a sequencer without pretending that the global clock
owns the sequencer's program counter.

Rust ownership affects the fetch interface when program storage lives inside the same machine that
will be mutably stepped. A driver cannot retain a reference borrowed from the machine's program
field while also borrowing the whole machine mutably for `step`. The selected API must end the
fetch borrow before execution—for example, by returning an owned runtime instruction—or separate
immutable program storage from the mutable composite. This is a concrete ownership constraint, not
a reason to prescribe one placement for all machines.

### Parking and Resumption

Parking divides ownership between the machine and driver:

- The component or resource owns the continuation data required to finish the operation.
- The composite ensures that effect handling registers that continuation atomically.
- The step outcome tells the driver that the execution context is no longer runnable.
- The driver owns when the context re-enters its runnable set.
- A completion event carries an owned identity that can be checked against resets or cancellation.

No borrow from resolution, execution, or effect handling may survive the step. A simple sequential
driver that does not wait for asynchronous work may return `Parked` to its caller. A timeline driver
may keep running other machines until the relevant event is ready. A hardware driver may wait for
an external completion and then resume the registered continuation. These are different driver
policies over the same machine boundary.

### Consequences for the Rewrite

The rewrite should establish these pieces in order:

1. Generate a one-runtime-instruction `step` operation for each composite.
2. Make its outcome sufficient for a caller to distinguish completion, parking, terminal control,
   and driver-facing work required by the reference machine.
3. Implement a simple sequential driver without requiring a clock.
4. Keep program data and cursor state conceptually separate, with an initial convenient ownership
   arrangement.
5. Add a timeline driver in which the global clock owns selection and scheduling policy.
6. Demonstrate a machine-owned program counter so hardware-driven progression does not become an
   afterthought.
7. Generalize a common driver trait only after these concrete drivers reveal the shared API.

This division keeps component execution reusable while allowing each runtime to decide what
“next,” “now,” and “runnable” mean.

## Atomicity and Faults

Atomicity means that another instruction from the same machine does not interleave with the current
step. It does not imply rollback. Every step reaches one of three boundaries:

- Complete.
- Parked with a registered continuation.
- Faulted.

Message resolution may consume operands before execution, and the selected component may mutate
itself before returning a fault. A terminal fault may therefore leave partially consumed or
mutated state.

Avoiding automatic rollback prevents the common path from cloning values solely to recover from a
fault. An operation that requires transactional semantics implements them explicitly in its owning
component or resource.

Each route documents the relevant failure boundary:

- Whether operands are read or consumed.
- Which mutations may occur before a fault.
- Whether effect handling itself can fault.
- Whether a parked operation is cancellable.
- What happens to stale completions after reset.
