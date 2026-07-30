// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

extern crate self as vihaco_runtime;

mod generated;
mod marker;
mod observe;

#[doc(hidden)]
pub mod __private;
pub mod observer;

// Re-export the sub-paths the runtime derive (`vihaco-runtime-derive`) emits so
// that `vihaco-runtime` is a valid derive root on its own, mirroring the facade
// (see design/crate-split.md §5.3).
pub use vihaco_abi::traits::{EffectSink, Reset};
pub use vihaco_abi::{Effects, metadata};
pub use vihaco_bytecode::{BytecodeSectionView, SstSectionView};
pub use vihaco_module::loader;

pub use generated::{CompositeMetadata, GeneratedComponent, expect_exactly_one_effect};
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
pub use vihaco_runtime_derive::{Message, component, composite, machine, observe};
