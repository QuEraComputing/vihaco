// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

extern crate self as vihaco_module;

pub mod color;
pub mod host;
pub mod loader;
pub mod module;
pub mod raising;

mod lowering;
mod program_header;
mod program_metadata;
/// Typed operand access for program-owned execution storage.
pub mod stack;

pub use stack::{PopStack, PushStack, StackValue};
