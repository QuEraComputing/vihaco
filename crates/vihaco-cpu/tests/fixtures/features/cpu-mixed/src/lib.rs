// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

// This graph intentionally opts into the framework without CPU's local flag.
// Check separately: Cargo features cannot activate a dependent crate in reverse.
pub fn cpu() -> host::CPU { host::CPU::default() }
