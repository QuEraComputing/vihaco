use vihaco::value::Value;
use vihaco::{
    frame::Frame,
    traits::{FrameMemory, StackFrame, StackMemory},
};

#[derive(Debug, Clone, Default)]
pub struct CPU {
    pub(crate) frames: Vec<Frame>,
    pub(crate) heap: Vec<Vec<Value>>,
    pub(crate) stack: Vec<Value>,
    pub(crate) span: (u32, u32, u32),
    pub(crate) pending_pc: Option<u32>,
    pub(crate) current_pc: u32,
    pub(crate) return_values: Vec<Value>,
}

impl StackMemory for CPU {
    type Value = Value;

    fn stack(&self) -> &Vec<Self::Value> {
        &self.stack
    }

    fn stack_mut(&mut self) -> &mut Vec<Self::Value> {
        &mut self.stack
    }

    fn stack_is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    fn stack_len(&self) -> usize {
        self.stack.len()
    }

    fn stack_get(&self, pos: usize) -> eyre::Result<&Self::Value> {
        self.stack
            .get(pos)
            .ok_or_else(|| eyre::eyre!("stack underflow"))
    }

    fn stack_get_mut(&mut self, pos: usize) -> eyre::Result<&mut Self::Value> {
        self.stack
            .get_mut(pos)
            .ok_or_else(|| eyre::eyre!("stack underflow"))
    }

    fn stack_pop(&mut self) -> eyre::Result<Self::Value> {
        self.stack
            .pop()
            .ok_or_else(|| eyre::eyre!("stack underflow"))
    }

    fn stack_push<T: Into<Self::Value>>(&mut self, v: T) {
        self.stack.push(v.into());
    }
}

impl StackFrame for CPU {
    fn get_frame(&self) -> eyre::Result<&Frame> {
        self.frames
            .last()
            .ok_or_else(|| eyre::eyre!("no current frame"))
    }

    fn get_frame_mut(&mut self) -> eyre::Result<&mut Frame> {
        self.frames
            .last_mut()
            .ok_or_else(|| eyre::eyre!("no current frame"))
    }

    fn push_frame(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    fn pop_frame(&mut self) -> eyre::Result<Frame> {
        self.frames
            .pop()
            .ok_or_else(|| eyre::eyre!("no frame to pop"))
    }
}

impl FrameMemory for CPU {
    fn frame_base(&self) -> eyre::Result<usize> {
        self.get_frame().map(|f| f.base)
    }

    fn get_local(&self, index: usize) -> eyre::Result<&Self::Value> {
        let base = self.frame_base()?;
        self.stack
            .get(base + index)
            .ok_or_else(|| eyre::eyre!("local index out of bounds"))
    }

    // TODO, this is wrong to extend the vector, because the stack is push from back, so it should be reversed
    fn get_local_mut(&mut self, index: usize) -> eyre::Result<&mut Self::Value> {
        let base = self.frame_base()?;
        let idx = base + index;
        let len = self.stack.len();

        if len <= idx {
            self.stack.resize(idx + 1, Value::Undefined);
        }
        self.stack
            .get_mut(idx)
            .ok_or_else(|| eyre::eyre!("Invalid local address at {:?}, stack size: {:?}", idx, len))
    }
}

impl CPU {
    pub fn push_heap_object(&mut self, values: Vec<Value>) -> u32 {
        let id = self.heap.len() as u32;
        self.heap.push(values);
        id
    }

    pub fn heap_object(&self, id: u32) -> eyre::Result<&[Value]> {
        self.heap
            .get(id as usize)
            .map(Vec::as_slice)
            .ok_or_else(|| eyre::eyre!("invalid heap object id {}", id))
    }

    pub fn take_pending_pc(&mut self) -> Option<u32> {
        self.pending_pc.take()
    }

    pub fn set_pending_pc(&mut self, pc: u32) {
        self.pending_pc = Some(pc);
    }

    pub fn clear_pending_pc(&mut self) {
        self.pending_pc = None;
    }

    pub fn set_current_pc(&mut self, pc: u32) {
        self.current_pc = pc;
    }

    pub fn return_values(&self) -> &[Value] {
        &self.return_values
    }

    pub fn set_return_values(&mut self, values: Vec<Value>) {
        self.return_values = values;
    }
}
