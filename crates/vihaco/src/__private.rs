// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[doc(hidden)]
pub trait GeneratedMachine {
    type Instruction;

    fn metadata(&self) -> crate::runtime::CompositeMetadata;
}
