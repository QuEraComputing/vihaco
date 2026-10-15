# Simplifying the instruction inventory collector

> **Status:** design review. Keep the current direct callback chain unless
> generated macro definitions become a measured problem. No producer or
> consumer API change is proposed.

## Goal and constraints

The inventory protocol must support producers in other crates, named and
wildcard selection, authored selection order, and re-export through components.
A producer returns its complete ordered inventory once. Each entry explicitly
carries `selector`, `runtime`, `surface`, and `source_steps`. The producer's
`RuntimeN` and `SurfaceN` aliases resolve types at its definition site; the
consumer records the producer namespace to qualify them.

A consumer supplies ordered producer requests, a tag for each request, and a
procedural finalizer. Tags have consumer-specific meaning: a component uses
them for selections, while a future composite can use them for child fields.
The collector forwards complete inventories and tags in request order. The
finalizer owns selection, coverage, collision checks, and any new export.

## Current collector

`inventory::collect` generates one local, one-arm callback macro per producer
request. Each callback appends one record containing the request's namespace,
tag, and complete producer inventory. It then directly invokes the next
producer or the finalizer. An empty request list invokes the finalizer
directly.

For three requests, the expansion path is:

```text
first producer → callback 0 → second producer → callback 1
               → third producer → callback 2 → finalizer
```

The callbacks carry only accumulated records as opaque state. The next
producer path and that request's tag are fixed in each generated callback;
there is no request queue, current-request marker, or branch at expansion
time. A repeated producer remains a separate request and yields a separate
record. The producer's `__entries!` macro has one request arm and passes
caller state through unchanged.

## Alternatives considered

| Design | Generated collector | Additional state or machinery | Assessment |
|---|---|---|---|
| Direct callback chain (current) | One one-arm macro per request | Accumulated records only | Clearest handoff and smallest state protocol. |
| Single declarative collector | One macro with at least two response arms | Remaining requests, current namespace and tag, accumulated records | Fewer definitions, but more branching and token forwarding. |
| Shared procedural callback | No local `macro_rules!` callbacks | Hidden proc-macro export/import and a parsed continuation state at every request | Smallest generated surface, but more procedural expansions and implementation code. |

### Single declarative collector

A two-arm form can call the first producer directly. One response arm matches
a nonempty request queue and calls its next producer; the other matches an
empty queue and calls the finalizer. Zero requests still need a direct
finalizer call. This is simpler than the earlier three-arm
start/receive/finish sketch, but both response arms must append a record and
the state still carries `remaining`, `current`, and `collected`.

A request does **not** need both a namespace and a duplicated exporter path.
The collector can capture the namespace as token trees and emit
`$($namespace)*::__entries!`. A standalone Rust 2024 spike compiled with both
bare and `crate::` namespaces. Matching a namespace as `$namespace:path` and
emitting `$namespace::__entries!` fails because a substituted path fragment
cannot be extended that way. Generated requests already contain validated
`syn::Path` values, and the finalizer can parse the forwarded namespace as a
path. This corrects the earlier draft's assertion that a second exporter path
was required.

The two-arm sketch has been checked only with a standalone macro expansion,
not with this repository's cross-crate fixtures, diagnostics, or editor
behavior. It adds a collector continuation expansion at each handoff and can
increase expansion depth. Fewer generated definitions do not imply less
overall token traffic or faster compilation.

### Shared procedural callback

A hidden function-like procedural macro can serve as the callback for every
producer. The consumer's generated module imports it under a local identifier
and invokes the first producer with the existing `callback: ident` contract.
On each response, that macro reads the current namespace and tag, remaining
requests, accumulated records, and consumer context. It emits either the next
producer call or the finalizer call. Rust code can construct the next
`namespace::__entries!` path without duplicating it in each request.

A three-crate `rustc` spike confirmed that an exported producer macro can
invoke a procedural macro imported under a local identifier in a consumer
module. This preserves the producer wire format and removes generated local
callback definitions. It also adds a hidden macro re-export, one procedural
collector expansion per request, repeated parsing and rebuilding of
continuation state, and more context forwarded through each producer. It is
worth prototyping only if generated definitions prove costly.

## Recommendation

Keep the direct callback chain. The proposed single declarative collector
reduces the count of generated macro definitions but makes the control flow
and state grammar more complicated. The procedural callback removes those
definitions but moves the continuation into a new hidden procedural API.
Neither change simplifies the authored `#[instructions(...)]` API or the
producer's one-arm `__entries!` contract.

A small cleanup within the existing `collect` implementation can construct
the repeated `record { namespace, tag, inventory }` token fragment
once and reuse it in its next-producer and finalizer branches. This changes no
wire format or expansion behavior.

If generated size or compile time becomes a concrete problem, compare the
three designs on the same multi-producer and cross-crate fixtures. Preserve
request order, repeated requests, empty inventories, definition-site alias
resolution, re-export, the current compile-fail diagnostics, and authored-path
editor navigation. Check recursion limits and expansion traces before
replacing the direct chain.

## Why a handoff remains

A procedural macro receives only its own invocation tokens; it cannot inspect
items emitted by an independent macro expansion through that API. Cross-crate
producers and wildcard selection therefore need an expansion that exposes
the producer's complete inventory to a later consumer expansion. This
conclusion follows from the [procedural macro API] and [macro expansion rules].
All three collector designs provide that handoff.

[procedural macro API]: https://doc.rust-lang.org/reference/procedural-macros.html
[macro expansion rules]: https://doc.rust-lang.org/reference/macros-by-example.html
