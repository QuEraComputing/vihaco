---
layout: ../../layouts/Guide.astro
title: Handling effects
slug: effects
description: "Choose how a step applies results and handles multiple effects."
---

# Handling effects

After a component succeeds, it hands a result back to the step. That effect
might be a computed value, a record to collect, a request for another action,
or several outputs. Your step decides what to do with it.

## Apply a result

`Apply<E>` takes ownership of one effect and applies it to a destination you
choose. For example, the standard `PushToStack` protocol inserts typed values
into operand storage. Call it after `execute`; execution alone won't apply the
result.

`Apply` has no error return. If handling a result involves validation or other
fallible work, do that in your step or a helper and propagate the error there.
You don't need an `Apply` implementation for every application effect. A step
can also store a result in another device or pass it to an application method.

## Handle several effects

`Effects<E>` holds zero, one, or many outputs. You can build up a collection and
iterate over its values in order:

```rust
use vihaco::Effects;

let effects = Effects::one(20_u64).append(22);
let results: Vec<_> = effects.into_iter().collect();
assert_eq!(results, vec![20, 22]);
```

In a step, handle each effect according to what it means for your machine.
If handling one result produces more work, decide whether to finish that work
before moving on or queue it for later. Vihaco leaves that choice to you.

## Preserve failure behavior

A helper can change device state and then return an error. Earlier calls and
stack insertions stay in place. Decide whether an error should stop processing,
and make sure you handle results that matter to execution.

Next, see how these steps fit into [running a program](/guide/program-execution).
