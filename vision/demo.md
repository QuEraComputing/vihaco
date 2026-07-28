# Heterogeneous Two-CPU Demo

## Purpose

The vihaco integration reference is a small heterogeneous computer built from reusable parts:

- One top-level composite owns a definitive global clock.
- The composite contains two CPU composites.
- Each CPU owns a local stack, arithmetic state, a local clock, a program, and a program counter.
- Both CPUs expose `add`, `sub`, `mul`, `send`, and `recv`.
- The CPUs exchange arithmetic results through a shared communication component.
- The two local clocks map their cycles to the global clock at different rates.

This demo is the concrete forcing case for the architecture mapped in
[`contents.md`](./contents.md). Those documents define the general instruction, component,
composite, effect, and driver boundaries. This document defines a machine that must be expressible
through those boundaries without adding CPU-, channel-, or clock-specific exceptions to vihaco's
core.

This is the only end-to-end reference runtime. Smaller machines may remain as conformance fixtures
for individual instruction and driver boundaries, but they do not define a competing integration
target.

The goal is not merely to make the example run. The goal is to show that vihaco supports fast and
correct prototyping of heterogeneous machines by composing ordinary Rust types, selecting a precise
instruction set, and changing configuration rather than rewriting execution logic.

## Machine Topology

The demo runtime has an external driver and one top-level machine:

```text
Runtime
├── TimelineDriver
└── HeterogeneousMachine
    ├── GlobalClock
    ├── shared communication component
    ├── CpuA
    │   ├── program and program counter
    │   ├── operand stack
    │   ├── arithmetic unit
    │   ├── communication endpoint
    │   └── LocalClock { global_ticks_per_local_cycle: 2 }
    └── CpuB
        ├── program and program counter
        ├── operand stack
        ├── arithmetic unit
        ├── communication endpoint
        └── LocalClock { global_ticks_per_local_cycle: 3 }
```

`HeterogeneousMachine` is the single top-level composite. `CpuA` and `CpuB` are two instances of
the same reusable CPU composition unless the implementation reveals a genuine need for different
CPU types. Their instruction semantics are identical. Their clock configuration, programs, local
state, and route identities are distinct.

The global clock is part of the modeled machine, but it does not call back into its containing
composite. `TimelineDriver` remains external to the machine so it can use the machine's state
without creating a self-borrowing driver field. The driver asks the machine for its next scheduled
work, invokes the appropriate child step, and returns any resulting scheduling work to the global
clock.

This arrangement intentionally separates:

- The global clock, which owns the definitive modeled time and event ordering.
- The local clocks, which translate local cycles into global duration.
- The driver, which repeatedly selects eligible work and calls `step`.
- The CPUs, which own their local execution state.

## Framework, Library, and Demo Boundaries

The demo uses channel and clock concepts, but those concepts do not become intrinsic vihaco
semantics. Vihaco provides the composition mechanisms; reusable libraries provide particular
machine components.

| Layer | Responsibilities |
|---|---|
| Vihaco core | Surface/runtime instruction separation, `Resolve`, `Execute<I>`, generated route dispatch, typed effects and handlers, nested composite boundaries, owned step outcomes, parking, and driver integration |
| Reusable component libraries | Stacks, arithmetic units, program storage, program counters, local and global clocks, timeline scheduling, channel endpoints, and a shared channel fabric |
| Demo machine | Selects the five instructions, instantiates two CPUs, assigns clock ratios, wires communication, loads the programs, and chooses initial stack values |

`ChannelFabric` is therefore an example library component, not a vihaco-level idea. The same is true
of a particular mailbox, interconnect, clock, stack, or arithmetic implementation. Such types may
ship with the vihaco project as useful libraries, but the framework must not contain special cases
for their names or semantics.

The core requirement is more general:

- A nested composite can emit an owned effect across its parent boundary.
- The parent can route that effect to any typed handler.
- A handler can later produce an owned completion for the correct child.
- A parked child can resume from that completion.
- Driver-facing scheduling work can leave `step` without retaining borrows.

A different communication library should be usable without changing vihaco's instruction or
composite machinery.

## CPU Instruction Set

Both CPUs select the same five surface and runtime operations:

```text
add
sub
mul
send
recv
```

The CPU composite owns the machine-local routes. Merely containing an arithmetic unit, stack, local
clock, or communication endpoint does not expose every operation offered by those components.

### Arithmetic

`add`, `sub`, and `mul` use the same staged path:

```text
resolve:
    consume rhs and lhs from the CPU's local operand stack

execute:
    run the selected reusable arithmetic operation

handle:
    push the result onto the same CPU's local operand stack
    account for one local cycle
```

The arithmetic component does not know which CPU contains it, which stack supplied the values, or
how long a local cycle lasts globally. The same instruction and component implementations execute
in both CPUs.

Each arithmetic route initially costs one local cycle. Because the local clocks have different
ratios, the same semantic operation has different global duration:

```text
CpuA add: 1 local cycle × 2 global ticks = 2 global ticks
CpuB add: 1 local cycle × 3 global ticks = 3 global ticks
```

The same conversion applies to `sub` and `mul` in the first version. Later timing models may assign
different local durations per route without changing arithmetic semantics.

### Send

`send` consumes a value from the CPU's local stack and targets a communication component supplied
by a reusable library:

```text
resolve:
    consume the value from the local operand stack
    use the resolved channel identifier from the runtime instruction

execute:
    validate or prepare the send through the CPU's communication endpoint

handle:
    emit an owned transmission request across the CPU boundary
    route it through the parent to the shared communication component
    account for one local cycle
```

The surface form may contain a symbolic channel name. The machine's `Resolve` implementation turns
that name into the runtime identifier used by the communication library.

### Receive

`recv` either obtains a queued value or parks:

```text
value available:
    receive the value
    push it onto the local operand stack
    account for one local cycle
    complete

value unavailable:
    register an owned continuation
    emit an owned receive request
    return Parked
```

When a matching value arrives, the communication library produces an owned completion containing
enough identity to select the CPU and continuation. The parent routes that completion to the parked
CPU, the receive result is placed on its stack, and its local clock determines when the CPU becomes
runnable again.

No borrow from message resolution, component execution, or effect handling survives the parked
step.

## Nested Effect Flow

The demo requires nested composites to communicate with sibling resources without reaching through
their parent's fields.

For a send from `CpuA` to `CpuB`:

```text
CpuA Send route
    -> transmission effect leaves CpuA
    -> HeterogeneousMachine preserves CpuA route identity
    -> shared communication handler accepts the effect
    -> handler queues or delivers the value
    -> completion is routed to CpuB when required
```

For a parked receive:

```text
CpuA Receive route
    -> continuation is registered inside CpuA or its endpoint
    -> receive request leaves CpuA
    -> shared communication handler records the waiter
    -> CpuA returns Parked
    -> a later send satisfies the waiter
    -> owned completion is routed back to CpuA
    -> CpuA becomes eligible on the global timeline
```

This is ordinary typed effect handling at two composite levels. The framework does not need to know
that the effect represents a channel operation. It only needs to preserve route provenance,
deterministic handler order, ownership across suspension, and the distinction between internal and
driver-facing work.

The first implementation may use direct generated match arms for this propagation. A generalized
hierarchical effect API is only necessary if the concrete demo reveals repeated code that cannot be
expressed cleanly by the composite declaration.

## Timing Model

The global clock is the definitive source of modeled time. Its event queue orders work by:

```text
(global_tick, deterministic_sequence)
```

The sequence value gives stable ordering to events scheduled for the same global tick. Host
execution time never contributes to modeled duration.

Each CPU route produces or is associated with a duration in local cycles. The selected CPU's local
clock translates that duration into a global scheduling request:

```text
route completes with local duration
    -> local clock applies its configured ratio
    -> owned global scheduling request leaves the CPU
    -> global clock schedules the CPU's next eligible step
```

The timing contract must make the following cases explicit:

- A completed instruction schedules the CPU's next instruction after its converted duration.
- A parked `recv` does not schedule the next instruction.
- A delivery wakes only the matching continuation.
- Completing a parked receive incurs its configured local duration before the following instruction
  becomes eligible.
- Program exhaustion removes the CPU from the runnable set.
- Events at the same global tick use deterministic ordering.

The communication library owns its transport policy. The initial demo may use immediate delivery at
the sender's current global tick, with sequence ordering defining visibility. A later library may
add fixed, state-dependent, or topology-dependent latency without changing vihaco core.

## Driver Flow

`TimelineDriver` repeatedly coordinates the machine:

```text
read the earliest global event
    -> advance GlobalClock.now to that event
    -> identify the target CPU or completion
    -> obtain an owned runtime instruction or completion
    -> call the relevant machine step or handler
    -> return scheduling requests to GlobalClock
    -> repeat until both programs finish or the machine deadlocks
```

The driver may be a reusable library item. The core architecture only requires the one-instruction
`Step` boundary and an owned result that communicates completion, parking, terminal control, and
driver-facing scheduling work.

Each CPU needs its own program and cursor. For this demo, keeping them in the CPU makes the program
counter modeled child state and demonstrates hardware-owned progression. The driver obtains the
next owned instruction through an explicit top-level operation, allowing any borrow of child
program storage to end before the whole machine is mutably stepped.

The demo should also distinguish normal completion from deadlock. If both CPUs are parked, no
delivery can satisfy either continuation, and the global event queue is empty, the driver returns a
deadlock result rather than waiting indefinitely.

## Demonstration Program

A small deterministic exchange can exercise arithmetic, communication, suspension, and unequal
clock ratios. With the rightmost value treated as the top of each stack:

```text
CpuA initial stack: [2, 2, 3]
CpuB initial stack: [10, 4]
```

The conceptual SST programs are:

```text
CpuA:
    add
    send to_b
    recv from_b
    mul

CpuB:
    sub
    recv from_a
    mul
    send to_a
```

The expected value flow is:

```text
CpuA: 2 + 3 = 5
CpuA sends 5 to CpuB
CpuB: 10 - 4 = 6
CpuB receives 5
CpuB: 6 × 5 = 30
CpuB sends 30 to CpuA
CpuA receives 30
CpuA: 2 × 30 = 60
```

With every instruction costing one local cycle and immediate communication delivery, one expected
global trace is:

```text
global 0: CpuA add; next eligible at 2
global 0: CpuB sub; next eligible at 3
global 2: CpuA send 5; CpuA next eligible at 4
global 3: CpuB recv 5; CpuB next eligible at 6
global 4: CpuA recv parks
global 6: CpuB mul -> 30; CpuB next eligible at 9
global 9: CpuB send 30; CpuA receive is satisfied
global 11: CpuA becomes eligible and mul -> 60
```

The exact trace depends on the selected communication timing contract, but the contract and expected
trace must be fixed before the end-to-end test is written. The final observable result for this
configuration is `60` on `CpuA`'s stack, with both programs completed and no parked continuation
left behind.

## Requirements on the Instruction Rewrite

The architecture mapped in [`contents.md`](./contents.md) must provide or prove the following
surface area for the demo:

1. A CPU composite can select only `add`, `sub`, `mul`, `send`, and `recv` from larger reusable
   component catalogs.
2. Two instances of the same CPU composite retain distinct machine-local route identities.
3. The top-level composite can address and step either child without exposing all descendant
   instructions accidentally.
4. A nested route can propagate an owned effect to its parent, and the parent can route it to a
   library-defined handler.
5. A parent can route an owned completion back to the correct child independently of that child's
   next program instruction.
6. One effect can reach multiple handlers deterministically, such as a local clock and a diagnostic
   trace handler.
7. A step outcome can carry owned driver-facing scheduling work.
8. Parking registers an owned continuation and prevents the driver from scheduling the next
   instruction prematurely.
9. Programs and program counters can live in each CPU while the external driver safely obtains an
   owned instruction for dispatch.
10. Surface channel names resolve to library-defined runtime identifiers before execution.
11. Generated code preserves typed faults and reports the CPU, route, instruction, global tick, and
    failed pipeline stage.

These requirements constrain the general architecture without making the demo's communication or
clock types part of vihaco core.

## Reusable Library Deliverables

The demo should be assembled from reusable items rather than defining all behavior inside the
example:

- A stack component with invariant-preserving operations.
- Arithmetic runtime instructions and an arithmetic component implementing `add`, `sub`, and
  `mul`.
- Surface instruction types and resolution support for those arithmetic operations.
- A local clock component with a configurable local-cycle-to-global-tick ratio.
- A global clock or event-queue component with deterministic ordering.
- A communication endpoint and shared communication component supplied by a library.
- Surface and runtime `send` and `recv` instructions supplied by that communication library.
- Owned send, receive, delivery, and wakeup effects.
- Program and program-counter components suitable for a child CPU.
- A timeline driver suitable for more than this one machine.

The final crate and module organization can be decided during implementation. The architectural
requirement is that none of these reusable components relies on the private fields or concrete type
of `HeterogeneousMachine`.

## Implementation Sequence

The demo should grow alongside the instruction rewrite:

1. Build one CPU from a stack and reusable arithmetic unit; execute `add`, `sub`, and `mul` through
   generated routes.
2. Instantiate the CPU twice in a parent composite and prove that route identity distinguishes the
   two instances.
3. Add local clocks, the global clock, and a timeline driver; verify the two clock ratios with only
   arithmetic instructions.
4. Add a library-provided communication component and complete non-parking `send`.
5. Add `recv`, owned continuation registration, parking, delivery, and wakeup.
6. Parse both SST programs, resolve channel names, and load the resulting runtime programs into the
   two CPUs.
7. Record and assert the deterministic global trace.
8. Document how to replace the clock or communication library without changing vihaco core.

Each stage should leave a runnable test. Macro ergonomics can improve after the manual relationships
are proven, but the final demo must use the public composition surface intended for downstream
users.

## Acceptance Criteria

The demo is complete when:

- One top-level composite contains the global clock and two CPU composites.
- Both CPUs use the same reusable component and instruction implementations.
- The top-level composite exposes only the intended child operations.
- `CpuA` maps one local cycle to two global ticks.
- `CpuB` maps one local cycle to three global ticks.
- Global time is monotonic and same-tick ordering is deterministic.
- Arithmetic touches only each CPU's local stack.
- Values cross CPUs only through typed effects and library-defined communication handlers.
- `recv` parks when no value is available and resumes without retaining a borrow.
- A parked CPU does not execute its next instruction.
- Both SST programs lower entirely to the selected runtime instruction sums.
- The expected trace is reproducible.
- `CpuA` finishes with `60` on its stack.
- Both programs terminate with no lost value, stale continuation, or pending event.
- Replacing the communication component does not require a change to vihaco core.
- Building `CpuB` from `CpuA` requires configuration and wiring changes rather than copied execution
  implementations.

The last criterion is central to the demonstration. Heterogeneity should arise from composition,
configuration, timing, and program choice while reusable semantic components remain unchanged.

## Non-Goals

The first demo does not need:

- A general network-on-chip model.
- Dynamic CPU discovery.
- Multiple host threads.
- Wall-clock synchronization.
- Nondeterministic or stochastic timing.
- Backpressure beyond what is required to demonstrate a parked receive.
- A complete debugger or visualization frontend.
- Performance representative of physical hardware.

Those capabilities may be layered onto the same boundaries later. They are not prerequisites for
showing that vihaco can prototype a heterogeneous machine correctly.
