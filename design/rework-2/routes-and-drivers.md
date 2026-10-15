# Typed routes and profile-specific drivers

Status: design direction agreed in discussion on 2026-10-07. API examples are
illustrative; the traits and builder extensions described here are not yet
implemented contracts.

This document develops the routing and driver model introduced in
[Composite instruction sets, field markers, and routes](composite-dispatch-and-step.md).
It refines that document's eager route execution and named continuation
examples. The remaining builder composition can itself be the continuation;
users do not need to name a continuation just to reach the next route stage.

## Responsibilities

The composite defines the logical machine: device fields, their identities,
and the complete available instruction set. Generated dispatch selects the
user's `Step<FieldMarker, Instruction>` behavior from the flat Rust instruction
enum. Bytecode identity remains the nested device-code chain, dialect ID, and
instruction variant.

Step implementations use routes to describe logical execution. Routes express
operations, dependencies, and subsequent computation.
The selected profile supplies a driver and binds logical destinations to
execution endpoints. The driver implements submission, waiting, scheduling,
and resumption.

A route does not contain an event queue, channel sender, OS thread handle, or
profile-specific clock. The same route can run with an emulation driver or a
real-time driver.

## Ordinary typed composition

The route builder uses ordinary generic Rust types. Each builder operation
composes another node into the route. Synchronous nodes call subsequent nodes
directly. A driver-handled boundary passes its operation and the remaining
composition to the driver.

No route-compilation macro is required. Here, “compiled” means the specialized
Rust code produced from typed composition through monomorphization and
optimization. Composite, field-marker, and dispatch generation remain separate
macro responsibilities.

There are two related values:

- A route definition describes the typed sequence of stages.
- A route invocation owns its input and any intermediate values required as
  execution proceeds.

The experiment's synchronous `Rest::run(...)` composition is a starting point.
Its boundary nodes must be changed to hand control to a driver rather than
eagerly executing the entire remaining route.

## Explicit driver boundaries

Potentially remote device execution is a boundary. Ordinary local computation
can run synchronously before it. At that boundary, the route hands a typed
request and continuation to a statically selected driver handler.

`BuildMessage` is an input to `ExecuteAt` only. `ExecuteAt` uses it to obtain
the message passed to `Execute`. In the examples, `build_message_for::<I>()`
configures that instruction-execution node; it is not an independent route
stage and does not apply to waits, effect propagation, or continuations.

An illustrative driver contract is:

```rust,ignore
trait Driver<M> {
    fn submit<R, K>(
        &mut self,
        request: R,
        continuation: K,
    ) -> eyre::Result<()>
    where
        R: Request<M>,
        K: Continuation<M, R::Completion>;
}
```

The full contract also needs the originating execution identity, submission
and completion semantics, and ownership bounds. This sketch shows how the
driver receives a boundary without inspecting the builder's internal types.

Conceptually, a boundary node executes:

```rust,ignore
let request = make_execution_request(instruction, execute_at);
driver.submit(request, rest)
```

Here `rest` is the owned remaining route. When completion arrives, the driver
resumes it with the typed completion result:

```rust,ignore
rest.resume(machine, driver, completion)
```

A route's `then_execute()` stage must not directly call a locally borrowed
device when the profile controls that device's placement. It describes the
execution request handled by the driver. The builder must distinguish such
execution from ordinary synchronous local computation.

`await_completion()` specifies that the remainder depends on completion.
It does not itself park a thread. The pair
`then_execute().await_completion()` describes one driver-handled operation
with a completion-dependent continuation, not two queue round trips.

Submission-dependent continuation is a separate logical behavior. Its precise
builder spelling and the guarantee represented by “accepted” remain open.

## A route with multiple boundaries

This route executes a playback instruction, uses its completion to construct
an acquisition request, then awaits acquisition work and stores its result:

```rust,ignore
route::<Machine>()
    .on::<field!(Machine::pulse)>()
    .build_message_for::<Play>()
    .then_execute()
    .await_completion()                    // Boundary 1
    .then(|machine, played: PlayFinished| {
        let settings = machine.acquisition_settings();

        Ok(AcquireRequest {
            start: played.finished_at,
            duration: settings.duration,
        })
    })
    .await_on::<AcquireWork>()              // Boundary 2
    .then(|machine, acquired: AcquireFinished| {
        machine.cpu.store_measurement(acquired.value)?;
        Ok(())
    })
    .finish()
    .run(machine, driver, play_instruction)
```

The experiment's builder needs extensions for these chained stages; this is
the intended user-facing shape rather than currently supported syntax.

`AcquireWork` describes acquisition work and its logical destination. Its input
is the `AcquireRequest` produced by the preceding stage, and its completion is
`AcquireFinished`. This boundary does not execute an instruction through
`ExecuteAt`, so it has no `BuildMessage` input. Only playback instruction
execution uses `build_message_for` in this example.

The internal composition is conceptually:

```text
PlaybackBoundary<
    BuildAcquisition<
        AcquisitionBoundary<
            StoreMeasurement<Finish>
        >
    >
>
```

At boundary 1, the driver receives the playback request and this remainder:

```text
build AcquireRequest from PlayFinished
→ await acquisition work
→ store measurement
→ finish
```

After playback completes, the driver resumes that remainder. Acquisition
request construction executes synchronously. At boundary 2,
the driver receives the acquisition request and the smaller remainder:

```text
store measurement from AcquireFinished
→ finish
```

After acquisition completes, the final segment stores the measurement and
finishes the route. The originating logical execution can then advance.

Each boundary moves the remaining composition into the driver. Completed
stages need not be retained. Intermediate values and closure captures that
survive suspension must be owned; a continuation must not retain a mutable
borrow of the machine.

## Profile-specific runtime behavior

A logical execution has an identity independent of an OS thread. The runtime
tracks whether it is ready, waiting, or finished. A suspended route belongs to
that logical execution even when different endpoints run its operations.

| Logical action | Emulation driver | Real-time driver |
| --- | --- | --- |
| Submit device work | Execute or schedule a simulated operation | Send the request to its device endpoint |
| Await completion | Record a waiting execution and its continuation | Retain a continuation pending endpoint completion |
| Produce completion | Retrieve a scheduled event or satisfy a registered wait | Receive a worker or hardware completion |
| Resume | Run the remaining route with the completion result | Run the same remaining route with the completion result |

An emulation driver owns its event queue and waiting tables. A simulated
operation with a known completion time can schedule an event. A wait for a
signal can remain registered until another operation produces that signal;
it need not immediately create a timed event.

A real-time driver owns its endpoint bindings and outstanding requests. It can
use an async runtime to suspend a task while awaiting completion, leaving
worker threads available. A profile using dedicated blocking threads can
implement a thread wait instead. These choices do not change the route.

The driver registers waiting state before exposing a request to an endpoint
that may complete immediately. A completion token can associate the response
with the saved continuation and originating execution. Continuations can stay
with the coordinating driver while only requests and tokens cross worker
channels.

Device completion, route completion, and permission to advance the instruction
stream are distinct events. A route may encounter further boundaries after a
device completes. Returning control to the driver after submission does not
mean the route has finished.

## Continuation storage

Typed composition provides concrete continuation types, but a heterogeneous
event queue still needs a common storage representation.

| Approach | Tradeoff |
| --- | --- |
| Driver-specific boxed continuations | Minimal enumeration boilerplate; allocation and indirect calls at deferred boundaries |
| Explicit continuation enums | Concrete storage and matching without mandatory boxing; variants must be enumerated |
| A small enum-generation macro | Reduces storage boilerplate while leaving route construction in ordinary Rust |

The storage choice remains open. The builder alone cannot automatically
enumerate arbitrary continuation types used elsewhere in the program.
Thread-transfer bounds apply to values actually crossing threads; locally
stored continuations need not inherit those bounds merely because the profile
uses workers.

## Cost model

Synchronous stretches execute as direct Rust calls. The design does not
require per-stage runtime enum matching, a route cursor, or a dynamic
interpreter. Field markers and static builder structure can disappear under
optimization.

At deferred boundaries, the driver must retain continuation state and perform
the scheduling or communication required by the profile. Heterogeneous
continuation storage may add an enum match or a boxed call at those boundaries.
Immediate completion may allow direct resumption where the route's ordering
semantics permit it.

The target is no additional cost over equivalent handwritten scheduling,
with scheduling machinery confined to explicit boundaries. This is a design
target, not a verified zero-overhead claim. Compare optimized output and
benchmarks against handwritten equivalents before making that claim.

## Details still to settle

- The Step signature and how it receives a generic driver or execution context.
- The exact builder state transitions, including how execution boundaries
  differ from synchronous local stages.
- Whether profile endpoints own whole components or only their backends, and
  where resumed synchronous segments run.
- Typed request, completion, continuation, and endpoint contracts, including
  failure and cancellation handling.
- Execution identity propagation and readiness transitions across suspension.
- Submission-versus-completion semantics and logical-time guarantees.
- Heterogeneous continuation storage and the minimal required macro support.

The core direction is ordinary typed route composition, explicit driver
boundaries, the remaining route as an owned continuation, and profile-specific
submission and resumption.
