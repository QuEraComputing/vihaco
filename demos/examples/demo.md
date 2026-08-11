# Heterogeneous Two-CPU Demo

## Purpose

The executable example in [`demo.rs`](./demo.rs) is the
concrete integration target for the current vision. It composes ordinary Rust types into a
clock-driven machine with two instances of the same reusable `Cpu`:

- `CpuA` starts with a value on its stack, waits for a value from `CpuB`, then multiplies.
- `CpuB` subtracts and multiplies local operands, then sends its result to `CpuA`.
- `CpuA` runs at one local cycle per three global ticks; `CpuB` runs at one local cycle per global
  tick.
- A shared in-memory `ChannelFabric<i64>` supplies the two directed channels.
- A concrete `HeterogeneousMachine` owns the root event loop and
  `GlobalClock<MachineEvent>`.

This is a working reference for the instruction, component, route, effect, suspension, and
runtime boundaries. It is intentionally implemented with explicit Rust wiring so those boundaries
remain visible while the corresponding macro surface is developed.

## Source organization

The example is one Cargo example file assembled with `include!`:

```text
demos/examples/demo.rs
├── demo/vihaco/       framework contracts and route plumbing
├── demo/stdlib/       stack, arithmetic, channel, clock, and tracing components
└── demo/src/          surface resolution, reusable Cpu, root machine, and test driver
```

The files under `demo/vihaco/` define the small contracts used by the example: `Execute<I>`,
`Resume<C>`, `Supply<M>`, `Absorb<E>`, `Observe<E, R>`, `Handle<E, R>`, and `Route`. The
`machine_macro.rs` file currently documents the intended effect-fanout expansion; it does not
define a macro used by the executable.

## Machine topology

```text
HeterogeneousMachine
├── GlobalClock<MachineEvent>
├── HashMap<CpuId, GlobalTicksPerLocalCycle>
├── SharedTransport<i64>
├── Cpu A
│   ├── Stack
│   ├── ArithmeticUnit
│   ├── ChannelEndpoint<i64, SharedTransport<i64>>  endpoint 0
│   ├── DebugTrace
│   └── SST-loaded program and pc
└── Cpu B
    ├── Stack
    ├── ArithmeticUnit
    ├── ChannelEndpoint<i64, SharedTransport<i64>>  endpoint 1
    ├── DebugTrace
    └── program and pc
```

`Cpu` is reusable and does not know whether it is `CpuA` or `CpuB`. The root adds that instance
identity when it maps a child `CpuEvent::RunNext` or a receive wakeup into `MachineEvent`:

```rust
enum MachineEvent {
    Step(CpuId),
    Resume {
        id: CpuId,
        continuation: ReceiveContinuation,
        value: i64,
    },
}
```

The root has no executable instruction section of its own. It seeds both CPUs, pops the earliest
event, dispatches it, submits any returned schedule, and drains transport wakeups. When the event
queue is empty it reports `Completed` unless a CPU remains parked, in which case it reports
`Deadlock`.

The root also owns a `HashMap<CpuId, GlobalTicksPerLocalCycle>`. The reusable `Cpu` does not store
its timing ratio; the root looks up the ratio for the selected instance and passes that value into
`step_at`, `resume`, and `next_boundary_at`. This keeps timing instance-specific without making it
part of the reusable CPU's state.

## SST and runtime programs

Arithmetic and channel components own their local syntax. The composite mounts those syntax sets
under the `arithmetic` and `channel` namespaces, then resolves the parsed component instructions
into runtime route products. Channel names become `ChannelId` values:

```text
to_b | from_a  -> ChannelId(0)  // A to B
to_a | from_b  -> ChannelId(1)  // B to A
```

The concrete programs are deliberately small:

```text
CpuA: recv from_b; mul
CpuB: sub; mul; send to_a
```

They are constructed in `main` as:

```rust
resolve_program(&[Recv("from_b"), Mul]);
resolve_program(&[Sub, Mul, Send("to_a")]);
```

Both CPU programs are now loaded from SST sections through the generated composite loader. Header
resolution, component syntax parsing, lowering, program installation, and debug-section forwarding
all happen before the event loop starts.

## Components and routes

`Stack` is a reusable `i64` operand stack. It supplies messages by popping values and absorbs
arithmetic results by pushing them. `ArithmeticUnit` is stateless and implements `Execute<Add>`,
`Execute<Sub>`, and `Execute<Mul>` with `BinaryOperands -> ValueResult`:

```text
route message: Stack supplies rhs, then lhs
component:     ArithmeticUnit computes wrapping add/sub/mul
route effect:   ValueResult(i64)
handler:       Stack absorbs and pushes the result
observer:      DebugTrace records the effect
```

The three arithmetic routes share the `ValueResult` effect but have distinct route markers
(`routes::IntegerAdd`, `IntegerSub`, and `IntegerMul`). The route marker disambiguates generated
message, effect, observer, handler, and fault wiring.

`ChannelEndpoint` implements `Execute<Send>` and `Execute<Recv>`:

- `Send` receives an `i64` from the stack and immediately queues it in the shared fabric. It
  produces an empty `SendEffect` and completes.
- `Recv` requires `NoMessage`. If a value is queued, it emits `ReceiveEffect::Received(value)`;
  otherwise it stores an owned `ReceiveContinuation`, emits `ReceiveEffect::Parked`, and returns
  `Execution::Parked`.
- `resume` consumes `ReceiveCompletion<i64>`, clears the endpoint's parked state, emits the
  received value, and completes the suspended receive.

The current `ChannelFabric` has FIFO queues and one waiter slot per channel. Its `send` operation
also moves a matching waiter into a wakeup queue. The root drains that queue and schedules the
receiver at its next local clock boundary.

## Timing and root execution

Every runtime instruction implements `TimedInstruction`; all five operations cost one local cycle.
The root converts local cycles using the selected CPU's `GlobalTicksPerLocalCycle` value. A CPU's
`next_boundary_at` rounds a global tick up to its next local boundary before adding the instruction
duration.

`GlobalClock<E>` is a reusable event queue. It orders events by `(GlobalTick, sequence)` using a
`BinaryHeap`, so equal-time events are deterministic. It owns modeled time and never calls back
into the root or fetches instructions.

The concrete root loop is:

```text
seed Step(A) and Step(B) at global tick 0
while the clock has an event:
    pop the earliest MachineEvent
    Step: fetch an owned instruction and call Cpu::step_at
    Resume: deliver an owned completion through Cpu::resume
    submit the child's next RunNext schedule
    drain channel wakeups into root Resume events
if the queue is empty:
    parked CPU -> Deadlock
    otherwise   -> Completed
```

On a completed instruction, `Cpu` advances its program counter and schedules its next instruction
after the converted duration. A parked receive does not advance the program counter or schedule
the next instruction until its completion is delivered.

## Actual execution trace

`main` initializes the stacks as follows. The rightmost value is the top, so `CpuB`'s first
subtraction consumes `2` and `4`, producing `2`:

```text
CpuA: [3]
CpuB: [10, 4, 2]
```

The deterministic trace asserted by `src/driver.rs` is:

```text
global  0: CpuA recv parks on ChannelId(1)
global  0: CpuB Sub
global  1: CpuB Mul
global  2: CpuB send on ChannelId(1)
global  3: CpuA wakes, recv 20
global  6: CpuA Mul
```

The value flow is:

```text
CpuB: 4 - 2 = 2
CpuB: 10 × 2 = 20
CpuB sends 20 on ChannelId(1)
CpuA receives 20
CpuA: 3 × 20 = 60
```

The send at global tick 2 wakes `CpuA`, but `CpuA` can resume only at its next local boundary,
global tick 3. The resumed receive completes there; its following multiplication becomes eligible
at global tick 6 because `CpuA` uses three global ticks per local cycle.

The example asserts `RunOutcome::Completed`, a final `CpuA` stack top of `60`, empty parked state
on both endpoints, and both CPUs finished. It also asserts that the trace has exactly the six
entries above.

## What this demo proves

The concrete example currently demonstrates:

- one reusable CPU instantiated twice with distinct root identities;
- explicit selection of five runtime routes from reusable components;
- surface channel-name resolution before execution;
- typed stack message and effect boundaries;
- route-specific effect observation and handling;
- owned receive continuations and `Complete`/`Parked` step outcomes;
- child-local scheduling mapped into a root-owned event sum;
- deterministic global timing with unequal local clock ratios;
- a scalar-only machine using `i64`, with no framework-wide `Value` or `Type` enum; and
- completion and deadlock as distinct root outcomes.

The example does not yet exercise a generated parser, module loading, bytecode encoding, a general
driver abstraction, or a macro invocation. Those remain architecture work rather than behavior
provided by this concrete demo.

## Running and testing

From the repository root, run the example and its tests with Cargo:

```bash
cargo run --example demo
cargo test --example demo
```

The executable prints the global trace, final outcome, both stacks, and each CPU's debug records,
then checks the expected completed exchange.
