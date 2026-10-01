// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub use vihaco_abi::Instruction;
#[cfg(feature = "typed-declarations")]
pub use vihaco_runtime::component;
#[cfg(not(feature = "typed-declarations"))]
pub use vihaco_runtime::component_macro as component;
pub use vihaco_runtime::{Message, composite, dispatch, observe};
