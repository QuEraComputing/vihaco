---
layout: ../../layouts/Guide.astro
title: Observing Effects
slug: observers
description: "Borrow effects for tracing and diagnostics while a composite routes ownership to one handler."
---

# Observing Effects With `Observe`

Observers inspect an effect without consuming it. Implement
`Observe<E, R>` for a field that records traces, updates metrics, or performs
diagnostics. `R` is the composite route marker, so the same effect type can be
observed differently on different routes.

```rust
use eyre::Result;
use vihaco::{Effects, Observe};

#[derive(Debug)]
struct Line(String);

#[derive(Default)]
struct Logger { lines: Vec<String> }

impl<R> Observe<Line, R> for Logger {
    type Effect = ();
    type Error = eyre::Report;

    fn observe(&mut self, line: &Line) -> Result<Effects<()>> {
        self.lines.push(line.0.clone());
        Ok(Effects::none())
    }
}
```

In a `composite!` route, list observers in the order they should run:

```text
effects {
    observe logger, metrics;
    absorb with output_stack;
}
```

Each observer borrows the same effect. The handler then receives ownership
exactly once. This makes the ownership flow clear:

```text
Execute -> Effects<E> -> Observe(&E) ... -> Handle(E)
```

The observer's associated `Effect` is reserved for typed follow-up work. The
current generated route dispatch does not automatically schedule those
follow-up effects; a future runtime extension may make that continuation
explicit. Until then, return `Effects::none()` or handle follow-up effects in
your own runtime boundary.

An observer is an ordinary component field; it does not need an instruction
catalog or a message source. A component can also implement `Observe` when it
needs to react to another component's output.

See [Defining a Composite](/guide/composites) for effect handlers and route
selection.
