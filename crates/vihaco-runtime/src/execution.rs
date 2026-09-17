// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub trait Execute<I> {
    type Message;
    type Effect;

    fn execute(&mut self, message: Self::Message, instruction: &I) -> Self::Effect;
}
