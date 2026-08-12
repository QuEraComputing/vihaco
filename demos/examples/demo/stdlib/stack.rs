// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::supply::Supply;

vihaco::component! {
    component Stack {
        items: Vec<i64>,
    }

    runtime {
        instruction {
            Push(i64),
            Pop,
        }
    }
}

pub use stack::Stack;

impl Stack {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Load initial operands with the rightmost value treated as the top of the stack.
    pub fn seeded(values: &[i64]) -> Self {
        Self {
            items: values.to_vec(),
        }
    }

    pub fn push(&mut self, value: i64) {
        self.items.push(value);
    }

    pub fn pop(&mut self) -> Result<i64, StackFault> {
        self.items.pop().ok_or(StackFault::Underflow)
    }

    pub fn top(&self) -> Option<i64> {
        self.items.last().copied()
    }

    pub fn view(&self) -> &[i64] {
        &self.items
    }
}

impl Supply<i64> for Stack {
    type Fault = StackFault;

    fn supply(&mut self) -> Result<i64, StackFault> {
        self.pop()
    }
}

#[derive(Debug)]
pub enum StackFault {
    Underflow,
}
