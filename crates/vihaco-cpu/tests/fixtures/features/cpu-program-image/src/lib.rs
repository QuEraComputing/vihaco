// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The facade opt-in forwards through stdlib to CPU and the runtime protocols.

pub fn instruction() -> host::cpu::AddU64 {
    host::cpu::AddU64
}

pub fn message() -> vm::PopFromStack<(bool,)> {
    rt::PopFromStack((true,))
}

pub fn effect() -> rt::PushToStack<(host::Word,)> {
    vm::PushToStack((1,))
}
