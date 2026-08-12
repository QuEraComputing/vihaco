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
defined together with the architecture mapped in [`contents.md`](./contents.md), the execution
pipeline in [`execution-pipeline.md`](./execution-pipeline.md), and the two-CPU integration target
in [`../demos/examples/demo.md`](../demos/examples/demo.md).

A clock is not a universal vihaco authority. Clock implementations are reusable library components
built through the same component and effect model as stacks, arithmetic units, and communication
resources. Vihaco core supplies the boundaries that let them participate:

- An executable child composite performs one supplied runtime instruction through `step`.
- Routes may associate execution with timing information.
- Effects can be handled by local clocks and propagated across nested composites.
- A child step returns owned status and root-facing work.
- Parked execution registers owned continuation state.
- The top-level runtime root selects and dispatches the next event.

The two-CPU demo chooses one concrete arrangement:

```text
HeterogeneousMachine
├── GlobalClock<MachineEvent>
├── reusable communication component
├── CpuA
│   └── LocalClock { global_ticks_per_local_cycle: 2 }
└── CpuB
    └── LocalClock { global_ticks_per_local_cycle: 3 }
```

`HeterogeneousMachine` is both the top-level composite and the concrete runtime root. It has no
local executable instruction section or program. Its inherent `run` loop removes owned events from
`GlobalClock`, dispatches them into the appropriate child or resource, and returns owned scheduling
requests to the clock.

The initial implementation deliberately does not introduce an external `TimelineDriver`,
`Driver<M>` trait, or `Runtime<M, D>` wrapper. Those abstractions can be revisited after a second
runtime demonstrates a different orchestration policy and a stable shared boundary.

## Time, Duration, and Local Cycles

The model distinguishes three quantities:

- **Global tick** is an absolute position on the definitive machine timeline.
- **Global duration** is a distance between two global ticks.
- **Local cycles** count work in the domain of one child clock.

They should not be interchangeable integers:

```rust
pub struct GlobalTick(pub u128);
pub struct GlobalDuration(pub u128);
pub struct LocalCycles(pub u64);
```

The exact representation remains a library API decision. Distinct types prevent an absolute time
from being used as a duration and prevent local cycles from being mistaken for global ticks.
Arithmetic that advances time or converts cycles must detect overflow rather than silently
wrapping.

Host execution time has no relationship to modeled time. A slow Rust call can represent zero
modeled duration, while a fast call can schedule work far into the future.

## Global Clock

The global clock is the definitive time authority for the demo. It owns:

- The current `GlobalTick`.
- An ordered collection of future events.
- A monotonically increasing sequence used to order events at the same tick.
- Any generation or reset state required to reject stale scheduled work.

It does not:

- Fetch or execute runtime instructions.
- Advance a program counter.
- Borrow a CPU and call its `step` method.
- Interpret arithmetic, communication, or other domain effects.
- Know the private fields or concrete type of `HeterogeneousMachine`.
- Call back into its containing composite.

The clock is generic over its event type:

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
remove the earliest owned event
    -> advance GlobalClock.now to its tick
    -> return the owned event to HeterogeneousMachine
    -> root dispatches the selected child or completion
    -> root returns owned scheduling requests
    -> insert those requests into GlobalClock
    -> repeat
```

Skipped ticks remain meaningful positions on the timeline; they simply contain no observable work.

## Root Event Loop and Rust Ownership

`HeterogeneousMachine` owns the machine-specific event sum:

```rust
pub enum CpuEvent {
    RunNext,
    Resume(ContinuationId),
}

pub enum MachineEvent {
    CpuA(CpuEvent),
    CpuB(CpuEvent),
    Deliver(Delivery),
}
```

The concrete variants may change as communication handling becomes concrete. Vihaco core does not
define them. The reusable CPU produces only `CpuEvent`; parent routing wraps it in the variant for
the child instance that produced it.

A representative root loop is:

```rust
impl HeterogeneousMachine {
    pub fn run(&mut self) -> eyre::Result<RunOutcome> {
        self.initialize_timeline()?;

        loop {
            let Some(scheduled) = self.global_clock.pop_next()? else {
                return self.classify_empty_timeline();
            };

            let requests =
                self.dispatch_event(scheduled.at, scheduled.event)?;

            self.global_clock.extend(requests)?;
        }
    }
}
```

`pop_next` returns an owned event. The mutable borrow of `self.global_clock` therefore ends before
`dispatch_event` borrows a child or another root field. Dispatch returns owned requests, which are
inserted only after child execution and parent-level effect handling complete.

`GlobalClock` must not solve the ownership problem by receiving a closure or reference that reaches
back into `HeterogeneousMachine`. The direction of control remains root-to-clock and
root-to-child.

## Local Clocks

A local clock relates child execution to the global timeline. It is not an independent event queue
or definitive time authority. The demo begins with a fixed integer ratio:

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

Both CPUs may execute the same `add` runtime instruction through the same reusable arithmetic
component and report one local cycle while becoming eligible at different global ticks.

A local clock is an ordinary component and typed handler. It can accept route-completion
information, update its local cycle count, and produce an owned converted delay. The containing CPU
route combines that delay with child-local next work to form `Schedule<CpuEvent>`. A debug
component may handle the same completion information for tracing. Both use the same typed handler
model.

Child clocks do not advance private timelines and later reconcile them with the parent. Their
converted work is submitted directly to the common global timeline, so global event ordering
defines how child execution interleaves.

If implementation shows that a local clock is only a pure fixed-ratio multiplication helper, its
configuration may later move into `GlobalClock` without changing vihaco core. The first demo keeps
local clocks as components to exercise nested timing and effect propagation.

## Instruction Timing

Runtime instructions describe semantic operations. They do not own a clock, event queue, or
universal timing trait. The same `Add` type can have different duration in different routes or
machines.

Timing information may come from:

- A route default.
- Optional route metadata.
- Runtime instruction data.
- A component result.
- Resource state.
- Root runtime configuration.
- A completion event.

The initial demo uses route-level local duration:

```text
add -> 1 local cycle
sub -> 1 local cycle
mul -> 1 local cycle
send -> 1 local cycle
successful recv -> 1 local cycle
```

This information does not belong in the reusable arithmetic component. After the route completes,
the selected local clock translates its local duration and emits root-facing global scheduling
work:

```text
runtime instruction
    -> resolve message
    -> execute on selected component
    -> handle semantic effects
    -> apply route-local timing through LocalClock
    -> return status and owned scheduling work
```

An instruction that mutates its component and returns `Effects<NoEffect>` still receives route
timing. The global clock does not need to observe the mutation or every effect. It only receives
the information required to determine global eligibility.

A `Tick` trait implemented by every instruction is not required. If repeated timing APIs become
useful after the first implementation, they can describe route or runtime timing without coupling
semantic instruction types to one clock model.

## Scheduling Requests

Scheduling work that leaves a child step crosses the boundary as owned data:

```rust
pub struct Schedule<E> {
    pub after: GlobalDuration,
    pub event: E,
}
```

The reusable CPU returns `Schedule<CpuEvent>` and does not name its parent field or construct a
root event. `HeterogeneousMachine` maps it into `Schedule<MachineEvent>` by wrapping the event with
`MachineEvent::CpuA` or `MachineEvent::CpuB`, then submits it to `GlobalClock`. The clock converts
`after` to an absolute tick relative to its current `now`, validates the arithmetic, assigns a
deterministic sequence, and inserts the event. An alternative request may already contain an
absolute tick when that time comes from a modeled resource.

The concrete event sum is machine-specific. Vihaco core does not define CPU instance, delivery, or
resume events. It only needs an owned child-step boundary through which the configured runtime can
communicate scheduling work.

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

This is the minimal status; the actual child outcome may also contain terminal control,
continuation identity, and root-facing work.

`Complete` means the instruction and all immediate effect handling reached a child-step boundary.
If the program has another instruction, its route normally returns scheduling work based on the
local duration. If the program is exhausted, the CPU leaves the runnable set instead.

`Parked` means the resource or component atomically registered an owned continuation and the root
must not schedule the CPU's next instruction. Parking is a readiness decision, not an unknown
duration added to an otherwise complete instruction.

When a completion becomes available:

1. A library handler identifies the parked CPU and continuation.
2. The root routes the owned completion to that child.
3. The continuation applies its result.
4. The child's local clock accounts for the completion duration.
5. An owned scheduling request re-enters `GlobalClock`.

No borrow from resolution, execution, effect handling, program fetch, or clock access survives the
parked step.

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
    -> CpuA route combines the delay with CpuEvent::RunNext
    -> owned Schedule<CpuEvent> leaves CpuA
    -> HeterogeneousMachine maps it to MachineEvent::CpuA
    -> HeterogeneousMachine submits it to GlobalClock
    -> GlobalClock schedules CpuA eligibility
```

`CpuB` follows the same path but converts one local cycle to three global ticks.

A communication completion follows the inverse direction:

```text
GlobalClock releases an owned delivery event
    -> HeterogeneousMachine routes it to the communication handler
    -> handler identifies the waiting CPU
    -> root forwards the completion into the child
    -> child continuation completes recv
    -> LocalClock converts the receive duration
    -> child route emits the next Schedule<CpuEvent>
    -> HeterogeneousMachine inserts it into GlobalClock
```

The framework preserves nested route identity and ownership. Clock and communication libraries
define event contents and resource behavior; the root defines the machine-specific event dispatch.

## Demonstration Trace

The trace in [`../demos/examples/demo.md`](../demos/examples/demo.md) is the acceptance case for
the clock model:

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
- The root can coordinate executable children without a local executable instruction section.

## Ownership Boundaries

The demo assigns ownership as follows:

| Owner | State and policy |
|---|---|
| Vihaco core | Typed instructions, execution relationships, effects, route generation, child-step status, and owned nested boundaries |
| `GlobalClock<E>` library component | Current global tick, event queue, sequence allocation, checked scheduling, and reset generation |
| `LocalClock` library component | Local cycle state and local-to-global conversion policy |
| `HeterogeneousMachine` runtime root | Machine event sum, event dispatch, parent effect routing, completion routing, termination, and deadlock detection |
| CPU composite | Local architectural state, selected instruction routes, program, program counter, and parked status |
| Communication library | Values in flight, waiting continuations, acceptance, delivery, and transport timing |
| Runtime instruction | Fully resolved semantic operands |

Instructions do not own clocks, queues, wakers, or scheduler state. `GlobalClock` does not own
component semantics or instruction dispatch. The root accesses children and resources through
explicit operations rather than giving the clock access to private fields.

## Faults, Reset, and Deadlock

Clock and scheduling faults retain enough context to identify:

- The current global tick.
- The event sequence and target.
- The CPU and route when applicable.
- The requested duration or absolute tick.
- Whether the failure occurred during conversion, insertion, dispatch, or completion.

Reset invalidates pending work through a generation or equivalent identity. A completion created
before reset cannot resume a newly reset CPU that happens to reuse the same local identifier.
Reset clears and reseeds the global queue consistently with child program, cursor, local clock,
communication, and continuation state.

`HeterogeneousMachine::run` detects deadlock when:

- No runnable CPU remains.
- Every incomplete CPU is parked.
- The global event queue contains no event capable of satisfying a continuation.

Deadlock is distinct from successful program exhaustion. Waiting for an external event is deferred
until a future runtime provides a concrete external completion source.

## Implementation Sequence

Clock work should develop alongside the instruction rewrite and demo:

1. Define distinct global tick, global duration, and local cycle types.
2. Implement a deterministic generic `GlobalClock<E>` with checked time arithmetic.
3. Implement fixed-ratio local clock conversion.
4. Add owned root-facing scheduling work to child outcomes.
5. Drive one clocked CPU from a small root event loop.
6. Place two CPU instances under one global clock and verify the two ratios.
7. Add library-defined send delivery.
8. Add parked receive, owned completion, wakeup, and stale-generation protection.
9. Assert the deterministic trace from `../demos/examples/demo.md`.

Each stage should leave a focused runnable test. The concrete `GlobalClock`, `LocalClock`, root
event sum, and inherent run loop should begin as ordinary Rust. Common traits or macro shorthand
should be introduced only after these implementations expose stable repetition.

## Acceptance Criteria

The clock model is ready for the integration demo when:

- Global tick, global duration, and local cycles are distinct types.
- Host execution time never changes modeled time.
- Global time advances monotonically.
- Same-tick events execute in deterministic sequence order.
- Empty spans can be skipped without changing results.
- `GlobalClock` is generic over an owned event type.
- `GlobalClock` never calls back into its containing runtime root.
- The clock borrow ends before the root mutably steps a child.
- Child-local events acquire CPU instance identity only when the parent maps them into the root
  event sum.
- `CpuA` converts one local cycle to two global ticks.
- `CpuB` converts one local cycle to three global ticks.
- The same arithmetic instruction can have different global duration without knowing either clock.
- A completed route schedules only the next eligible work.
- A parked route schedules no next instruction.
- A communication completion resumes only its registered continuation.
- Resume timing passes through the waiting CPU's local clock.
- Program exhaustion, deadlock, and parking are distinguishable.
- Reset prevents stale scheduled work from mutating a new execution generation.
- The global clock remains an ordinary library component rather than a required vihaco core
  concept.
- The root coordinates executable children without a local executable instruction section or
  `Step` implementation.

## Deferred Questions

The first implementation does not need to decide:

- A general driver or runtime-wrapper abstraction.
- Interchangeability between timeline, sequential, real-time, and external-hardware runtimes.
- External completion polling or waiting.
- Fractional or irrational clock ratios.
- Phase offsets and clock drift.
- Multiple visibility phases within one global tick.
- Stochastic latency.
- Real-time pacing against a wall clock.
- Distributed event queues.
- Dynamic clock-tree reconfiguration.
- General cancellation of in-flight operations.

These features may extend or replace the concrete root loop after another runtime provides evidence
for the right boundary. They do not change the ownership boundaries defined by
[`execution-pipeline.md`](./execution-pipeline.md), or the integration behavior required by
[`../demos/examples/demo.md`](../demos/examples/demo.md).
