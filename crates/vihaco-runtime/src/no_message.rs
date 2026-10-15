// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// Execution input for instructions that need no runtime-supplied message.
///
/// Unlike `()`, this type explicitly names the absence of an execution message.
/// Instruction operands and execution effects are independent of this choice.
/// Construct it with [`NoMessage::new`] or [`Default`] so callers do not depend
/// on its representation.
///
/// ```
/// use vihaco_runtime::{Message, NoMessage};
///
/// const INPUT: NoMessage = NoMessage::new();
/// fn accept_message<M: Message>(_: M) {}
/// accept_message(INPUT);
/// assert_eq!(INPUT, NoMessage::default());
/// ```
///
/// Ordinary unit values are not interchangeable with this message:
///
/// ```compile_fail,E0308
/// use vihaco_runtime::NoMessage;
/// let message: NoMessage = ();
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoMessage {
    _private: (),
}

impl NoMessage {
    /// Construct an empty execution message.
    pub const fn new() -> Self {
        Self { _private: () }
    }
}

impl crate::Message for NoMessage {}
