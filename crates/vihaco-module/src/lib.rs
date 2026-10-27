// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

extern crate self as vihaco_module;

pub mod color;
pub mod host;
pub mod loader;
pub mod module;

mod lowering;
mod program_metadata;
/// Typed operand access for program-owned execution storage.
#[cfg(feature = "program-image-execution")]
pub mod stack;

#[cfg(feature = "program-image-execution")]
pub use stack::{PopStack, PushStack, StackValue};
