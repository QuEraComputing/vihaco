// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod encoding;
mod event_sink;
mod instruction;
mod machine;

pub use encoding::{FromBytes, FromBytesWithOpcode, FromText, WriteBytes};
pub use event_sink::EffectSink;
pub use instruction::{Instruction, OpCode};
pub use machine::{FrameMemory, GetProgramGlobal, ProgramCounter, StackFrame, StackMemory, Stdout};

pub trait Reset {
    /// reset the component state into initial state
    fn reset(&mut self);
}

impl<T: Reset> Reset for Vec<T> {
    fn reset(&mut self) {
        for item in self.iter_mut() {
            item.reset();
        }
    }
}
