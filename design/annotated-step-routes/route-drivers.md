# Drivers for annotated step routes

A route defines typed operations and payload dependencies. A driver
delivers results to parked routes. The machine owner chooses which work
to run, and the checked runner owns instruction issue, retirement, and
debugger authorization. [Annotated step topologies](annotated-step-topologies.md)

## Prototype driver contract

```rust,ignore
trait Await<M, H> {
    type Output: 'static;

    fn wait(
        &self,
        machine: &mut M,
        handle: H,
        done: impl FnOnce(&mut M, &Self, eyre::Result<Self::Output>) + 'static,
    );
}
```

`await_work()` passes a handle from the previous node to `Await`. The
driver can call the continuation inline or retain it until the result
arrives. A parked continuation owns its captures and takes a fresh
mutable machine borrow on delivery. `FutureDriver` uses the same
continuation convention for Rust futures. The async execution node runs a
component's future while holding an exclusive lease, which returns on
completion or drop. [Example driver contract](../../crates/vihaco-runtime/examples/static_graph_routes/framework/runtime.rs)

The channel driver delivers channel results and polls woken futures. The
simulation driver also queues results by tick. The async digitizer test
exercises the same route on these drivers and on a current-thread Tokio
runtime. The warehouse uses `MinExecution<12>` to delay successful
delivery in simulation. The channel driver delivers the result
immediately. An eventual edge-latency policy would measure from producer
completion to successor delivery and could also require actual device
readiness. [Warehouse timing](../../crates/vihaco-runtime/examples/static_graph_routes/machines/warehouse/mod.rs)

## Proposed owner scheduling

A Stellarscope owner may consider instruction issue, signal or frame
delivery, drain release, and instrument events together. It needs to list
pending waits and resume one selected continuation. Each wait needs a work
ID, the owning instruction's invocation ID and stream, a typed wait
reason, and its earliest eligible time when known. The owner can match
a wait against an external bus or instrument queue and apply its own
ordering policy, including settling output first.

The driver validates the selected work and invocation IDs and removes
the saved callback before calling it. Other ready work stays available
for the owner's next decision. One invocation can register another wait
as it progresses.

## Proposed checked-runner integration

- **Issue.** Capture the originating PC and the continuation PC. If the
  route parks before retirement, call `ExecutionState::suspend` before
  returning to the runner. This keeps the visible PC at the originating
  instruction and gates further issue across runner calls.
  `ControlFlow::Break` yields while preserving the current run state.
- **Resume.** Enter the public `run` initializer under the reentrancy
  guard and call `resume_at` with the saved continuation PC. Deliver the
  selected result under that guard. Suspend again if the route parks.
- **Retire.** Apply values needed by the next instruction before release.
  An unfinished receive retires once after successful delivery. A
  barrier can retire and return a separate drain horizon that gates the
  next fetch. Releasing that gate adds no retirement.
- **Keep debugger authorization.** A displaced instruction continues
  under its original `Step` command across waits. Retiring it satisfies
  that command. A subsequent breakpoint stops separately.
- **Handle faults and reset.** Report deferred faults at the originating
  PC and retain prior effects. Invalidate outstanding invocation IDs
  on cancellation or reset so late completions cannot reach a new
  invocation or replacement program image.

Existing hooks include
[`ExecutionState::suspend` and `resume_at`](../../crates/vihaco-stdlib/src/execution_state/storage.rs),
[guarded execution](../../crates/vihaco-stdlib/src/execution_state/runner.rs),
and the [debugger runner](../../crates/vihaco-stdlib/src/debugger/runner.rs).
Library integration also needs generated coverage for sync and async
execution modes, async message preparation, and a way for deferred routes
to retain their program context. [Checked execution and debugger suspension](https://github.com/QuEraComputing/vihaco/issues/204)

## Integration cases to validate

- Select one of two ready continuations after settling earlier or tied
  output. Resume a receive displaced by a breakpoint under its original
  `Step`, then stop at the next breakpoint.
- Retire a barrier and release its drain at the horizon. Count one
  retirement and allow another stream to progress during the drain.
- Compare event order and scheduler budgets against Stellarscope
  playback. Include a deferred fault after a visible effect and a late
  completion after reset.
