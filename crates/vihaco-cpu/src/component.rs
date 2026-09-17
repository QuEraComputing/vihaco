// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::data::CPU;
use crate::word::*;
use crate::StepOutcome;
use crate::Word;
use eyre::Result;
use vihaco::{frame::Frame, traits::*};

trait StackOps {
    fn remove_range(&mut self, start: usize, end: usize) -> Vec<Word>;
    unsafe fn replace_top_two_with<F>(&mut self, operation: F)
    where
        F: FnOnce(Word, Word) -> Word;
    unsafe fn try_replace_top_two_with<F, E>(&mut self, operation: F) -> Result<(), E>
    where
        F: FnOnce(Word, Word) -> Result<Word, E>;
}

impl StackOps for Vec<Word> {
    fn remove_range(&mut self, start: usize, end: usize) -> Vec<Word> {
        self.drain(start..end).collect()
    }

    unsafe fn replace_top_two_with<F>(&mut self, operation: F)
    where
        F: FnOnce(Word, Word) -> Word,
    {
        let rhs = self
            .pop()
            .expect("verified bytecode guarantees two stack values");
        let lhs = self
            .pop()
            .expect("verified bytecode guarantees two stack values");
        self.push(operation(lhs, rhs));
    }

    unsafe fn try_replace_top_two_with<F, E>(&mut self, operation: F) -> Result<(), E>
    where
        F: FnOnce(Word, Word) -> Result<Word, E>,
    {
        let rhs = self
            .pop()
            .expect("verified bytecode guarantees two stack values");
        let lhs = self
            .pop()
            .expect("verified bytecode guarantees two stack values");
        self.push(operation(lhs, rhs)?);
        Ok(())
    }
}

impl Reset for CPU {
    fn reset(&mut self) {
        self.frames.clear();
        self.heap.clear();
        self.stack.clear();
        self.span = (0, 0, 0);
        self.pending_pc = Option::None;
        self.current_pc = 0;
        self.return_values.clear();
    }
}

use crate::data::cpu_dialect::*;

impl vihaco::Execute<Span> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &Span) -> Self::Effect {
        self.clear_pending_pc();
        self.span = (instruction.0, instruction.1, instruction.2);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<Label> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &Label) -> Self::Effect {
        self.clear_pending_pc();
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<FunctionStart> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &FunctionStart) -> Self::Effect {
        self.clear_pending_pc();
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<FunctionEnd> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &FunctionEnd) -> Self::Effect {
        self.clear_pending_pc();
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<Breakpoint> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &Breakpoint) -> Self::Effect {
        self.clear_pending_pc();
        Ok(StepOutcome::Breakpoint)
    }
}

impl vihaco::Execute<Branch> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &Branch) -> Self::Effect {
        self.clear_pending_pc();
        self.set_pending_pc(instruction.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ConditionalBranch> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(
        &mut self,
        _message: Self::Message,
        instruction: &ConditionalBranch,
    ) -> Self::Effect {
        self.clear_pending_pc();
        let cond = self.stack_pop()?;
        match canonical_bool(cond)? {
            true => {
                self.set_pending_pc(instruction.0);
                Ok(StepOutcome::Continue)
            }
            false => {
                self.set_pending_pc(instruction.1);
                Ok(StepOutcome::Continue)
            }
        }
    }
}

impl vihaco::Execute<Return> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &Return) -> Self::Effect {
        self.clear_pending_pc();
        let frame = self.pop_frame()?;
        let frame_len = self
            .stack
            .len()
            .checked_sub(frame.base)
            .ok_or_else(|| eyre::eyre!("frame base out of bounds"))?;
        if frame_len < instruction.0 as usize {
            return Err(eyre::eyre!("not enough values to return"));
        }
        let top = self.stack.len() - instruction.0 as usize;
        let return_values = self.stack.as_slice()[top..].to_vec();
        self.stack.remove_range(frame.base, top);
        if self.get_frame().is_err() {
            self.set_return_values(return_values);
            Ok(StepOutcome::Return)
        } else {
            self.set_pending_pc(frame.ret_pc);
            Ok(StepOutcome::Continue)
        }
    }
}

impl vihaco::Execute<IndirectCall> for CPU {
    type Message = FunctionInfo;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, message: Self::Message, _instruction: &IndirectCall) -> Self::Effect {
        self.clear_pending_pc();
        self.stack_push(message.arity);
        self.stack_push(message.start_address);
        let target: u32 = self.stack_pop()?.try_into()?;
        let arity: u32 = self.stack_pop()?.try_into()?;
        let f = decode_function_ref(self.stack_pop()?);
        if self.stack.len() < arity as usize {
            return Err(eyre::eyre!(
                "not enough arguments on stack to call function"
            ));
        }
        let base = self.stack.len() - arity as usize;
        self.push_frame(Frame {
            base,
            span: self.span,
            function: Some(f as usize),
            ret_pc: self.current_pc + 1,
        });
        self.set_pending_pc(target);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<Call> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &Call) -> Self::Effect {
        self.clear_pending_pc();
        if self.stack.len() < instruction.0 as usize {
            return Err(eyre::eyre!(
                "not enough arguments on stack to call function"
            ));
        }
        let base = self.stack.len() - instruction.0 as usize;
        self.push_frame(Frame {
            base,
            span: self.span,
            function: Option::None,
            ret_pc: self.current_pc + 1,
        });
        self.set_pending_pc(instruction.1);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<Halt> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &Halt) -> Self::Effect {
        self.clear_pending_pc();
        Ok(StepOutcome::Halt)
    }
}

impl vihaco::Execute<crate::data::cpu_dialect::Print> for CPU {
    type Message = Print;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(
        &mut self,
        message: Self::Message,
        _instruction: &crate::data::cpu_dialect::Print,
    ) -> Self::Effect {
        self.clear_pending_pc();
        self.stack_pop()?;
        drop(message.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LoadI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &LoadI32) -> Self::Effect {
        self.clear_pending_pc();
        let value = self.get_local(instruction.0 as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LoadI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &LoadI64) -> Self::Effect {
        self.clear_pending_pc();
        let value = self.get_local(instruction.0 as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LoadU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &LoadU32) -> Self::Effect {
        self.clear_pending_pc();
        let value = self.get_local(instruction.0 as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LoadU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &LoadU64) -> Self::Effect {
        self.clear_pending_pc();
        let value = self.get_local(instruction.0 as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LoadF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &LoadF32) -> Self::Effect {
        self.clear_pending_pc();
        let value = self.get_local(instruction.0 as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LoadF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &LoadF64) -> Self::Effect {
        self.clear_pending_pc();
        let value = self.get_local(instruction.0 as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LoadBool> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &LoadBool) -> Self::Effect {
        self.clear_pending_pc();
        let value = self.get_local(instruction.0 as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<StoreI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &StoreI32) -> Self::Effect {
        self.clear_pending_pc();
        let v: Word = self.stack_pop()?;
        log::debug!("store value {:?} at addr {}", v, instruction.0);
        *self.get_local_mut(instruction.0 as usize)? = v;
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<StoreI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &StoreI64) -> Self::Effect {
        self.clear_pending_pc();
        let v: Word = self.stack_pop()?;
        log::debug!("store value {:?} at addr {}", v, instruction.0);
        *self.get_local_mut(instruction.0 as usize)? = v;
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<StoreU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &StoreU32) -> Self::Effect {
        self.clear_pending_pc();
        let v: Word = self.stack_pop()?;
        log::debug!("store value {:?} at addr {}", v, instruction.0);
        *self.get_local_mut(instruction.0 as usize)? = v;
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<StoreU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &StoreU64) -> Self::Effect {
        self.clear_pending_pc();
        let v: Word = self.stack_pop()?;
        log::debug!("store value {:?} at addr {}", v, instruction.0);
        *self.get_local_mut(instruction.0 as usize)? = v;
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<StoreF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &StoreF32) -> Self::Effect {
        self.clear_pending_pc();
        let v: Word = self.stack_pop()?;
        log::debug!("store value {:?} at addr {}", v, instruction.0);
        *self.get_local_mut(instruction.0 as usize)? = v;
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<StoreF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &StoreF64) -> Self::Effect {
        self.clear_pending_pc();
        let v: Word = self.stack_pop()?;
        log::debug!("store value {:?} at addr {}", v, instruction.0);
        *self.get_local_mut(instruction.0 as usize)? = v;
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<StoreBool> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &StoreBool) -> Self::Effect {
        self.clear_pending_pc();
        let v: Word = self.stack_pop()?;
        log::debug!("store value {:?} at addr {}", v, instruction.0);
        *self.get_local_mut(instruction.0 as usize)? = v;
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<Dup> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &Dup) -> Self::Effect {
        self.clear_pending_pc();
        let v = *self.stack_top()?;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<HeapAlloc> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &HeapAlloc) -> Self::Effect {
        self.clear_pending_pc();
        let n = instruction.0 as usize;
        if self.stack.len() < n {
            return Err(eyre::eyre!("stack underflow"));
        }
        let start = self.stack.len() - n;
        let values: Box<[Word]> = self.stack.remove_range(start, self.stack.len()).into();
        let heap_id = self.push_heap_object(values);
        self.stack_push(encode_heap_ref(heap_id));
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GetItem> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GetItem) -> Self::Effect {
        self.clear_pending_pc();
        let index = match decode_i64(self.stack_pop()?) {
            index if index >= 0 => usize::try_from(index)
                .map_err(|_| eyre::eyre!("heap index {} does not fit in usize", index))?,
            index => {
                return Err(eyre::eyre!(
                    "heap index must be non-negative, got {}",
                    index
                ));
            }
        };
        let heap_id = decode_heap_ref(self.stack_pop()?);
        let value = *self
            .heap_object(heap_id)?
            .get(index)
            .ok_or_else(|| eyre::eyre!("heap index {} out of bounds", index))?;
        self.stack_push(value);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<HeapDealloc> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &HeapDealloc) -> Self::Effect {
        self.clear_pending_pc();
        let id = decode_heap_ref(self.stack_pop()?);
        self.dealloc_heap_object(id)?;
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ConstI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &ConstI32) -> Self::Effect {
        self.clear_pending_pc();
        self.stack.push(instruction.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ConstI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &ConstI64) -> Self::Effect {
        self.clear_pending_pc();
        self.stack.push(instruction.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ConstU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &ConstU32) -> Self::Effect {
        self.clear_pending_pc();
        self.stack.push(instruction.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ConstU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &ConstU64) -> Self::Effect {
        self.clear_pending_pc();
        self.stack.push(instruction.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ConstF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &ConstF32) -> Self::Effect {
        self.clear_pending_pc();
        self.stack.push(instruction.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ConstF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &ConstF64) -> Self::Effect {
        self.clear_pending_pc();
        self.stack.push(instruction.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ConstBool> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &ConstBool) -> Self::Effect {
        self.clear_pending_pc();
        self.stack.push(instruction.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ConstString> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &ConstString) -> Self::Effect {
        self.clear_pending_pc();
        self.stack.push(instruction.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ConstFunctionRef> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &ConstFunctionRef) -> Self::Effect {
        self.clear_pending_pc();
        self.stack.push(instruction.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ConstHeapRef> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, instruction: &ConstHeapRef) -> Self::Effect {
        self.clear_pending_pc();
        self.stack.push(instruction.0);
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<AddI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &AddI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i32(decode_i32(lhs).wrapping_add(decode_i32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<AddF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &AddF32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_f32(decode_f32(lhs) + decode_f32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<AddI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &AddI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i64(decode_i64(lhs).wrapping_add(decode_i64(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<AddU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &AddU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u32(decode_u32(lhs).wrapping_add(decode_u32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<AddU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &AddU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u64(decode_u64(lhs).wrapping_add(decode_u64(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<AddF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &AddF64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_f64(decode_f64(lhs) + decode_f64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<SubI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &SubI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i32(decode_i32(lhs).wrapping_sub(decode_i32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<SubI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &SubI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i64(decode_i64(lhs).wrapping_sub(decode_i64(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<SubU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &SubU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u32(decode_u32(lhs).wrapping_sub(decode_u32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<SubU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &SubU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u64(decode_u64(lhs).wrapping_sub(decode_u64(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<SubF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &SubF32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_f32(decode_f32(lhs) - decode_f32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<SubF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &SubF64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_f64(decode_f64(lhs) - decode_f64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<MulI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &MulI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i32(decode_i32(lhs).wrapping_mul(decode_i32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<MulI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &MulI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i64(decode_i64(lhs).wrapping_mul(decode_i64(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<MulU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &MulU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u32(decode_u32(lhs).wrapping_mul(decode_u32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<MulU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &MulU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u64(decode_u64(lhs).wrapping_mul(decode_u64(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<MulF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &MulF32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_f32(decode_f32(lhs) * decode_f32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<MulF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &MulF64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_f64(decode_f64(lhs) * decode_f64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<DivI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &DivI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.try_replace_top_two_with(|lhs, rhs| {
                decode_i32(lhs)
                    .checked_div(decode_i32(rhs))
                    .ok_or_else(|| eyre::eyre!("integer division error"))
                    .map(encode_i32)
            })?;
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<DivI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &DivI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.try_replace_top_two_with(|lhs, rhs| {
                decode_i64(lhs)
                    .checked_div(decode_i64(rhs))
                    .ok_or_else(|| eyre::eyre!("integer division error"))
                    .map(encode_i64)
            })?;
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<DivU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &DivU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.try_replace_top_two_with(|lhs, rhs| {
                decode_u32(lhs)
                    .checked_div(decode_u32(rhs))
                    .ok_or_else(|| eyre::eyre!("integer division error"))
                    .map(encode_u32)
            })?;
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<DivU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &DivU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.try_replace_top_two_with(|lhs, rhs| {
                decode_u64(lhs)
                    .checked_div(decode_u64(rhs))
                    .ok_or_else(|| eyre::eyre!("integer division error"))
                    .map(encode_u64)
            })?;
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<DivF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &DivF32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_f32(decode_f32(lhs) / decode_f32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<DivF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &DivF64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_f64(decode_f64(lhs) / decode_f64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RemI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RemI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.try_replace_top_two_with(|lhs, rhs| {
                decode_i32(lhs)
                    .checked_rem(decode_i32(rhs))
                    .ok_or_else(|| eyre::eyre!("integer remainder error"))
                    .map(encode_i32)
            })?;
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RemI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RemI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.try_replace_top_two_with(|lhs, rhs| {
                decode_i64(lhs)
                    .checked_rem(decode_i64(rhs))
                    .ok_or_else(|| eyre::eyre!("integer remainder error"))
                    .map(encode_i64)
            })?;
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RemU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RemU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.try_replace_top_two_with(|lhs, rhs| {
                decode_u32(lhs)
                    .checked_rem(decode_u32(rhs))
                    .ok_or_else(|| eyre::eyre!("integer remainder error"))
                    .map(encode_u32)
            })?;
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RemU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RemU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.try_replace_top_two_with(|lhs, rhs| {
                decode_u64(lhs)
                    .checked_rem(decode_u64(rhs))
                    .ok_or_else(|| eyre::eyre!("integer remainder error"))
                    .map(encode_u64)
            })?;
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RemF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RemF32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_f32(decode_f32(lhs) % decode_f32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RemF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RemF64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_f64(decode_f64(lhs) % decode_f64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<NegI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &NegI32) -> Self::Effect {
        self.clear_pending_pc();
        let value = decode_i32(self.stack_pop()?).wrapping_neg();
        self.stack_push(encode_i32(value));
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<NegI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &NegI64) -> Self::Effect {
        self.clear_pending_pc();
        let value = decode_i64(self.stack_pop()?).wrapping_neg();
        self.stack_push(encode_i64(value));
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<NegF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &NegF32) -> Self::Effect {
        self.clear_pending_pc();
        let value = -decode_f32(self.stack_pop()?);
        self.stack_push(encode_f32(value));
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<NegF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &NegF64) -> Self::Effect {
        self.clear_pending_pc();
        let value = -decode_f64(self.stack_pop()?);
        self.stack_push(encode_f64(value));
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ShlI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &ShlI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i32(decode_i32(lhs).wrapping_shl(decode_u32(rhs) & 31))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ShlI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &ShlI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i64(decode_i64(lhs).wrapping_shl(decode_u32(rhs) & 63))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ShlU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &ShlU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u32(decode_u32(lhs).wrapping_shl(decode_u32(rhs) & 31))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ShlU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &ShlU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u64(decode_u64(lhs).wrapping_shl(decode_u32(rhs) & 63))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ShrI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &ShrI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i32(decode_i32(lhs).wrapping_shr(decode_u32(rhs) & 31))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ShrI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &ShrI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i64(decode_i64(lhs).wrapping_shr(decode_u32(rhs) & 63))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ShrU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &ShrU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u32(decode_u32(lhs).wrapping_shr(decode_u32(rhs) & 31))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<ShrU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &ShrU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u64(decode_u64(lhs).wrapping_shr(decode_u32(rhs) & 63))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RolI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RolI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i32(decode_i32(lhs).rotate_left(decode_u32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RolI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RolI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i64(decode_i64(lhs).rotate_left(decode_u32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RolU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RolU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u32(decode_u32(lhs).rotate_left(decode_u32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RolU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RolU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u64(decode_u64(lhs).rotate_left(decode_u32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RorI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RorI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i32(decode_i32(lhs).rotate_right(decode_u32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RorI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RorI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_i64(decode_i64(lhs).rotate_right(decode_u32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RorU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RorU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u32(decode_u32(lhs).rotate_right(decode_u32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<RorU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &RorU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack.replace_top_two_with(|lhs, rhs| {
                encode_u64(decode_u64(lhs).rotate_right(decode_u32(rhs)))
            });
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitAndI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitAndI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_i32(decode_i32(lhs) & decode_i32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitAndI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitAndI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_i64(decode_i64(lhs) & decode_i64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitAndU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitAndU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_u32(decode_u32(lhs) & decode_u32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitAndU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitAndU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_u64(decode_u64(lhs) & decode_u64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitOrI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitOrI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_i32(decode_i32(lhs) | decode_i32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitOrI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitOrI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_i64(decode_i64(lhs) | decode_i64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitOrU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitOrU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_u32(decode_u32(lhs) | decode_u32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitOrU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitOrU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_u64(decode_u64(lhs) | decode_u64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitXorI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitXorI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_i32(decode_i32(lhs) ^ decode_i32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitXorI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitXorI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_i64(decode_i64(lhs) ^ decode_i64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitXorU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitXorU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_u32(decode_u32(lhs) ^ decode_u32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<BitXorU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &BitXorU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_u64(decode_u64(lhs) ^ decode_u64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<Not> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &Not) -> Self::Effect {
        self.clear_pending_pc();
        let value = !canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(value));
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<And> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &And) -> Self::Effect {
        self.clear_pending_pc();
        let rhs = canonical_bool(self.stack_pop()?)?;
        let lhs = canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(lhs && rhs));
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<Or> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &Or) -> Self::Effect {
        self.clear_pending_pc();
        let rhs = canonical_bool(self.stack_pop()?)?;
        let lhs = canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(lhs || rhs));
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<Xor> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &Xor) -> Self::Effect {
        self.clear_pending_pc();
        let rhs = canonical_bool(self.stack_pop()?)?;
        let lhs = canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(lhs ^ rhs));
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<EqI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &EqI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i32(lhs) == decode_i32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<EqI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &EqI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i64(lhs) == decode_i64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<EqU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &EqU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u32(lhs) == decode_u32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<EqU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &EqU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u64(lhs) == decode_u64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<EqF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &EqF32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f32(lhs) == decode_f32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<EqF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &EqF64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f64(lhs) == decode_f64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<NeI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &NeI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i32(lhs) != decode_i32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<NeI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &NeI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i64(lhs) != decode_i64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<NeU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &NeU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u32(lhs) != decode_u32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<NeU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &NeU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u64(lhs) != decode_u64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<NeF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &NeF32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f32(lhs) != decode_f32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<NeF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &NeF64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f64(lhs) != decode_f64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LtI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LtI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i32(lhs) < decode_i32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LtI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LtI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i64(lhs) < decode_i64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LtU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LtU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u32(lhs) < decode_u32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LtU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LtU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u64(lhs) < decode_u64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LtF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LtF32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f32(lhs) < decode_f32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LtF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LtF64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f64(lhs) < decode_f64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GtI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GtI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i32(lhs) > decode_i32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GtI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GtI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i64(lhs) > decode_i64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GtU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GtU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u32(lhs) > decode_u32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GtU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GtU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u64(lhs) > decode_u64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GtF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GtF32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f32(lhs) > decode_f32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GtF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GtF64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f64(lhs) > decode_f64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LeI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LeI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i32(lhs) <= decode_i32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LeI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LeI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i64(lhs) <= decode_i64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LeU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LeU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u32(lhs) <= decode_u32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LeU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LeU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u64(lhs) <= decode_u64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LeF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LeF32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f32(lhs) <= decode_f32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<LeF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &LeF64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f64(lhs) <= decode_f64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GeI32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GeI32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i32(lhs) >= decode_i32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GeI64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GeI64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_i64(lhs) >= decode_i64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GeU32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GeU32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u32(lhs) >= decode_u32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GeU64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GeU64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_u64(lhs) >= decode_u64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GeF32> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GeF32) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f32(lhs) >= decode_f32(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

impl vihaco::Execute<GeF64> for CPU {
    type Message = None;
    type Effect = Result<StepOutcome>;

    #[inline(never)]
    fn execute(&mut self, _message: Self::Message, _instruction: &GeF64) -> Self::Effect {
        self.clear_pending_pc();
        unsafe {
            self.stack
                .replace_top_two_with(|lhs, rhs| encode_bool(decode_f64(lhs) >= decode_f64(rhs)));
        }
        Ok(StepOutcome::Continue)
    }
}

#[derive(Debug, Clone, PartialEq, vihaco::Message)]
pub struct None;

#[derive(Debug, Clone, PartialEq, vihaco::Message)]
pub struct FunctionInfo {
    pub arity: u32,
    pub start_address: u32,
}

#[derive(Debug, Clone, PartialEq, vihaco::Message)]
pub struct Print(pub String);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::cpu_dialect::{
        AddI32, Branch, ConstI64, IndirectCall, Print as PrintInstruction, Return,
    };
    use vihaco::{frame::Frame, Execute};

    #[test]
    fn execute_const_pushes_value() {
        let mut cpu = CPU::default();
        let outcome = Execute::execute(&mut cpu, None, &ConstI64(encode_i64(7))).unwrap();
        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.stack(), &vec![encode_i64(7)]);
    }

    #[test]
    fn execute_branch_sets_pending_pc() {
        let mut cpu = CPU::default();
        let outcome = Execute::execute(&mut cpu, None, &Branch(9)).unwrap();
        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.take_pending_pc(), Some(9));
    }

    #[test]
    fn execute_return_stores_terminal_values() {
        let mut cpu = CPU::default();
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: Option::None,
            ret_pc: 0,
        });
        cpu.stack_push(encode_i64(7));

        let outcome = Execute::execute(&mut cpu, None, &Return(1)).unwrap();
        assert_eq!(outcome, StepOutcome::Return);
        assert_eq!(cpu.return_values(), &[encode_i64(7)]);
    }

    #[test]
    fn execute_indirect_call_consumes_function_info() {
        let mut cpu = CPU {
            current_pc: 10,
            ..Default::default()
        };
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: Option::None,
            ret_pc: 0,
        });
        cpu.stack_push(encode_function_ref(7));

        let outcome = Execute::execute(
            &mut cpu,
            FunctionInfo {
                arity: 0,
                start_address: 100,
            },
            &IndirectCall,
        )
        .unwrap();
        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.take_pending_pc(), Some(100));
        assert_eq!(cpu.frames[1].ret_pc, 11);
    }

    #[test]
    fn execute_print_consumes_message_and_stack_value() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i64(42));

        let outcome =
            Execute::execute(&mut cpu, super::Print("hello".into()), &PrintInstruction).unwrap();
        assert_eq!(outcome, StepOutcome::Continue);
        assert!(cpu.stack().is_empty());
    }

    #[test]
    fn execute_arithmetic_updates_stack() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i32(i32::MAX));
        cpu.stack_push(encode_i32(1));

        let outcome = Execute::execute(&mut cpu, None, &AddI32).unwrap();
        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.stack_pop().unwrap(), encode_i32(i32::MIN));
    }
}

fn canonical_bool(value: Word) -> Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(eyre::eyre!("invalid boolean word {}", other)),
    }
}
