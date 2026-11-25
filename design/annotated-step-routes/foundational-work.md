# Foundational work for annotated step routes

This document tracks the runtime changes that can land before the
[annotated step routes RFC](annotated-step-topologies.md). Each item should
be usable from authored execution code and verifiable on its own before
the route framework uses it.

## Composite and receiving-field effect handling

Introduce `HandleEffect` as a public runtime trait before exposing the route
API. It gives authored steps a shared contract for composite-specific
effect handling. `BuildMessage` is the corresponding input hook. A composite
can delegate input preparation to `ReusableMessageRecipe::prepare` and
delegate delivery to the receiving field's `Apply` implementation.

```rust,ignore
pub trait HandleEffect<F, E, Context: ?Sized = ()>: Field<F> {
    type Effect;

    fn handle_effect(
        &mut self,
        effect: E,
        context: &Context,
    ) -> eyre::Result<Self::Effect>;
}
```

`F` identifies the emitting field, so the composite can handle identical
effect types differently based on their source. `E` identifies the incoming
effect, so instructions from the same field can share a handler. The
associated `Effect` is the value passed to the next link. Use `()` when
the handler consumes the effect and ends the chain.

For example, a sensor can emit a completed sample and the composite can
validate it before emitting `PushToStack<[Word; 1]>`. An authored step
calls the composite handler, then applies the stack effect to the program
field. A chain can repeat this pattern across any number of handlers.
Each link consumes the preceding output in order. An error stops the
chain and preserves mutations already made by earlier links.

### Apply effects at the receiving field

`Apply` lets a receiving component validate an effect and emit a
typed result to the next handler.

```rust,ignore
pub trait Apply<E> {
    type Effect;
    type Error;

    fn apply(&mut self, effect: E) -> Result<Self::Effect, Self::Error>;
}
```

For infallible terminal handlers such as `PushToStack`, use
`Effect = ()` and `Error = Infallible`. A parcel-records receiver can
reject a duplicate label. A waveform clamp can return a clamped waveform
for the next field. The caller or route adapter propagates an application
error through the step's `eyre::Result`.

An authored `Step` and a route use the same field-local handler, including
handlers that fail or produce another effect. The route names the destination with
`apply_effect_at::<DestinationField>()`. The node projects that field
with `Field::get_mut`, calls `Apply`, and forwards its output. This keeps
each receiving field visible in the topology and limits the handler's
mutable borrow to that field. When handling requires composite state or
source-dependent routing, the authored step uses
`HandleEffect<SourceField, E, Context>`. That hook can delegate to one or
more receivers according to the composite's policy.

[Apply contract](../../crates/vihaco-runtime/src/apply.rs) · [Route effect application](../../crates/vihaco-runtime/examples/static_graph_routes/framework/operations.rs)

## Async preparation, execution, and effect handling

Add three public runtime traits so authored execution code can suspend
while preparing an instruction's input, executing the instruction, or
handling its output. Each stage is independently selectable. For example,
an instruction can execute synchronously and hand its result to an async
effect handler.

### Component coverage

`#[component]` must check every instruction in the component's declared
dialects under the component's own generic bounds. Each instruction may
implement `Execute`, `AsyncExecute`, or both. An instruction with neither
implementation is an error at the component declaration. This check
applies to components used directly and to components used in a composite
with authored steps or routes.

`#[component]` generates one `ExecutionCapability<I>` implementation for
each declared instruction. Authors write ordinary `Execute` and
`AsyncExecute` impls. The runtime exposes a hidden helper for the macro
expansion.

```rust,ignore
pub trait ExecutionCapability<I>: Component {}

// Export these helpers under __private for downstream macro expansions.
pub struct ExecutionProbe<C, I>(std::marker::PhantomData<fn() -> (C, I)>);

impl<C, I> ExecutionProbe<C, I> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

pub trait CheckExecution {
    fn check_execution(self);
}

impl<C: Execute<I>, I> CheckExecution for &ExecutionProbe<C, I> {
    fn check_execution(self) {}
}

impl<C: AsyncExecute<I>, I> CheckExecution for &&ExecutionProbe<C, I> {
    fn check_execution(self) {}
}
```

Emit the marker impl and probe call for each leaf in the component's
resolved inventory. For a component declared as `Digitizer<T: Host>`, a
single leaf expands to the following Rust.

```rust,ignore
impl<T: Host> ExecutionCapability<acquisition::Capture> for Digitizer<T> {}

const _: () = {
    fn check_component<T: Host>() {
        use vihaco_runtime::__private::{CheckExecution, ExecutionProbe};

        (&&ExecutionProbe::<Digitizer<T>, acquisition::Capture>::new())
            .check_execution();
    }
};
```

The compiler checks the function body even when the function is unused.
Keep the component's original generic bounds and use its nominal type
in each probe call. A generic helper over an arbitrary `C` would have
too little information to resolve either method. Starting at `&&`
lets method lookup use the async impl or dereference once to use the sync
impl. When both are available, the async probe wins. This preference
only affects the check. Route or step authors still select which
execution method to call.

Generic code can then use `C: ExecutionCapability<I>` to require
availability in either mode. A call site that executes the instruction
directly still needs the bound for the method it calls.

Add compile-pass coverage for sync-only, async-only, and dual-mode leaves
in the same dialect, including a generic component in a downstream crate.
Add compile-fail cases for a missing leaf and for an implementation that
requires a bound absent from the component declaration. Neither failing
fixture needs to construct a composite or a route.
[Current declaration-time check](../../crates/vihaco-runtime-derive/src/attr_typed_component.rs)

### Driver boundary

At the driver boundary, all three stages submit an owned
`Future<Output = eyre::Result<T>> + 'static`. The first poll can report
a setup failure. A later poll can report completion or a deferred
failure through the same result. The driver can borrow the machine
between polls and deliver the result to the next stage with a fresh
borrow.

```rust,ignore
use std::future::Future;

pub trait AsyncBuildMessage<F, I, Context: ?Sized = ()>: Field<F> {
    type Message: 'static;

    fn build_message(
        &mut self,
        instruction: &I,
        context: &Context,
    ) -> impl Future<Output = eyre::Result<Self::Message>> + 'static;
}

pub trait AsyncExecute<I> {
    type Message;
    type Effect;
    type Error;

    fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> impl Future<Output = Result<Self::Effect, Self::Error>>;
}

pub trait AsyncHandleEffect<F, E, Context: ?Sized = ()>: Field<F> {
    type Effect: 'static;

    fn handle_effect(
        &mut self,
        effect: E,
        context: &Context,
    ) -> impl Future<Output = eyre::Result<Self::Effect>> + 'static;
}
```

### Borrowing across suspension

Submitted work owns what it needs while pending. Take a short borrow
of the machine, instruction, and context to prepare that work. The
submitted future captures owned values, owned handles to shared
data, or an exclusive component lease.

An inline step can borrow the fetched instruction and the program image
for its entire execution. A stage that can suspend copies or moves
only the data it needs past dispatch. For example, it can resolve a
module reference to an owned value before submitting work, or carry a
stable identifier and resolve it with a fresh borrow on resumption.
This keeps ordinary checked dispatch on the borrowed path and lets
the owner inspect the machine between polls.

Use the narrowest ownership that the operation needs.

| Operation | What the pending work owns |
| --- | --- |
| Issue a device request, then wait for its result | The work handle or an owned future. The component remains in its field. |
| Prepare a message or handle an effect asynchronously | Owned inputs extracted during a short borrow of the composite. A later stage can apply the result with a fresh borrow. |
| Run a component method that borrows `&mut self` across suspension | The owned instruction and an exclusive lease of the component. |

`AsyncSlot` is the prototype's way to implement the third case. The
adapter removes the component from the slot and places it in a lease
owned by the submitted future. The method borrows the leased component
inside that future. The field is busy while the lease is out. The lease
restores the component before the next route stage uses the field, or
when the pending future is dropped.

The slot supplies exclusive access to a single field and makes the
busy state explicit. Competing work for that field can wait in the
runtime or report that the field is busy. Work on another machine, or
on another available field when the runtime permits it, can proceed.
Moving a field into a slot is an adapter choice for borrowed async
methods. The prototype uses a thread-local slot. Backends can run
`Send` work elsewhere and deliver its result on the owner thread.

An operation that can wait using owned data or a work handle needs
no slot. For example, a digitizer can increment its issue counter
and return a sample handle synchronously. The route waits on the
handle, then stores the sample through `Apply` with a fresh borrow
of the digitizer. A slot is useful when the component's own async
method must retain its exclusive borrow.
[Device completion example](../../crates/vihaco-runtime/examples/static_graph_routes/machines/device_completion.rs) · [Component lease prototype](../../crates/vihaco-runtime/examples/static_graph_routes/framework/async_execution.rs)

For example, preparation can wait for a sample handle, execution can wait
for the sensor result, and effect handling can wait for validation before
emitting `PushToStack<[Word; 1]>`. The destination applies the stack effect only
after that last future succeeds. A failed stage stops the sequence and
preserves earlier mutations. Dropping a pending future releases its
owned resources and restores a leased field with its mutations intact.
The driver validates the invocation before using a ready result. This
keeps a completion from an earlier invocation from applying an effect
after reset or image replacement.
[Driver ownership and delivery](route-drivers.md#ownership-of-parked-routes)

The preparation contract depends on whether preparation can suspend.
Use `BuildMessage` when it completes during the initial borrow and
`AsyncBuildMessage` when it returns a future. Both can supply the
message for `Execute` or `AsyncExecute`. Generalize the existing
`BuildMessage` bound, which gets the message type from `Execute`, so
the selected execution stage checks its input type. Synchronous
preparation then needs only one hook even when execution can suspend.
[Current BuildMessage bound](../../crates/vihaco-runtime/src/build_message.rs)
