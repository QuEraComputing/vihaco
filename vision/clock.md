Clock, send, and recv in execution mode

Instead of having some scheduler or executor for send/recv, we will have the notion of a clock:
- Per composite clocks,
- Global clocks

We can think of a "clock" as a timeline; each event that could happen during the execution of the CPU
has some associated unit of time, and we place these on the timeline. Then, we split this timeline up
into ticks, where we define each tick to have a unit of time associated with it. Then, during execution,
while we are working through ticks, each event that has its time within that tick will be executed in the
order of their times.

Global clocks will be responsible for syncing the clocks of the devices below it, think like a translator;
for example, I have a global clock, and two child CPUs, each with their own clock:
- On CPU 1, ADD takes 3 tick of the global clock
- On CPU 2, ADD takes 1 tick of the global clock

So when we execute one global clock tick:
- 3 ticks on CPU 1 will pass, and
- 1 tick on CPU 2 will pass.

We will have some way for instructions to dictate how much time passes. For example:
- send takes one clock tick
- recv takes one clock tick + however long it takes for the data to be received.

If recv doesn't have the value available, that thread of execution will continue to be parked. Then, it will
be awoken once we have a value, and execution will continue.

Questions:
- What should the clock look like?
- How do runtimes assign a unit of time to each instruction? Some Tick trait?
- How are clocks synced?
- What should the clock own?

---

## Updated Direction

The material above records the questions that motivated the clock design. The current direction is
defined together with the architecture mapped in [`contents.md`](./contents.md) and the two-CPU
integration target in [`demo.md`](./demo.md).

A clock is not a universal vihaco authority and does not replace instruction dispatch, resource
handling, or the driver. Clock implementations are reusable library components built through the
same component and effect model as stacks, arithmetic units, and communication resources. Vihaco
core supplies the boundaries that let those components participate:

- A composite executes one supplied runtime instruction through `step`.
- Routes may associate execution with timing information.
- Effects can be handled by local components and propagated across nested composites.
- A step returns owned status and driver-facing work.
- Parked execution registers owned continuation state.
- An external driver selects the next eligible work.

The two-CPU demo chooses one concrete arrangement:

```text
Runtime
├── TimelineDriver
└── HeterogeneousMachine
    ├── GlobalClock
    ├── reusable communication component
    ├── CpuA
    │   └── LocalClock { global_ticks_per_local_cycle: 2 }
    └── CpuB
        └── LocalClock { global_ticks_per_local_cycle: 3 }
```

`GlobalClock` is modeled state inside the top-level composite. `TimelineDriver` remains external so
it can use the clock and CPU state without a field borrowing its containing machine. Another
runtime may place its global clock state inside the driver instead. Clock placement is a runtime
choice, not part of the `Instruction` or `Execute<I>` contracts.

## Time, Duration, and Local Cycles

The model distinguishes three quantities:

- **Global tick** is an absolute position on the definitive machine timeline.
- **Global duration** is a distance between two global ticks.
- **Local cycles** count work in the domain of one child clock.

They should not be interchangeable integers. Conceptually:

```rust
pub struct GlobalTick(pub u128);
pub struct GlobalDuration(pub u128);
pub struct LocalCycles(pub u64);
```

The exact representation remains a library API decision. Distinct types prevent an absolute time
from being used as a duration and prevent one CPU's local cycles from being mistaken for global
ticks. Arithmetic that advances time or converts cycles must detect overflow rather than silently
wrapping.

Host execution time has no relationship to modeled time. A slow Rust call can represent zero
modeled duration, while a fast call can schedule work far into the future.

## Global Clock

The global clock is the definitive time authority for a particular modeled machine. In the demo it
owns:

- The current `GlobalTick`.
- An ordered collection of future events.
- A monotonically increasing sequence used to order events at the same tick.
- Any generation or reset state required to reject stale work.

It does not:

- Fetch or execute runtime instructions.
- Advance a program counter.
- Borrow a CPU and call its `step` method.
- Interpret arithmetic, communication, or other domain effects.
- Observe every mutation made by every component.

Those responsibilities belong to the driver, the configured program-counter owner, and typed
effect handlers.

The global clock can be generic over the event type used by a library or machine:

```rust
pub struct Scheduled<E> {
    pub at: GlobalTick,
    pub sequence: u64,
    pub event: E,
}
```

Events are ordered by `(at, sequence)`. Sequence order makes same-tick behavior deterministic and
prevents device field order or collection iteration order from accidentally changing execution.
Additional phases may later become part of the ordering key if a runtime needs separate evaluation
and visibility stages.

The first implementation is event-driven. It advances directly to the next scheduled event rather
than visiting every empty global tick:

```text
remove the earliest event
    -> advance GlobalClock.now to its tick
    -> return the owned event to the driver
    -> driver performs the selected work
    -> insert resulting events
    -> repeat
```

Skipped ticks remain meaningful positions on the timeline; they simply contain no observable work.

## Local Clocks

A local clock relates child execution to the global timeline. It is not an independent time
authority. The demo begins with a fixed integer ratio:

```rust
pub struct LocalClock {
    pub cycle: u64,
    pub global_ticks_per_local_cycle: u64,
}
```

The ratio must be nonzero. A production API may enforce that invariant with construction-time
validation or a nonzero numeric type.

Conversion follows:

```text
global duration =
    local cycles × global ticks per local cycle
```

For the demo:

```text
CpuA: 1 local cycle × 2 = 2 global ticks
CpuB: 1 local cycle × 3 = 3 global ticks
```

Both CPUs may therefore execute the same `add` runtime instruction through the same reusable
arithmetic component and report one local cycle, while becoming eligible at different global
ticks.

A local clock may be an ordinary component and typed handler. It can accept route completion
information, update its local cycle count, and produce an owned global scheduling request. A debug
component may handle the same completion information for tracing. Both use the same typed handler
model.

Child clocks do not advance private timelines and later reconcile them with the parent. Their
converted work is scheduled directly on the common global timeline, so global event ordering
defines how child execution interleaves.

The fixed integer ratio is sufficient for the integration demo. Rational periods, phase offsets,
drift, and clock-domain crossings can be library extensions after this model is proven.

## Clock and Driver Roles

A clock and a driver answer different questions:

| Question | Owner in the demo |
|---|---|
| What is the current definitive tick? | `GlobalClock` |
| Which event is earliest? | `GlobalClock` event ordering |
| Which work does that event represent? | The machine-specific event type |
| Who obtains the corresponding instruction or completion? | `TimelineDriver` through explicit machine operations |
| Who calls `step`? | `TimelineDriver` |
| Who applies returned scheduling requests? | `TimelineDriver`, by inserting them into `GlobalClock` |
| Who advances a CPU program counter? | The CPU's modeled program-counter component |

The driver loop is:

```text
read the earliest event from GlobalClock
    -> advance global time
    -> identify the target CPU or completion
    -> obtain an owned runtime instruction or completion
    -> call the top-level machine route
    -> interpret Complete, Parked, terminal control, and scheduling work
    -> return future events to GlobalClock
```

The driver must not retain a reference borrowed from a child program while mutably stepping the
whole machine. A CPU-owned program source therefore returns an owned runtime instruction, or the
immutable program is stored outside the mutable composite.

A clock can itself fill the driver role in another runtime when it is external to the machine and
owns both event selection and the driving loop. The demo keeps the roles separate because its
global clock is explicitly a field of the top-level composite.

Vihaco must also support drivers with no clock. A sequential interpreter or direct caller can
invoke `step` without modeled time. The existence of `GlobalClock` and `LocalClock` library types
does not make clocks a requirement for a composite.

## Instruction Timing

Runtime instructions describe semantic operations. They do not own a clock, event queue, driver, or
universal timing trait. The same `Add` type can have different duration in different routes or
machines.

Timing information may come from:

- A route default.
- Optional route metadata.
- Runtime instruction data.
- A component result.
- Resource state.
- Driver configuration.
- An external completion event.

The initial demo uses route-level local duration:

```text
add -> 1 local cycle
sub -> 1 local cycle
mul -> 1 local cycle
send -> 1 local cycle
successful recv -> 1 local cycle
```

This information does not belong in the reusable arithmetic component. After the route completes,
the selected local clock translates its local duration and emits driver-facing global scheduling
work.

```text
runtime instruction
    -> resolve message
    -> execute on selected component
    -> handle semantic effects
    -> apply route-local timing through LocalClock
    -> return status and owned scheduling work
```

An instruction that mutates its component and returns `Effects<NoEffect>` still receives route
timing. The global clock does not need to observe the mutation or every effect. It only receives the
information required to determine global eligibility.

A `Tick` trait implemented by every instruction is not required. If repeated timing APIs become
useful after the first implementation, they can describe route or runtime timing without coupling
semantic instruction types to one clock model.

## Scheduling Requests

Scheduling work that affects an external driver must cross the `step` boundary as owned data, or be
stored in explicit machine state that the driver drains. Returning owned requests is the clearest
initial model.

Conceptually, a request identifies when and what becomes eligible:

```rust
pub struct Schedule<E> {
    pub after: GlobalDuration,
    pub event: E,
}
```

The driver submits the request to `GlobalClock`. The clock converts `after` to an absolute tick
relative to its current `now`, validates the arithmetic, assigns a deterministic sequence, and
inserts the event. An alternative request may already contain an absolute tick when that time comes
from an external source.

The concrete event sum is machine- or library-specific. Vihaco core does not define `RunCpu`,
`DeliverValue`, or other demo events. It only needs an owned step boundary through which the
configured runtime can communicate scheduling work.

Scheduling the past is an error. Scheduling at the current tick is allowed when same-tick sequence
ordering defines when the new event becomes visible.

## Completion, Parking, and Readiness

Timing does not replace the instruction execution status:

```rust
pub enum Execution {
    Complete,
    Parked,
}
```

This is the minimal status; the actual step outcome may also contain terminal control and
driver-facing work.

`Complete` means the instruction and all immediate effect handling have reached a step boundary. If
the program has another instruction, its route normally returns scheduling work based on the local
duration. If the program is exhausted, the CPU leaves the runnable set instead.

`Parked` means the resource or component has atomically registered an owned continuation and the
driver must not schedule the CPU's next instruction. Parking is a readiness decision, not an
unknown duration added to an otherwise complete instruction.

When a completion becomes available:

1. A library handler identifies the parked CPU and continuation.
2. The parent routes the owned completion to that child.
3. The continuation applies its result.
4. The child's local clock accounts for the completion duration.
5. A global event makes the CPU eligible after the converted duration.

No borrow from resolution, execution, or effect handling survives the parked step.

## Communication Timing

`send` and `recv` demonstrate timing, but their resource semantics belong to a reusable
communication library rather than vihaco core.

For `send`:

```text
resolve:
    consume the local stack value

execute and handle:
    emit an owned library-defined transmission request
    account for the route's local duration
```

Sender acceptance and value delivery are distinct events. A library may choose:

- Immediate acceptance with delivery at the current global tick.
- Immediate acceptance with delivery at a future tick.
- Parked acceptance until capacity or a receiver becomes available.

For `recv`:

```text
value available:
    remove and deliver the value
    account for the successful receive duration
    complete

value unavailable:
    atomically register an owned continuation
    return Parked without scheduling the next instruction
```

The atomic check-or-register operation prevents a value from arriving between the availability
check and waiter registration.

The integration demo initially uses immediate delivery at the sender's current global tick.
Sequence order defines visibility relative to other events at that tick. A later communication
library may introduce transport latency without changing vihaco's clock, instruction, or composite
contracts.

## Nested Clock and Effect Flow

The demo uses two levels of composite routing:

```text
CpuA Add completes with 1 local cycle
    -> CpuA LocalClock converts it to 2 global ticks
    -> owned scheduling request leaves CpuA
    -> HeterogeneousMachine returns it to TimelineDriver
    -> TimelineDriver inserts CpuA eligibility into GlobalClock
```

`CpuB` follows the same path but converts one local cycle to three global ticks.

A communication completion follows the inverse direction:

```text
GlobalClock releases delivery event
    -> TimelineDriver routes the owned event through HeterogeneousMachine
    -> communication handler identifies the waiting CPU
    -> parent forwards the completion into the child
    -> child continuation completes recv
    -> LocalClock schedules the child's next eligibility globally
```

The framework preserves nested route identity and ownership. Clock and communication libraries
define the event contents and resource behavior.

## Demonstration Trace

The trace in [`demo.md`](./demo.md) is the acceptance case for the clock model. Its important timing
points are:

```text
global 0: CpuA add; next eligible at 2
global 0: CpuB sub; next eligible at 3
global 2: CpuA send; next eligible at 4
global 3: CpuB recv completes; next eligible at 6
global 4: CpuA recv parks
global 6: CpuB mul; next eligible at 9
global 9: CpuB send satisfies CpuA's receive
global 11: CpuA becomes eligible after one local receive cycle
```

This proves:

- Both CPUs use the same semantic instructions and local duration.
- Local clock configuration produces different global eligibility.
- The global event order is definitive and deterministic.
- A parked receive removes a CPU from normal instruction scheduling.
- Delivery resumes the correct continuation and re-enters the timeline through its local clock.

## Ownership Boundaries

The demo assigns ownership as follows:

| Owner | State and policy |
|---|---|
| Vihaco core | Typed instructions, execution relationships, effects, route generation, step status, and owned driver boundary |
| `GlobalClock` library component | Current global tick, event queue, sequence allocation, and reset generation |
| `LocalClock` library component | Local cycle state and local-to-global conversion policy |
| `TimelineDriver` library item | The loop that selects events, invokes machine work, and applies scheduling requests |
| CPU composite | Local architectural state, selected instruction routes, program, program counter, and parked status |
| Communication library | Values in flight, waiting continuations, acceptance, delivery, and transport timing |
| Runtime instruction | Fully resolved semantic operands |

Instructions do not own clocks, queues, wakers, or scheduler state. The global clock does not own
component semantics or instruction dispatch. The driver does not mutate private fields directly;
it uses explicit machine operations.

## Faults, Reset, and Deadlock

Clock and scheduling faults retain enough context to identify:

- The current global tick.
- The event sequence and target.
- The CPU and route when applicable.
- The requested duration or absolute tick.
- Whether the failure occurred during conversion, insertion, dispatch, or completion.

Reset invalidates pending work through a generation or equivalent identity. A completion created
before reset cannot resume a newly reset CPU that happens to reuse the same local identifier.

The driver detects deadlock when:

- No runnable CPU remains.
- Every incomplete CPU is parked.
- The global event queue contains no event capable of satisfying a continuation.

Deadlock is distinct from successful program exhaustion and from waiting on an external event that
the selected driver knows may still arrive.

## Implementation Sequence

Clock work should develop alongside the instruction rewrite and demo:

1. Define distinct global tick, global duration, and local cycle types.
2. Implement a deterministic generic global event queue with checked time arithmetic.
3. Implement fixed-ratio local clock conversion.
4. Add owned scheduling work to the composite step outcome.
5. Drive one clocked CPU through route-local timing.
6. Place two CPU instances under one global clock and verify the two ratios.
7. Add library-defined send delivery.
8. Add parked receive, owned completion, wakeup, and stale-generation protection.
9. Assert the deterministic trace from `demo.md`.

Each stage should leave a focused runnable test. The concrete `GlobalClock`, `LocalClock`, and
`TimelineDriver` APIs may begin as ordinary library types. Common traits or macro shorthand should
be introduced only after these implementations expose stable repetition.

## Acceptance Criteria

The clock model is ready for the integration demo when:

- Global tick, global duration, and local cycles are distinct types.
- Host execution time never changes modeled time.
- Global time advances monotonically.
- Same-tick events execute in deterministic sequence order.
- Empty spans can be skipped without changing results.
- `CpuA` converts one local cycle to two global ticks.
- `CpuB` converts one local cycle to three global ticks.
- The same arithmetic instruction can have different global duration without knowing either clock.
- A completed route schedules only the next eligible work.
- A parked route schedules no next instruction.
- A communication completion resumes only its registered continuation.
- Resume timing passes through the waiting CPU's local clock.
- Program exhaustion, deadlock, parking, and external waiting are distinguishable.
- Reset prevents stale scheduled work from mutating a new execution generation.
- The global clock remains an ordinary library component rather than a required vihaco core
  concept.
- A clockless sequential driver can use the same `step` boundary.

## Deferred Questions

The first implementation does not need to decide:

- Fractional or irrational clock ratios.
- Phase offsets and clock drift.
- Multiple visibility phases within one global tick.
- Stochastic latency.
- Real-time pacing against a wall clock.
- Distributed event queues.
- Dynamic clock-tree reconfiguration.
- General cancellation of in-flight operations.

These features may extend the library-level clock and driver implementations later. They do not
change the ownership boundaries defined by [`instruction-model.md`](./instruction-model.md),
[`execution-pipeline.md`](./execution-pipeline.md), and
[`runtime-drivers.md`](./runtime-drivers.md), or the integration behavior required by
[`demo.md`](./demo.md).
