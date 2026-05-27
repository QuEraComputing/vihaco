mod event_sink;
mod instruction;
mod machine;

pub use event_sink::EffectSink;
pub use instruction::{FromBytes, FromBytesWithOpcode, Instruction, OpCode, WriteBytes};
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
