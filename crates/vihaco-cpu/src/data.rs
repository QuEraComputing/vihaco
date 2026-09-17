// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{instruction::SurfaceValue, Word};
use vihaco::{
    frame::Frame,
    traits::{FrameMemory, StackFrame, StackMemory},
};
use vihaco_parser::Ident;

pub mod cpu_dialect {
    use super::Word;

    #[derive(Default, Debug, Clone, Copy)]
    pub struct Span(pub u32, pub u32, pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct Label(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct FunctionStart;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct FunctionEnd;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct Breakpoint;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct Branch(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ConditionalBranch(pub u32, pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct Return(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct IndirectCall;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct Call(pub u32, pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct Halt;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct Print;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LoadI32(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LoadI64(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LoadU32(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LoadU64(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LoadF32(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LoadF64(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LoadBool(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct StoreI32(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct StoreI64(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct StoreU32(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct StoreU64(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct StoreF32(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct StoreF64(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct StoreBool(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct Dup;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct HeapAlloc(pub u32);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GetItem;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct HeapDealloc;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ConstI32(pub Word);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ConstI64(pub Word);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ConstU32(pub Word);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ConstU64(pub Word);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ConstF32(pub Word);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ConstF64(pub Word);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ConstBool(pub Word);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ConstString(pub Word);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ConstFunctionRef(pub Word);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ConstHeapRef(pub Word);
    #[derive(Default, Debug, Clone, Copy)]
    pub struct AddI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct AddF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct AddI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct AddU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct AddU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct AddF64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct SubI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct SubI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct SubU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct SubU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct SubF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct SubF64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct MulI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct MulI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct MulU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct MulU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct MulF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct MulF64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct DivI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct DivI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct DivU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct DivU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct DivF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct DivF64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RemI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RemI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RemU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RemU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RemF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RemF64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct NegI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct NegI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct NegF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct NegF64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ShlI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ShlI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ShlU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ShlU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ShrI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ShrI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ShrU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ShrU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RolI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RolI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RolU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RolU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RorI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RorI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RorU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct RorU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitAndI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitAndI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitAndU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitAndU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitOrI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitOrI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitOrU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitOrU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitXorI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitXorI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitXorU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BitXorU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct Not;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct And;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct Or;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct Xor;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct EqI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct EqI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct EqU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct EqU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct EqF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct EqF64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct NeI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct NeI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct NeU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct NeU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct NeF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct NeF64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LtI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LtI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LtU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LtU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LtF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LtF64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GtI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GtI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GtU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GtU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GtF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GtF64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LeI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LeI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LeU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LeU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LeF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct LeF64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GeI32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GeI64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GeU32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GeU64;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GeF32;
    #[derive(Default, Debug, Clone, Copy)]
    pub struct GeF64;
}

vihaco::component! {
    #[derive(Default, Debug)]
    pub component CPU {
        pub(crate) frames: Vec<Frame>,
        pub(crate) heap: Heap,
        pub(crate) stack: Vec<Word>,
        pub(crate) span: (u32, u32, u32),
        pub(crate) pending_pc: Option<u32>,
        pub(crate) current_pc: u32,
        pub(crate) return_values: Vec<Word>,
    }

    type Type = vihaco::Type;
    value Word = crate::Word;

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

        #[pattern = "'load_i32 $0"]
        LoadI32(u32),
        #[pattern = "'load_i64 $0"]
        LoadI64(u32),
        #[pattern = "'load_u32 $0"]
        LoadU32(u32),
        #[pattern = "'load_u64 $0"]
        LoadU64(u32),
        #[pattern = "'load_f32 $0"]
        LoadF32(u32),
        #[pattern = "'load_f64 $0"]
        LoadF64(u32),
        #[pattern = "'load_bool $0"]
        LoadBool(u32),
        #[pattern = "'store_i32 $0"]
        StoreI32(u32),
        #[pattern = "'store_i64 $0"]
        StoreI64(u32),
        #[pattern = "'store_u32 $0"]
        StoreU32(u32),
        #[pattern = "'store_u64 $0"]
        StoreU64(u32),
        #[pattern = "'store_f32 $0"]
        StoreF32(u32),
        #[pattern = "'store_f64 $0"]
        StoreF64(u32),
        #[pattern = "'store_bool $0"]
        StoreBool(u32),

        Dup,

        #[pattern = "'heap_alloc $0"]
        HeapAlloc(u32),

        #[pattern = "'get_item"]
        GetItem,

        #[pattern = "'heap_dealloc"]
        HeapDealloc,

        #[pattern = "'const_i32 $0"]
        ConstI32(SurfaceValue => Word),
        #[pattern = "'const_i64 $0"]
        ConstI64(SurfaceValue => Word),
        #[pattern = "'const_u32 $0"]
        ConstU32(SurfaceValue => Word),
        #[pattern = "'const_u64 $0"]
        ConstU64(SurfaceValue => Word),
        #[pattern = "'const_f32 $0"]
        ConstF32(SurfaceValue => Word),
        #[pattern = "'const_f64 $0"]
        ConstF64(SurfaceValue => Word),
        #[pattern = "'const_bool $0"]
        ConstBool(SurfaceValue => Word),
        #[pattern = "'const_string $0"]
        ConstString(SurfaceValue => Word),
        #[pattern = "'const_fn_ref $0"]
        ConstFunctionRef(SurfaceValue => Word),
        #[pattern = "'const_heap_ref $0"]
        ConstHeapRef(SurfaceValue => Word),

        #[pattern = "'add_i32"]
        AddI32,
        #[pattern = "'add_f32"]
        AddF32,
        #[pattern = "'add_i64"]
        AddI64,
        #[pattern = "'add_u32"]
        AddU32,
        #[pattern = "'add_u64"]
        AddU64,
        #[pattern = "'add_f64"]
        AddF64,
        #[pattern = "'sub_i32"]
        SubI32,
        #[pattern = "'sub_i64"]
        SubI64,
        #[pattern = "'sub_u32"]
        SubU32,
        #[pattern = "'sub_u64"]
        SubU64,
        #[pattern = "'sub_f32"]
        SubF32,
        #[pattern = "'sub_f64"]
        SubF64,
        #[pattern = "'mul_i32"]
        MulI32,
        #[pattern = "'mul_i64"]
        MulI64,
        #[pattern = "'mul_u32"]
        MulU32,
        #[pattern = "'mul_u64"]
        MulU64,
        #[pattern = "'mul_f32"]
        MulF32,
        #[pattern = "'mul_f64"]
        MulF64,
        #[pattern = "'div_i32"]
        DivI32,
        #[pattern = "'div_i64"]
        DivI64,
        #[pattern = "'div_u32"]
        DivU32,
        #[pattern = "'div_u64"]
        DivU64,
        #[pattern = "'div_f32"]
        DivF32,
        #[pattern = "'div_f64"]
        DivF64,
        #[pattern = "'rem_i32"]
        RemI32,
        #[pattern = "'rem_i64"]
        RemI64,
        #[pattern = "'rem_u32"]
        RemU32,
        #[pattern = "'rem_u64"]
        RemU64,
        #[pattern = "'rem_f32"]
        RemF32,
        #[pattern = "'rem_f64"]
        RemF64,
        #[pattern = "'neg_i32"]
        NegI32,
        #[pattern = "'neg_i64"]
        NegI64,
        #[pattern = "'neg_f32"]
        NegF32,
        #[pattern = "'neg_f64"]
        NegF64,
        #[pattern = "'shl_i32"]
        ShlI32,
        #[pattern = "'shl_i64"]
        ShlI64,
        #[pattern = "'shl_u32"]
        ShlU32,
        #[pattern = "'shl_u64"]
        ShlU64,
        #[pattern = "'shr_i32"]
        ShrI32,
        #[pattern = "'shr_i64"]
        ShrI64,
        #[pattern = "'shr_u32"]
        ShrU32,
        #[pattern = "'shr_u64"]
        ShrU64,
        #[pattern = "'rol_i32"]
        RolI32,
        #[pattern = "'rol_i64"]
        RolI64,
        #[pattern = "'rol_u32"]
        RolU32,
        #[pattern = "'rol_u64"]
        RolU64,
        #[pattern = "'ror_i32"]
        RorI32,
        #[pattern = "'ror_i64"]
        RorI64,
        #[pattern = "'ror_u32"]
        RorU32,
        #[pattern = "'ror_u64"]
        RorU64,
        #[pattern = "'bitand_i32"]
        BitAndI32,
        #[pattern = "'bitand_i64"]
        BitAndI64,
        #[pattern = "'bitand_u32"]
        BitAndU32,
        #[pattern = "'bitand_u64"]
        BitAndU64,
        #[pattern = "'bitor_i32"]
        BitOrI32,
        #[pattern = "'bitor_i64"]
        BitOrI64,
        #[pattern = "'bitor_u32"]
        BitOrU32,
        #[pattern = "'bitor_u64"]
        BitOrU64,
        #[pattern = "'bitxor_i32"]
        BitXorI32,
        #[pattern = "'bitxor_i64"]
        BitXorI64,
        #[pattern = "'bitxor_u32"]
        BitXorU32,
        #[pattern = "'bitxor_u64"]
        BitXorU64,

        Not,

        And,

        Or,

        Xor,

        #[pattern = "'eq_i32"]
        EqI32,
        #[pattern = "'eq_i64"]
        EqI64,
        #[pattern = "'eq_u32"]
        EqU32,
        #[pattern = "'eq_u64"]
        EqU64,
        #[pattern = "'eq_f32"]
        EqF32,
        #[pattern = "'eq_f64"]
        EqF64,
        #[pattern = "'ne_i32"]
        NeI32,
        #[pattern = "'ne_i64"]
        NeI64,
        #[pattern = "'ne_u32"]
        NeU32,
        #[pattern = "'ne_u64"]
        NeU64,
        #[pattern = "'ne_f32"]
        NeF32,
        #[pattern = "'ne_f64"]
        NeF64,
        #[pattern = "'lt_i32"]
        LtI32,
        #[pattern = "'lt_i64"]
        LtI64,
        #[pattern = "'lt_u32"]
        LtU32,
        #[pattern = "'lt_u64"]
        LtU64,
        #[pattern = "'lt_f32"]
        LtF32,
        #[pattern = "'lt_f64"]
        LtF64,
        #[pattern = "'gt_i32"]
        GtI32,
        #[pattern = "'gt_i64"]
        GtI64,
        #[pattern = "'gt_u32"]
        GtU32,
        #[pattern = "'gt_u64"]
        GtU64,
        #[pattern = "'gt_f32"]
        GtF32,
        #[pattern = "'gt_f64"]
        GtF64,
        #[pattern = "'le_i32"]
        LeI32,
        #[pattern = "'le_i64"]
        LeI64,
        #[pattern = "'le_u32"]
        LeU32,
        #[pattern = "'le_u64"]
        LeU64,
        #[pattern = "'le_f32"]
        LeF32,
        #[pattern = "'le_f64"]
        LeF64,
        #[pattern = "'ge_i32"]
        GeI32,
        #[pattern = "'ge_i64"]
        GeI64,
        #[pattern = "'ge_u32"]
        GeU32,
        #[pattern = "'ge_u64"]
        GeU64,
        #[pattern = "'ge_f32"]
        GeF32,
        #[pattern = "'ge_f64"]
        GeF64,
    }
}

pub use cpu::runtime::Instruction as RuntimeInstruction;
pub use cpu::syntax::Instruction as SurfaceInstruction;
pub use cpu::CPU;

type HeapSlot = Option<Box<[Word]>>;

#[derive(Debug, Clone, Default)]
pub struct Heap {
    slots: Vec<HeapSlot>,
    free_list: Vec<u32>,
}

impl Heap {
    pub fn alloc(&mut self, values: Box<[Word]>) -> u32 {
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

    pub fn get(&self, id: u32) -> eyre::Result<&[Word]> {
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
    type Value = Word;

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
            self.stack.resize(idx + 1, 0);
        }
        self.stack
            .get_mut(idx)
            .ok_or_else(|| eyre::eyre!("Invalid local address at {:?}, stack size: {:?}", idx, len))
    }
}

impl CPU {
    pub fn push_heap_object(&mut self, values: Box<[Word]>) -> u32 {
        self.heap.alloc(values)
    }

    pub fn heap_object(&self, id: u32) -> eyre::Result<&[Word]> {
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

    pub fn return_values(&self) -> &[Word] {
        &self.return_values
    }

    pub fn set_return_values(&mut self, values: Vec<Word>) {
        self.return_values = values;
    }
}
