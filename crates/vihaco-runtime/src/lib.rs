// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Component execution, composite metadata, and observation machinery.
//!
//! The nondefault `typed-declarations` feature enables experimental typed
//! execution and capability contracts, including derive support. It decouples
//! the Component/Composite markers from HasInstructionSet; Cargo unification
//! selects that contract for the shared runtime package instance. Declaration
//! macros provide complete dialect/component capabilities and typed composite
//! fields. Feature-off execution retains the existing declarations. The two
//! declaration modes require separate Cargo feature graphs.
//!
//! With `typed-declarations`, `#[dialect]` pairs public runtime structs with
//! surface structs defined in the same module and reexported as `syntax::Name`.
//! Both use the authored operand types;
//! operands implement [`Parse`] through ordinary Rust. Only immediate marked
//! unit/tuple structs belong to the dialect. Generated `syntax::Add` parses the
//! local mnemonic; selection and target prefixes belong to instruction sets.
//!
//! ```
//! # #[cfg(feature = "typed-declarations")]
//! # {
//! use vihaco_runtime::{dialect, Parse, chumsky::Parser};
//!
//! #[dialect]
//! # #[vihaco(crate = ::vihaco_runtime)]
//! pub mod arithmetic {
//!     #[instruction]
//!     pub struct Add(pub i64);
//!
//!     #[instruction]
//!     #[pattern = "'stop"]
//!     pub struct Halt;
//! }
//!
//! let source = arithmetic::syntax::Add::parser().parse("add -3")
//!     .into_result().unwrap();
//! assert_eq!(source, arithmetic::syntax::Add(-3));
//! let runtime = arithmetic::Add(source.0);
//! assert_eq!(runtime.0, -3);
//! # }
//! ```
//!
//! The attribute consumes `instruction` and `pattern` helpers, preserves module
//! and field visibility, and supplies Clone/Debug/PartialEq without duplicating
//! those authored derives. Named fields, generic or raw leaf names, conditional
//! membership, and generated-name collisions are rejected. Gate the entire
//! module with `cfg`/`cfg_attr`. The generated `syntax` module is reserved.

extern crate self as vihaco_runtime;

mod generated;
mod marker;
mod observe;

#[cfg(feature = "typed-declarations")]
mod capability;
#[cfg(feature = "typed-declarations")]
mod execute;
#[cfg(feature = "typed-declarations")]
mod field;

/// Generated declaration contracts. Manual implementations are unsupported.
#[cfg(feature = "typed-declarations")]
#[doc(hidden)]
#[path = "declarations.rs"]
pub mod __private;

#[cfg(feature = "typed-declarations")]
pub use execute::Execute;
#[cfg(feature = "typed-declarations")]
pub use field::Field;

// Re-export the sub-paths the runtime derive (`vihaco-runtime-derive`) emits so
// that `vihaco-runtime` is a valid derive root on its own, mirroring the facade
// (see design/crate-split.md §5.3).
pub use chumsky;
#[doc(hidden)]
pub use eyre;
pub use vihaco_abi::traits::{EffectSink, Reset};
pub use vihaco_abi::{Effects, metadata};
pub use vihaco_bytecode::{BytecodeSectionView, SstSectionView};
pub use vihaco_module::loader;
pub use vihaco_parser::{Parse, SurfaceInstruction};

pub use generated::{
    Component, Composite, CompositeMetadata, GeneratedComponent, HasInstructionSet,
    expect_exactly_one_effect,
};
pub use marker::Message;
pub use observe::Observe;

// `#[derive(Message)]` emits `#root::runtime::Message`; keep a `runtime` segment
// available on this crate too (the facade exposes it via `pub use vihaco_runtime
// as runtime;`) so direct dependents resolve the marker identically.
pub use crate as runtime;

// The `Instruction` derive lives in `vihaco-abi(-derive)`; re-export it here so
// `#[composite]`'s generated `#root::Instruction` derive resolves through the
// runtime root as well.
#[cfg(feature = "derive")]
pub use vihaco_abi::Instruction;

// Re-export the runtime derives behind the `derive` feature (serde convention).
#[cfg(feature = "derive")]
pub use vihaco_runtime_derive::{Message, dispatch, machine, observe};

/// Declare device fields, complete child capabilities, and loadable sections.
///
/// With `typed-declarations`, field relationships retain the actual authored
/// child type, including aliases and generic or associated types. Ordinary
/// state and `#[loadable]` fields do not contribute parent capabilities.
/// Source aliases are metadata; this macro does not generate an instruction
/// enum, parser, or dispatch. Device codes are optional metadata. Each eligible
/// field has a distinct marker type named through [`field!`]. Nested field paths
/// compose those markers. Child namespaces are inferred from authored type paths.
///
/// ```
/// # #[cfg(feature = "typed-declarations")]
/// # mod example {
/// use vihaco_runtime::{Execute, component, composite, dialect};
/// #[dialect]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// mod control { #[instruction] pub struct Halt; }
/// #[component]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// #[instructions { control }]
/// struct Cpu;
/// impl Execute<control::Halt> for Cpu {
///     type Message = ();
///     type Effect = ();
///     type Error = std::convert::Infallible;
///     fn execute(&mut self, _: &control::Halt, _: ()) -> Result<(), Self::Error> {
///         Ok(())
///     }
/// }
/// #[composite]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// struct Machine { #[device(7, alias = "host")] cpu: Cpu }
/// # }
/// # fn main() {}
/// ```
///
/// Without `typed-declarations`, this retains the legacy composite expansion.
#[cfg(feature = "derive")]
pub use vihaco_runtime_derive::composite;

#[cfg(all(feature = "derive", not(feature = "typed-declarations")))]
pub use vihaco_runtime_derive::component_macro;

#[cfg(all(feature = "derive", not(feature = "typed-declarations")))]
pub use vihaco_runtime_derive::component_macro as component;

#[cfg(feature = "typed-declarations")]
pub use vihaco_parser_derive::Parse;

/// Declare runtime instructions and their paired `syntax` types.
///
/// Available with `typed-declarations`. The framework dependency is discovered
/// automatically, including renamed dependencies. To use a module reexporting
/// the framework API, apply `#[vihaco(crate = path)]` to the dialect module.
/// The path is resolved in the annotated module's parent scope; `self`, `super`,
/// `crate`, and absolute paths are supported. Operand types retain their
/// authored scope.
///
/// ```
/// use vihaco_runtime::dialect;
///
/// mod framework {
///     pub use vihaco_runtime::*;
/// }
///
/// #[dialect]
/// #[vihaco(crate = crate::framework)]
/// pub mod arithmetic {
///     #[instruction]
///     pub struct Add(pub i64);
/// }
///
/// fn main() {
///     use framework::{Parse, chumsky::Parser};
///     assert_eq!(arithmetic::syntax::Add::parser().parse("add 2")
///         .into_result().unwrap().0, 2);
/// }
/// ```
#[cfg(feature = "typed-declarations")]
pub use vihaco_runtime_derive::dialect;

/// Declare a component's complete dialect capabilities.
///
/// Available with `typed-declarations`. Apply to any struct with exactly one
/// nonempty `#[instructions { dialect_paths, ... }]` helper. Paths name whole
/// dialect modules, including imported aliases. Implement [`Execute`] for every
/// instruction in every listed dialect under the struct's original bounds.
/// Each implementation may choose its own message, effect, and error types.
/// The declaration preserves the struct and implements [`Component`] without
/// requiring an aggregate instruction set. Canonical instruction overlaps are
/// rejected even on unused generic components. `#[vihaco(crate = path)]`
/// selects an explicit framework root in the struct's authored scope.
///
/// ```
/// use vihaco_runtime::{component, dialect, Execute};
/// #[dialect]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// mod control {
///     #[instruction]
///     pub struct Halt;
/// }
/// #[component]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// #[instructions { control }]
/// struct Cpu<T = u8>(T);
/// impl<T> Execute<control::Halt> for Cpu<T> {
///     type Message = ();
///     type Effect = ();
///     type Error = std::convert::Infallible;
///     fn execute(&mut self, _: &control::Halt, _: ()) -> Result<(), Self::Error> {
///         Ok(())
///     }
/// }
/// ```
#[cfg(feature = "typed-declarations")]
pub use vihaco_runtime_derive::component;

/// Name a composite field or nested route as a marker type.
///
/// Write `field!(Machine::controller::cpu)` with the composite declaration name
/// first, followed by Rust field names. The generated namespace must be in scope;
/// renamed composite imports and root type aliases are not discovered automatically.
/// For generic roots, supply their arguments: `field!(Machine<T>::controller::cpu)`.
/// The macro checks the route through CheckedRoute before resolving to its
/// unchanged nominal marker type. An unused type alias may defer that check.
/// Nested routes compose nominal markers with FieldPath and project the final
/// child. Route SOURCE describes the final field, not the full source prefix.
/// Ordinary state, loadable fields, and source aliases are not field targets.
///
/// Child namespaces follow their authored type paths. Alternate type spellings
/// need matching companion namespace reexports. Opaque generic or associated
/// child types retain one-field markers but do not expose descendant namespaces.
///
/// ```
/// use vihaco_runtime::{composite, field};
/// use vihaco_runtime::Field;
/// #[composite]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// struct Leaf { state: u8 }
/// #[composite]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// struct Controller<T: vihaco_runtime::__private::Capabilities> { #[device(1)] cpu: T }
/// #[composite]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// struct Machine<T: vihaco_runtime::__private::Capabilities> {
///     #[device(7)] controller: Controller<T>,
/// }
/// type Route = field!(Machine<Leaf>::controller::cpu);
/// type Child = <Machine<Leaf> as Field<Route>>::Child;
/// fn main() {
///     let _: Child = Leaf { state: 0 };
/// }
/// ```
#[cfg(feature = "typed-declarations")]
pub use vihaco_runtime_derive::field;
