// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub mod program;

pub use program::ProgramImage;

pub mod clock;
pub mod observer;

#[cfg(feature = "program-image-execution")]
pub mod execution_state;

#[cfg(feature = "program-image-execution")]
pub use execution_state::ExecutionState;

#[cfg(feature = "program-image-execution")]
pub mod image;

#[cfg(feature = "program-image-execution")]
pub use image::Image;

/// Re-exported so image construction needs no direct `vihaco-verifier` dependency.
#[cfg(feature = "program-image-execution")]
pub use vihaco_verifier::Verified;

#[cfg(feature = "program-image-execution")]
pub mod loadable;

#[cfg(feature = "program-image-execution")]
pub use loadable::Loadable;

#[cfg(feature = "program-image-execution")]
#[doc(hidden)]
pub use execution_state::{__vihaco_fields_ExecutionState, __vihaco_inventory_ExecutionState};
