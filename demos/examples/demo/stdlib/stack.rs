// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// A reusable operand-stack component with invariant-preserving operations.
struct Stack {
    items: Vec<i64>,
}

impl Stack {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Load initial operands with the rightmost value treated as the top of the stack.
    fn seeded(values: &[i64]) -> Self {
        Self {
            items: values.to_vec(),
        }
    }

    fn push(&mut self, value: i64) {
        self.items.push(value);
    }

    fn pop(&mut self) -> Result<i64, StackFault> {
        self.items.pop().ok_or(StackFault::Underflow)
    }

    fn top(&self) -> Option<i64> {
        self.items.last().copied()
    }
}

impl Supply<i64> for Stack {
    type Fault = StackFault;

    fn supply(&mut self) -> Result<i64, StackFault> {
        self.pop()
    }
}

#[derive(Debug)]
enum StackFault {
    Underflow,
}
