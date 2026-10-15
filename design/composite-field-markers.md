# Composite field markers and nested routes

Status: implemented locally; changes are uncommitted.

## Caller API

`field!(Machine::controller::cpu::a::b::c)` names a type representing the
complete field route. The first identifier is the root composite declaration;
every remaining identifier is a Rust field name. A single field returns its
nominal marker. Multiple fields compose markers through `FieldPath<Head, Tail>`.

Callers do not name generated markers. Device codes and source aliases do not
participate in marker identity. Ordinary and loadable fields are unavailable.

## Generated namespace tree

Each typed composite emits a hidden public sibling namespace with local markers
in `__markers` and child namespace reexports in `__children`. The namespace
also reexports FieldPath as `__FieldPath` and CheckedRoute as `__CheckedRoute`,
using the declaration's framework root:

```rust,ignore
pub mod __vihaco_fields_Machine {
    pub use framework::__private::FieldPath as __FieldPath;
    pub use framework::__private::CheckedRoute as __CheckedRoute;
    pub mod __markers {
        pub struct controller;
        pub struct spare;
    }
    pub mod __children {
        pub use crate::__vihaco_fields_Controller as controller;
        pub use crate::__vihaco_fields_Controller as spare;
    }
}
```

Typed components emit empty namespaces to terminate traversal. Marker spelling
preserves raw identifiers, case, and Unicode. Conditional declaration gates
cover the namespaces and trait implementations. Naming allowances are scoped
to generated items, and child reexports do not enumerate descendant fields.

The macro expands `field!(Machine::controller::cpu)` to:

```rust,ignore
<Machine as __vihaco_fields_Machine::__CheckedRoute<
    __vihaco_fields_Machine::__FieldPath<
        __vihaco_fields_Machine::__markers::controller,
        __vihaco_fields_Machine::__children::controller::__markers::cpu,
    >,
>>::Marker
```

Additional fields append `__children::<field>` to the namespace path. No field
names are encoded as characters, hashes, numeric keys, or type-level lookup keys.
No callback protocol or global registry is used.

## Field relationships

`Field<F>` associates an actual parent type and nominal field marker with `Child`
and `SOURCE`. Each generated relation also implements DirectField for its nominal marker.
The blanket `Field<FieldPath<Head, Tail>>` implementation requires that direct
head relation and projects
through `Field<Head>::Child`, then through `Field<Tail>` on that child. Its SOURCE
is the final field's canonical name and aliases, not the complete source prefix.

Routes through different outer fields stay distinct even when child types and
final leaf markers coincide. Generic instantiations share declaration markers,
while their Field projections retain their actual child types and domain bounds.
Metadata and capability descriptions retain declaration order. Numeric device
codes remain optional metadata, independent of field identity.


## Checked route resolution

`CheckedRoute<Route>: Field<Route>` has a blanket implementation for every
root satisfying Field<Route>, with `type Marker = Route`. The field macro returns
that associated-type projection. Using it in an ordinary type or Step implementation
forces actual traversal checking, even if the consumer has no Field bound.
Marker identity, zero size, and Field projections remain unchanged. Missing marker
names fail name resolution; incompatible marker sequences fail trait resolution.
An unused alias may defer normalization and therefore defer route validation.
Restricting heads to DirectField prevents unresolved field names from triggering
an unbounded recursive search through candidate FieldPath heads during compiler
error recovery. The macro already constructs paths with single-field heads.

## Finding child namespaces

Direct declared type paths derive their sibling namespace, dropping generic
arguments only from that namespace path. Authored `self`, `super`, `crate`, and
absolute paths retain their scope when reexported from generated modules.

Alternate type spellings must have matching companion namespace reexports in
scope, using ordinary Rust `pub use`.
Opaque generic, associated, or non-path child types have no inferred descendant
namespace. Their own field marker and direct Field relationship remain available,
but nested lookup cannot traverse them with this namespace-tree implementation.

Use a bare root declaration name in `field!`. Its generated sibling namespace
must be in scope. Renamed root imports and type aliases are not automatically
followed. Supply root generic arguments when needed, such as
`field!(Machine<T>::controller::cpu)`. They participate in the check but are
omitted from the generated namespace paths. Module-qualified roots are intentionally not part of this grammar:
a caller importing a root must bring its generated namespace into scope as well.

## Scope and verification

All behavior is gated by the nondefault `typed-declarations` feature. Legacy
feature-off macros, loaders, source aliases, and capability ordering retain their
behavior. Instruction enums, parser generation, Step/Dispatch, and route builders
are outside this change.

Ordinary integration tests cover deep routes, repeated children, exact generic
child projections, raw/Unicode names, aliases, and cross-crate namespaces.
Trybuild fixtures cover unavailable ordinary/loadable/unknown fields, leaf
termination, unsupported device options, generated-name collisions, and malformed syntax.
In-process tests cover expansion and helper validation. Verification includes
default and supported typed tests, doctests, clippy, formatting, license headers,
and external consumer graphs. No custom compiler subprocess test harness is added.
