// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! CPU opt-in works without a facade dependency.

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

pub fn message() -> rt::PopFromStack<(host::Word,)> {
    rt::PopFromStack((1,))
}
