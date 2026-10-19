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
