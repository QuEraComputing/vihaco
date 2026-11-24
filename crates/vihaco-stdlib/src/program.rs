// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Standard program instructions for execution state.

mod dialect;

pub use dialect::program;

mod literal;

pub mod lower;

pub mod operand;

pub mod raise;
