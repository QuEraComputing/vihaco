// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! CPU opt-in works without a facade dependency.

pub fn instruction() -> host::cpu::AddU64 {
    host::cpu::AddU64
}

pub fn message() -> rt::PopFromStack<(host::Word,)> {
    rt::PopFromStack((1,))
}
