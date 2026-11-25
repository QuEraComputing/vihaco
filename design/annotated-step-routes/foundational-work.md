# Foundational work for annotated step routes

This document tracks the runtime changes that can land before the
[annotated step routes RFC](annotated-step-topologies.md). Each item should
be usable from an authored `Step` and verifiable on its own before the
route framework uses it.

## Composite effect handling

Introduce `HandleEffect` as a public runtime trait before exposing the route
API. It gives authored steps a shared contract for composite-specific
effect handling. `BuildMessage` is the corresponding input hook. A composite
can delegate input preparation to `ReusableMessageRecipe::prepare` and
infallible effect delivery to `Apply`.

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

The authored step or route chooses each next destination. Field-targeted
route nodes continue to use `Receive` or `Apply`, with `Receive::Output`
providing the same typed chaining for subdevice handlers. An authored
`Step` can use the trait directly.
[BuildMessage](../../crates/vihaco-runtime/src/build_message.rs) · [Apply](../../crates/vihaco-runtime/src/stack.rs) · [Route receivers](../../crates/vihaco-runtime/examples/static_graph_routes/framework/operations.rs)
