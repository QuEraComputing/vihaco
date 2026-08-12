# Typed observation trees

## Status

Proposal for the instruction-pipeline rewrite.

This document describes how composite runtime routes can preserve statically
known effect types while allowing observers to emit follow-up effects. The
design is intended for any composite whose observers form a typed
fan-out/fan-in-free graph.

## Summary

An instruction route's `effects` block describes a statically typed effect tree.

- Sibling `observe` entries are fan-out branches and receive the same effect.
- A nested observer block receives each concrete effect emitted by its parent
  observer.
- A handler is terminal unless a later extension explicitly permits handler
  follow-ups.
- No universal composite effect enum and no dynamic `DispatchEffect` trait are
  required for the observation graph.

Example:

```rust
Produce(ProducerInstruction) => producer {
    message with resolve_producer_message;

    effects {
        observe transform {
            observe sink;
        }

        observe monitor;
    }
}
```

The generated graph is:

```text
ProducerEffect
├── transform → IntermediateEffect → sink
└── monitor
```

Every edge is checked at compile time. `sink` must implement observation for
the concrete output type of `transform`; it does not receive an erased
machine-level effect.

## Motivation

The current runtime contract allows an observer to return effects, but
generated composite code discards those effects. That supports observers that
only mutate state, but cannot represent workflows such as:

```text
producer effect
  → state update
  → intermediate-effect construction
  → terminal observer
```

Replacing the concrete output with a composite-wide enum would make the flow
dynamic and lose useful static knowledge. A general dispatcher would solve the
problem operationally, but would also move type selection from the composite
declaration into runtime routing. The observation tree keeps the graph in the
declaration and lets Rust type-check each edge.

## Runtime model

The observer trait retains its concrete associated output type:

```rust
pub trait Observe<E, R = ()> {
    type Effect;
    type Error;

    fn observe(&mut self, effect: &E) -> Result<Effects<Self::Effect>, Self::Error>;
}
```

`Effects<T>` remains the existing zero/one/many container. A node may emit no
follow-ups, one follow-up, or multiple follow-ups. The generated executor
iterates the returned `Effects<T>` and evaluates the node's nested children for
each value.

The route's component effects remain concrete. For example:

```rust
impl Execute<ProducerInstruction> for Producer {
    type Message = ProducerMessage;
    type Effect = ProducerEffect;
    type Fault = eyre::Report;
    // ...
}

impl Observe<ProducerEffect, TransformRoute> for Transform {
    type Effect = IntermediateEffect;
    type Error = eyre::Report;
    // ...
}

impl Observe<IntermediateEffect, SinkRoute> for Sink {
    type Effect = NoEffect;
    type Error = eyre::Report;
    // ...
}
```

`NoEffect` is the terminal output type for observers that intentionally emit
nothing. If an observer returns another meaningful type, it should either have
nested consumers or be explicitly discarded by syntax that makes the discard
visible.

## Proposed syntax

The existing route shape remains the foundation:

```rust
runtime {
    RouteName(Payload) => target {
        message ...;
        effects {
            // observation tree and/or terminal handler
        }
    }
}
```

### Basic observer

```rust
effects {
    observe debug;
}
```

The observer receives the component effect produced by the route. Its output
must be terminal or explicitly discarded.

### Nested follow-up

```rust
effects {
    observe transform {
        observe sink;
    }
}
```

The nested observer receives the concrete associated `Effect` type returned by
`transform`.

### Fan-out

```rust
effects {
    observe transform {
        observe sink;
    }

    observe monitor;
}
```

Sibling entries receive the original route effect independently. The generated
code must not feed the output of `transform` into `monitor`.

### Nested fan-out

```rust
effects {
    observe transform {
        observe sink;
        observe effect_logger;
    }

    observe monitor;
}
```

Both nested observers receive each `IntermediateEffect` emitted by
`transform`.

### Terminal handler

```rust
effects {
    observe event_logger {
        handle with emit_log;
    }
}
```

The handler receives the concrete output type of `event_logger`. Handlers
are terminal in the initial design and return `Result<(), Error>`.

### Direct route handler

```rust
effects {
    observe monitor;
    handle with handle_producer_effect;
}
```

The direct handler receives the original component effect, independently of
the observer branches.

### Explicit discard

If an observer intentionally produces an effect that is not consumed, make
that decision visible:

```rust
effects {
    observe metrics => discard;
}
```

The generated code still knows the observer's concrete output type, but does
not require a child consumer. The compiler should reject `discard` for an
observer whose output is not explicitly permitted to be dropped if the project
chooses a strict-loss policy.

The initial implementation may instead require terminal observers to return
`NoEffect`, postponing `discard` until a concrete use case needs it.

## Semantics

For each route:

1. Resolve the route message.
2. Execute the selected component instruction.
3. For every component effect, evaluate every sibling observation branch.
4. For every observer output, evaluate its nested observation branches.
5. Invoke terminal handlers at the point where their input type is known.
6. Return the route's `Execution` result to the composite's caller.

The generated implementation is conceptually recursive, but it should not use
unbounded Rust call-stack recursion for effect values. A small internal work
stack or queue can evaluate nested nodes while preserving the declared order.
The logical order is depth-first, left-to-right:

```text
for root effect:
    branch 1 and all descendants
    branch 2 and all descendants
```

This preserves the current declaration-order expectation for observers and
handlers while avoiding a dynamic type-erased effect queue.

The implementation can generate typed helper functions for each node rather
than storing heterogeneous nodes in one collection. For example:

```rust
fn observe_transform(
    &mut self,
    effect: &ProducerEffect,
) -> Result<(), CompositeError> {
    let follow_ups = Observe::<ProducerEffect, TransformRoute>::observe(
        &mut self.transform,
        effect,
    )?;

    for intermediate in follow_ups {
        self.observe_sink(&intermediate)?;
    }

    Ok(())
}
```

This approach keeps the generated code monomorphic and lets the compiler
report an invalid edge at the observer declaration.

## Macro representation

The macro parser should represent an observation node as a tree rather than as
the current flat observer list:

```rust
struct ObservationNode {
    observer: Ident,
    children: Vec<EffectNode>,
    terminal: TerminalAction,
}

enum EffectNode {
    Observe(ObservationNode),
    Handle(Ident),
    Discard,
}
```

The exact internal names are not important. The essential property is that the
parent-child relationship survives parsing and validation.

The route validator should walk the tree with an input effect type:

```text
validate(node, input_effect_type):
    observer = observer_field(node)
    output_type = <observer as Observe<input_effect_type, route>>::Effect
    validate(each child, output_type)
```

Handlers are validated against the current input effect type. Sibling nodes are
each validated against the same parent input type.

## Route markers

The existing route marker mechanism should be extended so each observation edge
has a stable marker. This allows one component to observe the same effect type
in different routes with different behavior:

```rust
Observe<ProducerEffect, ProducerTransformRoute>
Observe<ProducerEffect, AnotherRoute>
```

Nested edges may receive generated marker names derived from their path, such
as `ProducerTransformSinkRoute`. Markers should remain private implementation
details.

## Errors and partial execution

An observer error stops evaluation of the current route and is converted into
the composite error, just as component and handler errors are today.

Effects already applied before the error are not rolled back. This matches the
existing mutable observer model. Documentation should state that observers
should either be order-independent or perform validation before mutating state
when transactional behavior matters.

The generated code must not install or advance program state based on a route
until the route's message resolution and component execution have succeeded.
Observation errors occur after component execution, so the component's state
mutation is also not rolled back.

## Cycles and resource limits

The syntax naturally describes a tree, not a runtime graph. An observer cannot
refer to an ancestor through the declaration, so accidental cycles are
impossible in the generated observation structure.

If a future feature allows handlers to emit follow-ups or dynamically selects
observers, it must introduce an explicit effect budget or cycle policy. That is
outside the initial design.

## Worked example

An example producer route could use:

```rust
Produce(ProducerInstruction) => producer {
    message with resolve_producer_message;
    effects {
        observe transform {
            observe sink;
        }
        observe monitor;
    }
}
```

An event-logging path could use:

```rust
Event(EventInstruction) => event_source {
    message with resolve_event_message;
    effects {
        observe event_logger {
            handle with emit_log;
        }
    }
}
```

Instruction completion and program-counter policy should remain separate from
observation effects. Execution state controls whether the route completed;
observation effects describe typed side effects and follow-ups.

## Implementation phases

### Phase 1: Runtime contract

- Keep `Observe<E, R>::Effect` as a concrete associated type.
- Add tests covering zero, one, and many observer follow-up effects.
- Decide whether terminal observers require `NoEffect` or support explicit
  `discard`.
- Document ordering and non-transactional mutation behavior.

### Phase 2: Macro syntax and AST

- Replace the flat observer list in route effects with an observation tree.
- Parse nested observer blocks.
- Parse terminal handler and discard actions.
- Preserve source spans for nested validation errors.

### Phase 3: Static validation

- Validate every observer field.
- Validate each nested edge against the parent's associated output type.
- Validate handlers against their input effect type.
- Reject observers whose output has no consumer unless they are terminally
  allowed.
- Add compile-fail tests for mismatched nested observers and invalid handlers.

### Phase 4: Code generation

- Generate typed helper functions or typed nested blocks.
- Generate private route markers for observation edges.
- Preserve sibling fan-out and left-to-right depth-first ordering.
- Convert all observer errors into the composite error.
- Ensure no heterogeneous runtime effect container is introduced.

### Phase 5: Integration tests

Add tests for:

- one observer with one follow-up;
- two sibling observers receiving the same input;
- nested fan-out;
- multiple effects emitted at one level;
- terminal `NoEffect` observers;
- explicit discard, if supported;
- error propagation from parent and nested observers;
- declaration-order guarantees;
- route-local marker specialization.

### Phase 6: composite integration

- Port a producer, transforming observer, and terminal observer as a reference
  implementation.
- Express a typed fan-out and nested follow-up pipeline.
- Add a terminal observer/handler chain.
- Keep instruction completion and scheduling policy separate from observation
  behavior.
- Add end-to-end tests that assert both machine state and observation order.

## Non-goals

This proposal does not introduce:

- a universal composite effect enum;
- dynamic effect dispatch;
- runtime observer registration;
- handler follow-up effects;
- rollback or transactional observers;
- arbitrary cyclic effect graphs;
- a scheduler or event loop.

Those features may be useful later, but they would weaken the simple static
model needed for the instruction pipeline rewrite.
