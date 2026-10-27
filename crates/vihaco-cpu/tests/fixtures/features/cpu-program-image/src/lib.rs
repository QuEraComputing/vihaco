// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The facade opt-in forwards through stdlib to CPU and the runtime protocols.

pub use vm::{execute, resume};

pub fn instruction() -> host::cpu::AddU64
where
    host::CPU: rt::Execute<
            host::cpu::AddU64,
            Message = rt::PopFromStack<(host::Word, host::Word)>,
            Effect = rt::PushToStack<(host::Word,)>,
        >,
{
    host::cpu::AddU64
}

pub fn message() -> vm::PopFromStack<(bool,)> {
    rt::PopFromStack((true,))
}

pub fn effect() -> rt::PushToStack<(host::Word,)> {
    vm::PushToStack((1,))
}
