# Counter machine

The counter-machine example demonstrates a clock-driven composite that plays
multiple channels concurrently. `CounterGroup` is the channel manager:

- `Queue { start, duration }` adds a counter to the pending queue.
- `Play` moves all pending counters into the active set.
- Each active counter advances once per global clock tick.
- A `PlayReport` describes what every active counter emitted on that tick.

The machine itself does not own counter state. It owns the `GlobalClock`, the
runtime program counter, and the event loop that decides when the group is
sampled. This is the same boundary intended for a future waveform or FPGA
component: the component evaluates its channels, while the machine supplies
the timeline.

## Execution flow

The example program queues two counters and then starts playback:

```text
Queue(start = 10,  duration = 2)
Queue(start = 100, duration = 4)
Play
```

The `Play` instruction starts both counters at the same global time. The
machine schedules the first `AdvanceCounters` event one tick later. Each
advancement produces one report containing all channels that are still active:

```text
tick 1: Counter 0 -> Advanced(11),  Counter 1 -> Advanced(101)
tick 2: Counter 0 -> Done(12),      Counter 1 -> Advanced(102)
tick 3:                         Counter 1 -> Advanced(103)
tick 4:                         Counter 1 -> Done(104)
```

Playback ends when there are no active counters. Counters with a zero duration
are discarded when `Play` starts.

## Ownership and event loop

The responsibilities are intentionally separate:

```text
CounterMachine
├── GlobalClock<MachineEvent>
├── runtime program and program counter
└── CounterGroup
    ├── queued counters
    └── active counters
```

`Step` executes one runtime instruction at the current tick. A `Play` step
starts queued channels and schedules `AdvanceCounters` if playback is active.
An `AdvanceCounters` event asks `CounterGroup` for its next `PlayReport`, sends
that report to `DebugTrace` with the current global tick, and schedules the
next tick while any channel remains active.

The machine guards against duplicate advance events when multiple `Play`
instructions occur at the same time. A later `Play` can add newly queued
channels to an already-running group without interrupting existing channels.

## Future sample rates

The current implementation samples once per global tick to keep the example
small. `CounterGroup` does not depend on the clock, so a future component or
timing trait can choose a different sample period. The machine would then use
that period when scheduling the next `AdvanceCounters` event, while the
component would continue to own channel evaluation and report construction.

Run the example with:

```bash
cargo run -p vihaco-demos --example counter-machine
```
