# Scheduled device work and simulator turns

**In progress.** This note explores extensions to the route-driver
RFC. The interfaces and scheduling rules below are proposals to validate
in a prototype. [Route drivers](route-drivers.md)

## Work can outlive an instruction

A route describes the operations and dependencies of one instruction.
It can produce an owned request for future device work. A waveform play
instruction can produce a claim for particular channels and a duration.
A composite effect handler can submit that claim through a typed service
in the driver context. The host configures the backend's timing and
resource policy. A named route edge can identify a specific handoff for
that configuration. [Backend delivery policy](route-drivers.md#delivery-policy-at-the-backend)

Submission, completion, and retirement give the host three distinct
events to track.

| Term | Meaning |
| --- | --- |
| **Work** | An owned request, such as a future, a device command, or a request to advance an active device operation. |
| **Submission** | Asking the backend to accept work. A successful submission returns a ticket and confirms acceptance. |
| **Ticket** | The driver's handle to accepted work, identified by a task ID. It can carry progress and a terminal outcome. |
| **Progress** | An intermediate event from active work, such as the effects of a playback cycle. |
| **Completion** | The accepted work's terminal success, failure, or cancellation. |
| **Retirement** | The point at which the program driver records successful instruction completion. |
| **Turn** | One action selected by the host, such as issuing an instruction, delivering a result, or advancing a device. |

For queued playback, the route finishes when the backend accepts the
claim. The instruction retires and the host continues to schedule the
accepted work. For blocking playback, the route keeps the ticket and
waits for completion before retiring. If admission itself can wait,
the instruction stays in flight until the backend accepts or declines
the request.

The host owns queued device work through completion or cancellation.
Dropping the submitting route's ticket leaves accepted device work
scheduled. A device failure after retirement is reported as a device
event. A host can continue draining device work after the program halts
and decide when the run as a whole has finished.

## One device advance per turn

For cycle-based playback, the device defines an advance. A scheduled
turn can mean that a particular playback session is ready to advance one
cycle. On readiness the driver borrows the machine, calls the device,
and handles the resulting effects in order. For example, a frame may
update an optics model before a camera reads the updated positions.
The driver requests another cycle after handling those effects and
returns control to the host. On the final cycle it handles the final
effects before completing the operation.

A route runs until it finishes, waits on a dependency, reaches a handoff
the backend defers, or yields a progress turn. The device's protocol
defines the boundaries between playback cycles. The backend determines
when each turn is eligible. The same operation could receive scheduled
cycle notifications in a simulator and readiness notifications from a
physical device.

An illustrative extension to the existing submission vocabulary is a
ticket that can carry progress before its terminal outcome.

```rust,ignore
pub trait Work {
    type Progress: 'static;
    type Output: 'static;
}

pub enum WorkEvent<P, T> {
    Progress(P),
    Finished(eyre::Result<T>),
    Cancelled,
}

pub trait Ticket<W: Work> {
    fn id(&self) -> TaskId;
    fn try_take_event(&mut self) -> Option<WorkEvent<W::Progress, W::Output>>;
}

pub trait Submit<W: Work> {
    type Ticket: Ticket<W>;

    fn submit(&self, work: W) -> eyre::Result<Self::Ticket>;
}
```

A one-shot future can use `Infallible` for `Progress`. A playback
adapter can request one cycle at a time, with the next cycle becoming
eligible after the current cycle's effects are handled. This gives the
host a chance to act between cycles. For work attached to a parked
instruction, the driver associates events with the run and instruction
identities. A fault or reset closes that continuation to further effects.
Accepted device work that outlives retirement needs its own owner and
cancellation scope so reset can still find it.
[Ownership of parked routes](route-drivers.md#ownership-of-parked-routes)

## Selecting the next simulator turn

A simulator can queue accepted device work as soon as its earliest
eligible time is known. If a claim is accepted at tick 10 and its first
sample is due at tick 20, the simulator can queue that sample during the
submission turn. If the instruction retires at acceptance, its stream
can issue another instruction while the sample stays queued.

A runnable stream contributes a fetch candidate at its own next issue
time. The simulator can pick the earliest action across runnable
streams, ready input deliveries, and scheduled device events, then
advance directly to that time. A fetch and a device advance can share a
timestamp. The host's ordering policy determines which turn runs first.
When it selects a fetch, the driver checks that the stream can issue and
reads the current PC. After that turn, the driver outcome tells the host
whether the stream can issue again. The backend determines when it is
next eligible.

Computing fetch candidates from current stream state is a simple option
for a small number of streams. A priority queue can also hold one next
fetch per runnable stream. The queued version needs to invalidate or
recheck entries when a stream resets, stops at a breakpoint, or becomes
gated. Both implementations can share the same eligibility and
validation rules.

## Drain gates and a prototype test

A barrier can capture a drain point for the selected device channels,
retire, and leave a fetch gate. The host clears the gate once the device
reaches that point. Gate release makes the next instruction eligible
and adds no retirement. The driver outcome should therefore report
retirement separately from permission to fetch.
[Program driver operations](route-drivers.md#program-driver-operations)

Start with a program that queues playback, executes another instruction
while samples are still being delivered, and then crosses a drain
barrier. Check that `Play` retires at acceptance, the host observes
device progress in order, and the stream fetches past the barrier only
after drain. Run a blocking variant that issues `Play` once and retires
after the final cycle. These two traces can inform the event API, the
cancellation scope for accepted work, and how a backend exposes an
action's eligibility time.
