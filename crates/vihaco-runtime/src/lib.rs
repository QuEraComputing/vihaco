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
//! The nondefault `program-image-execution` feature implies `typed-declarations`
//! and adds `PopFromStack`, `PushToStack`, typed stack access, and capability-
//! gated message preparation. A `#[program]` device supplies program access and
//! operand-storage wiring. Composite authors implement Steps and choose effect
//! handling; program execution lives in stdlib.
//!
//! With `typed-declarations`, `#[dialect]` gives public runtime instruction
//! structs syntax parsers. Operand types retain their authored scope. Only
//! immediate marked nongeneric unit/tuple structs belong to a dialect. Differing
//! annotated operands generate a phase parameter defaulting to [`Runtime`].
//! Composite syntax aggregates lower through instruction-owned [`Lower`]
//! implementations; unchanged instructions receive identity lowering.
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
//! let source = arithmetic::Add::parser().parse("add -3")
//!     .into_result().unwrap();
//! assert_eq!(source, arithmetic::Add(-3));
//! let runtime = source;
//! assert_eq!(runtime.0, -3);
//! # }
//! ```
//!
//! The attribute consumes `instruction` and `pattern` helpers, preserves module
//! and field visibility, and supplies Clone/Debug/PartialEq without duplicating
//! those authored derives. Named fields, generic or raw leaf names, conditional
//! membership, and generated-name collisions are rejected. Gate the entire
//! module with `cfg`/`cfg_attr`. Unannotated pattern bindings use their runtime
//! field's parser. A binding such as `$0:u64` retains that syntax operand type;
//! differing runtime types require explicit lowering. Parser annotations retain
//! ordinary Rust type scope.

extern crate self as vihaco_runtime;

mod generated;

mod marker;

mod member_of;

mod no_message;

mod observe;

#[cfg(feature = "typed-declarations")]
mod build_message;

#[cfg(feature = "typed-declarations")]
mod capability;

#[cfg(feature = "typed-declarations")]
mod dispatch;

#[cfg(feature = "typed-declarations")]
mod execute;

#[cfg(feature = "typed-declarations")]
mod field;

#[cfg(feature = "typed-declarations")]
mod step;

/// Generated declaration contracts. Manual implementations are unsupported.
#[cfg(feature = "typed-declarations")]
#[doc(hidden)]
#[path = "declarations.rs"]
pub mod __private;

#[cfg(feature = "typed-declarations")]
pub use build_message::{BuildMessage, PrepareMessage};

#[cfg(feature = "typed-declarations")]
#[doc(hidden)]
pub use build_message::AutomaticMessage;

#[cfg(feature = "typed-declarations")]
pub use dispatch::Dispatch;

#[cfg(feature = "typed-declarations")]
pub use execute::Execute;

#[cfg(feature = "typed-declarations")]
pub use field::Field;

#[cfg(feature = "typed-declarations")]
pub use step::Step;

// Re-export the sub-paths the runtime derive (`vihaco-runtime-derive`) emits so
// that `vihaco-runtime` is a valid derive root on its own, mirroring the facade
// (see design/crate-split.md §5.3).
pub use chumsky;

#[doc(hidden)]
pub use eyre;
pub use vihaco_abi::traits::{EffectSink, Reset};
pub use vihaco_abi::{Effects, Lower, Runtime, Syntax, metadata};
pub use vihaco_bytecode::{BytecodeSectionView, SstSectionView};
pub use vihaco_module::loader;
#[doc(hidden)]
pub use vihaco_parser::trivia;
pub use vihaco_parser::{Parse, SurfaceInstruction};

pub use generated::{
    Component, Composite, CompositeMetadata, GeneratedComponent, HasInstructionSet,
    expect_exactly_one_effect,
};

pub use marker::Message;
pub use member_of::MemberOf;
pub use no_message::NoMessage;
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

/// Declare device fields and complete child capabilities.
///
/// With `typed-declarations`, named device declarations contribute every leaf
/// from their declared dialects to a hidden flat runtime enum exposed through
/// [`HasInstructionSet`]. Its syntax parser uses
/// `field::nested_field::dialect::instruction` prefixes and accepts device aliases.
/// Implement [`Step`] for every destination/leaf pair. With `program-image-execution`,
/// a `#[program]` device selects program access and operand-storage wiring; Steps
/// and effect handling remain authored.
/// The generated [`Dispatch`] implementation requires complete [`Step`] coverage
/// for the selected context when a `Dispatch<Context>` bound is required or dispatch
/// is called. Declaring a composite alone does not check Step coverage.
/// Generic declarations retain their original bounds.
/// Every device needs an explicit u8 code, unique within its parent. Ordinary
/// state is excluded from parent instruction sets.
/// Without `program-image-execution`, `#[loadable]` devices form section boundaries
/// and are excluded from parent instruction sets. The macro generates legacy
/// section-loading implementations in that configuration. With the feature
/// enabled, construct images explicitly and pair them with stdlib `Loadable`;
/// `#[loadable]` helpers and generated section loading are unavailable.
/// [`Field::get`] and [`Field::get_mut`] borrow eligible devices through direct or
/// nested field routes, including devices that contain borrowed data.
/// Named generic children are supported; bare type parameters and associated
/// child types cannot supply an inferred inventory companion. Renamed child
/// imports/type aliases require matching `__vihaco_inventory_Name` and field
/// companion reexports. Inventory slots and variant names are not bytecode IDs.
///
/// ```
/// # #[cfg(feature = "typed-declarations")]
/// # mod example {
/// use vihaco_runtime::{Execute, NoMessage, Step, field, component, composite, dialect};
/// #[dialect]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// mod control { #[instruction] pub struct Halt; }
/// #[component]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// #[instructions { control }]
/// struct Cpu;
/// impl Execute<control::Halt> for Cpu {
///     type Message = NoMessage;
///     type Effect = ();
///     type Error = std::convert::Infallible;
///     fn execute(&mut self, _: &control::Halt, _: NoMessage) -> Result<(), Self::Error> {
///         Ok(())
///     }
/// }
/// #[composite]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// struct Machine { #[device(7, alias = "host")] cpu: Cpu }
/// impl Step<field!(Machine::cpu), control::Halt> for Machine {
///     fn step(&mut self, instruction: &control::Halt, _: &()) -> vihaco_runtime::eyre::Result<()> {
///         self.cpu.execute(instruction, NoMessage::new())?;
///         Ok(())
///     }
/// }
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

#[cfg(feature = "derive")]
pub use vihaco_parser_derive::Parse;

/// Declare instructions with retained syntax operands and capability-based lowering.
///
/// Available with `typed-declarations`. The framework dependency is discovered
/// automatically, including renamed dependencies. To use a module reexporting
/// the framework API, apply `#[vihaco(crate = path)]` to the dialect module.
/// The path is resolved in the annotated module's parent scope; `self`, `super`,
/// `crate`, and absolute paths are supported. Operand types retain their
/// authored scope. Unannotated bindings use the corresponding field's parser.
/// `$0:SyntaxType` retains another `Parse` type. If it differs from the field,
/// the generated instruction has a phase parameter defaulting to [`Runtime`],
/// and its [`Syntax`] form requires an instruction-owned [`Lower`] implementation.
/// Unchanged instructions remain nongeneric and receive identity lowering.
/// Write authored lowering implementations outside the dialect module, beside
/// its declaration or in a separate module in the same crate.
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
///     assert_eq!(arithmetic::Add::parser().parse("add 2")
///         .into_result().unwrap().0, 2);
/// }
/// ```
///
/// Differing operand types are retained until explicit lowering, even when
/// Rust already provides a conversion between them:
///
/// ```
/// use vihaco_runtime::{dialect, Lower, Parse, Syntax, chumsky::Parser};
/// #[dialect]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// mod control {
///     #[instruction]
///     #[pattern = "'call $0:u64"]
///     pub struct Call(pub u32);
/// }
/// impl<C: ?Sized> Lower<C> for control::Call<Syntax> {
///     type Runtime = control::Call;
///     fn lower(self, _: &mut C) -> vihaco_runtime::eyre::Result<Self::Runtime> {
///         Ok(control::Call(self.0.try_into()?))
///     }
/// }
/// let source = control::Call::<Syntax>::parser().parse("call 4294967296")
///     .into_result().unwrap();
/// assert_eq!(source.0, 4294967296);
/// assert!(source.lower(&mut ()).is_err());
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
/// use vihaco_runtime::{component, dialect, Execute, NoMessage};
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
///     type Message = NoMessage;
///     type Effect = ();
///     type Error = std::convert::Infallible;
///     fn execute(&mut self, _: &control::Halt, _: NoMessage) -> Result<(), Self::Error> {
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
/// Ordinary state and source aliases are not field targets.
///
/// Child namespaces follow their authored type paths. Alternate type spellings
/// need matching companion namespace reexports. Inventory generation requires
/// a named child declaration; it cannot infer bare generic or associated children.
///
/// ```
/// use vihaco_runtime::{composite, field};
/// use vihaco_runtime::Field;
/// #[composite]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// struct Leaf { state: u8 }
/// #[composite]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// struct Controller { #[device(1)] cpu: Leaf }
/// #[composite]
/// # #[vihaco(crate = ::vihaco_runtime)]
/// struct Machine {
///     #[device(7)] controller: Controller,
/// }
/// type Route = field!(Machine::controller::cpu);
/// type Child = <Machine as Field<Route>>::Child;
/// fn main() {
///     let _: Child = Leaf { state: 0 };
/// }
/// ```
#[cfg(feature = "typed-declarations")]
pub use vihaco_runtime_derive::field;

#[cfg(feature = "program-image-execution")]
mod stack;

#[cfg(feature = "program-image-execution")]
pub use stack::{Apply, HasOperandStack, InputCount, PopFromStack, PushToStack, StackInputCount};

#[cfg(feature = "program-image-execution")]
mod stack_batch;

#[cfg(feature = "program-image-execution")]
pub use stack_batch::{StackBatch, StackBatchIntoIter};

#[cfg(feature = "program-image-execution")]
pub use vihaco_module::{PopStack, PushStack, StackValue};

#[cfg(feature = "program-image-execution")]
mod execution;

#[cfg(feature = "program-image-execution")]
pub use execution::HasProgram;
