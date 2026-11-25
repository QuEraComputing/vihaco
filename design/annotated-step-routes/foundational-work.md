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
pub trait HandleEffect<Source, Destination, E, Context: ?Sized = ()>:
    Field<Source> + Field<Destination>
{
    type Effect;

    fn handle_effect(
        &mut self,
        effect: E,
        context: &Context,
    ) -> eyre::Result<Self::Effect>;
}
```

The composite implements the hook for a particular source, destination,
and incoming effect type. For example, a sensor can emit a completed
sample. The composite can validate it for the program field and return
`PushToStack<[Word; 1]>`, or transform the same sample for a display
field using a separate impl. Both fields resolve through `Field`.
The source and destination may be the same field.

Choose one handler for each effect handoff. A field-local node handles
delivery through `Apply` or `AsyncApply`. A composite node handles the
handoff through `HandleEffect` or `AsyncHandleEffect` and can delegate
to the receiving field inside that hook. This is the same choice a
route makes between a message recipe and a composite builder.

If a handler emits a new effect, that effect can have its own handoff.
Each link consumes the previous output and chooses its own handler.
Use `()` when the hook consumes the effect and ends the chain. An
error stops the chain and preserves prior mutations.

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
mutable borrow to that field. When handling depends on the composite,
the authored step uses `HandleEffect<SourceField, DestinationField, E, Context>`.
The `handle_effect_at::<DestinationField>()` node infers the source from
the upstream field operation and calls the composite hook. If that
hook delegates to `Apply`, the delegation stays inside the impl.
`BuildMessage` selects its hook using the instruction destination.

[Apply contract](../../crates/vihaco-runtime/src/apply.rs) · [Route effect application](../../crates/vihaco-runtime/examples/static_graph_routes/framework/operations.rs)

## Async preparation, execution, and effect handling

The RFC proposes public traits for suspending during message preparation,
instruction execution, and effect handling. Each stage is independently
selectable. A synchronous instruction can hand its result to an async effect
handler. This branch implements `AsyncExecute` in the runtime and
prototypes the other async hooks in the route examples.
`async_prepare_message_at` calls the message type's reusable recipe.
`async_build_message_at` calls the composite's builder.
`async_apply_effect_at` calls `AsyncApply` at the receiving field.
`async_handle_effect_at` calls `AsyncHandleEffect` on the composite
using the inferred source and the named destination. Each submits owned work through the
same driver boundary.
[Async effect node](../../crates/vihaco-runtime/examples/static_graph_routes/framework/async_execution.rs) · [Validation example](../../crates/vihaco-runtime/examples/static_graph_routes/machines/device_completion.rs)

### Component coverage

`#[component]` must check every instruction in the component's declared
dialects under the component's own generic bounds. Each instruction may
implement `Execute`, `AsyncExecute`, or both. An instruction with neither
implementation is an error at the component declaration. This check
applies to components used directly and to components used in a composite
with authored steps or routes.

Authors write ordinary `Execute` and `AsyncExecute` impls. The macro
checks the leaves and emits the existing dialect membership witness.
A blanket implementation derives `ExecutionCapability<I>` from that
membership and the generated instruction's dialect identity.

```rust,ignore
// Export these helpers under __private for downstream macro expansions.
pub struct ExecutionProbe<C, I>(pub std::marker::PhantomData<fn() -> (C, I)>);

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

Emit a probe call for each leaf in the resolved inventory. For
`Digitizer<T: Host>`, a single-leaf dialect expands to the following Rust.

```rust,ignore
impl<T: Host> Supports<acquisition::__VihacoDialect> for Digitizer<T> {}

const _: () = {
    fn check_component<T: Host>() {
        use vihaco_runtime::__private::{CheckExecution, ExecutionProbe};

        (&&ExecutionProbe::<Digitizer<T>, acquisition::Capture>(std::marker::PhantomData))
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
availability in either mode. The blanket impl avoids overlapping
per-leaf impls over associated type projections from upstream dialects.
A call site that executes the instruction
directly still needs the bound for the method it calls.

The branch prototype checks these cases across crate boundaries. The
compile-fail cases cover a missing leaf and implementations requiring
bounds absent from the component declaration. Neither failing fixture
constructs a composite or a route. Generated dispatch requires the chosen
`Step` for each leaf. The step can call either execution mode.
[Declaration-time check](../../crates/vihaco-runtime-derive/src/inventory.rs) · [Cross-crate tests](../../crates/vihaco-runtime/tests/fixtures/dialects/consumer/src/lib.rs)

### Driver boundary

At the driver boundary, all three stages submit an owned
`Future<Output = eyre::Result<T>> + 'static`. The first poll can report
a setup failure. A later poll can report completion or a deferred
failure through the same result. The driver can borrow the machine
between polls and deliver the result to the next stage with a fresh
borrow.

```rust,ignore
use std::future::Future;

pub trait AsyncReusableMessageRecipe<Parent, F, I, Context: ?Sized = ()>: Sized {
    type Error;

    fn prepare(parent: &mut Parent, instruction: &I, context: &Context)
        -> impl Future<Output = Result<Self, Self::Error>> + 'static;
}

pub trait AsyncBuildMessage<F, I, Context: ?Sized = ()>: Field<F> {
    type Message: 'static;
    type Error;

    fn build_message(
        &mut self,
        instruction: &I,
        context: &Context,
    ) -> impl Future<Output = Result<Self::Message, Self::Error>> + 'static;
}

pub trait AsyncExecute<I>: Component {
    type Message;
    type Effect;
    type Error;

    fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> impl Future<Output = Result<Self::Effect, Self::Error>>;
}

pub trait AsyncApply<E> {
    type Effect: 'static;
    type Error;

    fn apply(&mut self, effect: E)
        -> impl Future<Output = Result<Self::Effect, Self::Error>> + 'static;
}

pub trait AsyncHandleEffect<Source, Destination, E, Context: ?Sized = ()>:
    Field<Source> + Field<Destination>
{
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
| Prepare a message or handle a composite-level effect asynchronously | Owned inputs extracted during a short borrow of the composite. |
| Handle an effect asynchronously at a receiving field | The effect and any inputs or handles extracted during a short field borrow. The next stage can borrow the machine afresh. |
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
handle and uses `AsyncApply` to wait for validation. The owned handler
future stores the accepted sample through a field-owned handle. A slot is useful when the component's own async
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

The preparation contract depends on where preparation is defined and
whether it can suspend. A recipe lives on the message type. A builder
lives on the composite. Both can supply the message for `Execute`
or `AsyncExecute`. `BuildMessage` still defaults to the synchronous
execution message. An explicit message type lets it also feed the async
execution node. The execution node checks message compatibility.
The route adapters submit futures that report failures as `eyre::Report`.
[BuildMessage contract](../../crates/vihaco-runtime/src/build_message.rs)
