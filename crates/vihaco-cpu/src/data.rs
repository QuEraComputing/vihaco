// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::instruction::{SurfaceType, SurfaceValue};
use vihaco::value::Value;
use vihaco::{
    frame::Frame,
    traits::{FrameMemory, StackFrame, StackMemory},
};
use vihaco_parser::Ident;

vihaco::component! {
    #[derive(Default, Debug)]
    pub component CPU {
        pub(crate) frames: Vec<Frame>,
        pub(crate) heap: Heap,
        pub(crate) stack: Vec<Value>,
        pub(crate) span: (u32, u32, u32),
        pub(crate) pending_pc: Option<u32>,
        pub(crate) current_pc: u32,
        pub(crate) return_values: Vec<Value>,
    }

    type Type = vihaco::Type;
    value Value = vihaco::Value;

    instruction {
        #[pattern = "'span $0 $1 $2"]
        Span(u32, u32, u32),

        #[pattern = "'label `@` $0"]
        Label(Ident),

        #[pattern = "'func_start"]
        FunctionStart,

        #[pattern = "'func_end"]
        FunctionEnd,

        Breakpoint,

        #[pattern = "'br `@` $0"]
        Branch(Ident => u32),

        #[pattern = "'cond_br `@` $0 `,` `@` $1"]
        ConditionalBranch(Ident => u32, Ident => u32),

        #[pattern = "'ret $0"]
        Return(u32),

        #[pattern = "'call_indirect"]
        IndirectCall,

        Call(u32, Ident => u32),

        Halt,

        Print,

        Load(SurfaceType => Type, u32),

        Store(SurfaceType => Type, u32),

        Dup,

        #[pattern = "'heap_alloc $0"]
        HeapAlloc(u32),

        #[pattern = "'get_item"]
        GetItem,

        #[pattern = "'heap_dealloc"]
        HeapDealloc,

        Const(SurfaceType => Type, SurfaceValue => Value),

        Add(SurfaceType => Type),

        Sub(SurfaceType => Type),

        Mul(SurfaceType => Type),

        Div(SurfaceType => Type),

        Rem(SurfaceType => Type),

        Neg(SurfaceType => Type),

        Shl(SurfaceType => Type),

        Shr(SurfaceType => Type),

        Rol(SurfaceType => Type),

        Ror(SurfaceType => Type),

        #[pattern = "'bitand $0"]
        BitAnd(SurfaceType => Type),

        #[pattern = "'bitor $0"]
        BitOr(SurfaceType => Type),

        #[pattern = "'bitxor $0"]
        BitXor(SurfaceType => Type),

        Not,

        And,

        Or,

        Xor,

        Eq(SurfaceType => Type),
        Ne(SurfaceType => Type),
        Lt(SurfaceType => Type),
        Gt(SurfaceType => Type),
        Le(SurfaceType => Type),
        Ge(SurfaceType => Type),
    }
}

pub use cpu::CPU;
pub use cpu::runtime::Instruction as RuntimeInstruction;
pub use cpu::syntax::Instruction as SurfaceInstruction;

type HeapSlot = Option<Box<[Value]>>;

#[derive(Debug, Clone, Default)]
pub struct Heap {
    slots: Vec<HeapSlot>,
    free_list: Vec<u32>,
}

impl Heap {
    pub fn alloc(&mut self, values: Box<[Value]>) -> u32 {
        if let Some(id) = self.free_list.pop() {
            self.slots[id as usize] = Some(values);
            id
        } else {
            let id = self.slots.len() as u32;
            self.slots.push(Some(values));
            id
        }
    }

    pub fn dealloc(&mut self, id: u32) -> eyre::Result<()> {
        match self.slots.get_mut(id as usize) {
            Some(slot @ Some(_)) => {
                *slot = None;
                self.free_list.push(id);
                Ok(())
            }
            Some(None) => Err(eyre::eyre!(
                "double-free: heap object {} already deallocated",
                id
            )),
            None => Err(eyre::eyre!("invalid heap object id {}", id)),
        }
    }

    pub fn get(&self, id: u32) -> eyre::Result<&[Value]> {
        match self.slots.get(id as usize) {
            Some(Some(v)) => Ok(v),
            Some(None) => Err(eyre::eyre!("heap object {} has been deallocated", id)),
            None => Err(eyre::eyre!("invalid heap object id {}", id)),
        }
    }

    pub fn clear(&mut self) {
        self.slots.clear();
        self.free_list.clear();
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
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
    pub fn push_heap_object(&mut self, values: Box<[Value]>) -> u32 {
        self.heap.alloc(values)
    }

    pub fn heap_object(&self, id: u32) -> eyre::Result<&[Value]> {
        self.heap.get(id)
    }

    pub fn dealloc_heap_object(&mut self, id: u32) -> eyre::Result<()> {
        self.heap.dealloc(id)
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
