// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use eyre::Result;
use std::ops::{
    Add as _, BitAnd as _, BitOr as _, BitXor as _, Div as _, Mul as _, Rem as _, Shl as _,
    Shr as _, Sub as _,
};

use crate::StepOutcome;
use crate::data::CPU;
use vihaco::program::{Type, Value};
use vihaco::{Execute, NoEffect, NoMessage, StepResult, Supply, frame::Frame, traits::*};

pub use crate::instruction::cpu::runtime::instruction::*;

impl Reset for CPU {
    fn reset(&mut self) {
        self.frames.clear();
        self.heap.clear();
        self.stack.clear();
        self.span = (0, 0, 0);
        self.pending_pc = None;
        self.current_pc = 0;
        self.return_values.clear();
    }
}

pub mod message {
    #[derive(Debug, Clone, PartialEq)]
    pub struct FunctionInfo {
        pub arity: u32,
        pub start_address: u32,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Print(pub String);
}

impl vihaco::Message for message::FunctionInfo {}
impl vihaco::Message for message::Print {}

#[derive(Debug, Clone, PartialEq)]
pub struct PrintEffect(pub String);

impl Supply<message::FunctionInfo> for CPU {
    type Fault = eyre::Report;

    fn supply(&mut self) -> Result<message::FunctionInfo, Self::Fault> {
        let start_address: u32 = self.stack_pop()?.try_into()?;
        let arity: u32 = self.stack_pop()?.try_into()?;
        Ok(message::FunctionInfo {
            arity,
            start_address,
        })
    }
}

impl Execute<Span> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &Span,
        _message: Self::Message,
    ) -> eyre::Result<StepResult<Self::Effect>> {
        self.span = (instruction.0, instruction.1, instruction.2);
        vihaco::complete!()
    }
}

impl Execute<Label> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &Label,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        vihaco::complete!()
    }
}

impl Execute<FunctionStart> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &FunctionStart,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        vihaco::complete!()
    }
}

impl Execute<FunctionEnd> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &FunctionEnd,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        vihaco::complete!()
    }
}

impl Execute<Breakpoint> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &Breakpoint,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        vihaco::complete!(StepOutcome::Breakpoint)
    }
}

impl Execute<Halt> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &Halt,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        vihaco::complete!(StepOutcome::Halt)
    }
}

impl Execute<Branch> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &Branch,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        self.set_pending_pc(instruction.0);
        vihaco::complete!(StepOutcome::Continue)
    }
}

impl Execute<ConditionalBranch> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &ConditionalBranch,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        let cond = self.stack.pop().ok_or(eyre::eyre!("stack underflow"))?;
        let outcome = match cond {
            Value::Bool(true) => {
                self.set_pending_pc(instruction.0);
                Ok(StepOutcome::Continue)
            }
            Value::Bool(false) => {
                self.set_pending_pc(instruction.1);
                Ok(StepOutcome::Continue)
            }
            _ => Err(eyre::eyre!("type error: expected bool on stack")),
        }?;
        vihaco::complete!(outcome)
    }
}

impl Execute<Return> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &Return,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        let frame = self.pop_frame()?;
        if self.stack.len() - frame.base < (instruction.0 as usize) {
            return Err(eyre::eyre!("not enough values to return"));
        }

        // Collect return values before truncating
        let top = self.stack.len() - instruction.0 as usize;
        let return_values: Vec<Value> = self.stack[top..].to_vec();
        self.stack.drain(frame.base..top);

        let outcome = if self.get_frame().is_err() {
            // No more frames - program is returning
            self.set_return_values(return_values);
            StepOutcome::Return
        } else {
            self.set_pending_pc(frame.ret_pc);
            StepOutcome::Continue
        };
        vihaco::complete!(outcome)
    }
}

impl Execute<Call> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &Call,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        if self.stack.len() < (instruction.0 as usize) {
            return Err(eyre::eyre!(
                "not enough arguments on stack to call function"
            ));
        }

        let base = self.stack.len() - (instruction.0 as usize);
        let frame = Frame {
            base,
            span: self.span,
            function: None,
            ret_pc: self.current_pc + 1,
        };
        self.push_frame(frame);
        self.set_pending_pc(instruction.1);
        vihaco::complete!(StepOutcome::Continue)
    }
}

impl Execute<IndirectCall> for CPU {
    type Message = message::FunctionInfo;
    type Effect = StepOutcome;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &IndirectCall,
        message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        self.stack_push(message.arity);
        self.stack_push(message.start_address);

        // Similar order to `Call`, but the target and function come from the
        // stack rather than the instruction and message respectively.
        let target: u32 = self.stack_pop()?.try_into()?;
        let arity: u32 = self.stack_pop()?.try_into()?;
        let function = self.stack_pop()?.get_function_ref()?;

        if self.stack.len() < arity as usize {
            return Err(eyre::eyre!(
                "not enough arguments on stack to call function"
            ));
        }

        let base = self.stack.len() - arity as usize;
        let frame = Frame {
            base,
            span: self.span,
            function: Some(function as usize),
            ret_pc: self.current_pc + 1,
        };
        self.push_frame(frame);
        self.set_pending_pc(target);
        vihaco::complete!(StepOutcome::Continue)
    }
}

impl Execute<Print> for CPU {
    type Message = message::Print;
    type Effect = PrintEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &Print,
        message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        vihaco::complete!(PrintEffect(message.0))
    }
}

impl Execute<Load> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &Load,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        // addr should be local to frame.
        let value = self.get_local(instruction.1 as usize)?;
        if value.type_of() != instruction.0 {
            return Err(eyre::eyre!(format!(
                "type error: expected {:?} at address {}, got {:?}",
                instruction.0,
                instruction.1,
                value.type_of()
            )));
        }
        self.stack_push(*value);
        vihaco::complete!()
    }
}

impl Execute<Store> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &Store,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        let v: Value = self.stack_pop()?;
        log::debug!("store value {:?} at addr {}", v, instruction.1);
        if !v.is_undefined() && v.type_of() != instruction.0 {
            return Err(eyre::eyre!("Type mismatch"));
        }
        *self.get_local_mut(instruction.1 as usize)? = v;
        vihaco::complete!()
    }
}

impl Execute<Dup> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &Dup,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        let v = *self.stack_top()?;
        self.stack.push(v);
        vihaco::complete!()
    }
}

impl Execute<HeapAlloc> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &HeapAlloc,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        let n: usize = instruction.0 as usize;
        if self.stack.len() < n {
            return Err(eyre::eyre!("stack underflow"));
        }
        let start = self.stack.len() - n;
        let values: Box<[Value]> = self.stack.drain(start..).collect();
        let heap_id = self.push_heap_object(values);
        self.stack_push(Value::HeapRef(heap_id));
        vihaco::complete!()
    }
}

impl Execute<GetItem> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &GetItem,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        let index = Self::heap_index(self.stack_pop()?)?;
        let heap_id = self.stack_pop()?.get_heap_ref()?;
        let value = *self
            .heap_object(heap_id)?
            .get(index)
            .ok_or_else(|| eyre::eyre!("heap index {} out of bounds", index))?;
        self.stack_push(value);
        vihaco::complete!()
    }
}

impl Execute<HeapDealloc> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &HeapDealloc,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        let heap_id = self.stack_pop()?.get_heap_ref()?;
        self.dealloc_heap_object(heap_id)?;
        vihaco::complete!()
    }
}

impl Execute<Const> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &Const,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        self.stack_push(instruction.0);
        vihaco::complete!()
    }
}

impl CPU {
    fn heap_index(value: Value) -> Result<usize> {
        match value {
            Value::U32(index) => Ok(index as usize),
            Value::U64(index) => usize::try_from(index)
                .map_err(|_| eyre::eyre!("heap index {} does not fit in usize", index)),
            Value::I64(index) if index >= 0 => usize::try_from(index)
                .map_err(|_| eyre::eyre!("heap index {} does not fit in usize", index)),
            Value::I64(index) => Err(eyre::eyre!(
                "heap index must be non-negative, got {}",
                index
            )),
            _ => Err(eyre::eyre!(
                "type error: expected integer heap index, got {:?}",
                value.type_of()
            )),
        }
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use vihaco::{Execute, Execution, NoMessage, frame::Frame, traits::StackMemory};

    #[test]
    fn const_executes_without_message() {
        let mut cpu = CPU::default();

        Execute::execute(&mut cpu, &Const(Value::I64(7)), NoMessage).unwrap();

        assert_eq!(cpu.stack(), &vec![Value::I64(7)]);
    }

    #[test]
    fn branch_updates_pending_program_counter_with_continue_effect() {
        let mut cpu = CPU::default();

        let branch = Execute::execute(&mut cpu, &Branch(9), NoMessage).unwrap();
        assert_eq!(
            vihaco::expect_exactly_one_effect(branch.effects).unwrap(),
            StepOutcome::Continue
        );
        assert_eq!(cpu.take_pending_pc(), Some(9));

        let halt = Execute::execute(&mut cpu, &Halt, NoMessage).unwrap();
        assert_eq!(
            vihaco::expect_exactly_one_effect(halt.effects).unwrap(),
            StepOutcome::Halt
        );
        assert_eq!(cpu.take_pending_pc(), None);
    }

    #[test]
    fn op_return_stores_terminal_values_in_runtime_state() {
        let mut cpu = CPU::default();
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });
        cpu.stack_push(Value::I64(7));

        let result = Execute::execute(&mut cpu, &Return(1), NoMessage).unwrap();

        assert_eq!(
            vihaco::expect_exactly_one_effect(result.effects).unwrap(),
            StepOutcome::Return
        );
        assert_eq!(cpu.return_values(), &[Value::I64(7)]);
    }

    #[test]
    fn op_return_restores_callers_pc() {
        let mut cpu = CPU {
            current_pc: 10,
            ..Default::default()
        };
        // Outer ("main") frame so the inner Return takes the Continue branch.
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });

        // Caller would be executing `call 0, 100` at some PC; op_call sets
        // pending_pc to the callee target.
        Execute::execute(&mut cpu, &Call(0, 100), NoMessage).unwrap();
        assert_eq!(cpu.take_pending_pc(), Some(100));
        assert_eq!(cpu.frames[1].ret_pc, 11);

        // Callee returns immediately. pending_pc should be restored to the
        // instruction after the call.
        let result = Execute::execute(&mut cpu, &Return(0), NoMessage).unwrap();
        assert_eq!(
            vihaco::expect_exactly_one_effect(result.effects).unwrap(),
            StepOutcome::Continue
        );
        assert_eq!(cpu.take_pending_pc(), Some(11),);
    }

    #[test]
    fn op_indirect_call_records_return_pc_after_call_site() {
        let mut cpu = CPU {
            current_pc: 10,
            ..Default::default()
        };
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });

        // Supply consumes target and arity, leaving FunctionRef on the stack.
        cpu.stack_push(Value::FunctionRef(7));
        cpu.stack_push(Value::U32(0));
        cpu.stack_push(Value::U32(100));

        let message = Supply::<message::FunctionInfo>::supply(&mut cpu).unwrap();
        Execute::execute(&mut cpu, &IndirectCall, message).unwrap();
        assert_eq!(cpu.take_pending_pc(), Some(100));
        assert_eq!(cpu.frames[1].ret_pc, 11);

        let result = Execute::execute(&mut cpu, &Return(0), NoMessage).unwrap();
        assert_eq!(
            vihaco::expect_exactly_one_effect(result.effects).unwrap(),
            StepOutcome::Continue
        );
        assert_eq!(cpu.take_pending_pc(), Some(11));
    }

    #[test]
    fn op_return_keeps_bottom_of_frame_when_callee_leaves_scratch() {
        let mut cpu = CPU::default();
        // Outer frame so Return takes the Continue branch.
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });

        // Simulate a callee frame holding [scratch_a, scratch_b, return_val]
        // where only `return_val` (the top) should survive `ret 1`.
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });
        cpu.stack_push(Value::I64(111)); // scratch — bottom of callee frame
        cpu.stack_push(Value::I64(222)); // scratch — middle
        cpu.stack_push(Value::I64(999)); // intended return value — top

        let result = Execute::execute(&mut cpu, &Return(1), NoMessage).unwrap();
        assert_eq!(
            vihaco::expect_exactly_one_effect(result.effects).unwrap(),
            StepOutcome::Continue
        );

        assert_eq!(cpu.stack(), &vec![Value::I64(999)],);
    }

    #[test]
    fn op_heap_alloc_preserves_natural_push_order_and_returns_heap_ref() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(10));
        cpu.stack_push(Value::I64(20));
        cpu.stack_push(Value::I64(30));

        let result = Execute::execute(&mut cpu, &HeapAlloc(3), NoMessage).unwrap();

        assert_eq!(result.execution, Execution::Complete);
        assert_eq!(cpu.stack(), &vec![Value::HeapRef(0)]);
        assert_eq!(
            cpu.heap.get(0).unwrap(),
            &[Value::I64(10), Value::I64(20), Value::I64(30)]
        );
    }

    #[test]
    fn op_heap_alloc_supports_empty_heap_objects() {
        let mut cpu = CPU::default();

        let result = Execute::execute(&mut cpu, &HeapAlloc(0), NoMessage).unwrap();

        assert_eq!(result.execution, Execution::Complete);
        assert_eq!(cpu.stack(), &vec![Value::HeapRef(0)]);
        assert_eq!(cpu.heap.get(0).unwrap(), &[] as &[Value]);
    }

    #[test]
    fn op_get_item_reads_heap_value() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(10));
        cpu.stack_push(Value::I64(20));
        cpu.stack_push(Value::I64(30));
        Execute::execute(&mut cpu, &HeapAlloc(3), NoMessage).unwrap();
        cpu.stack_push(Value::U32(1));

        let result = Execute::execute(&mut cpu, &GetItem, NoMessage).unwrap();

        assert_eq!(result.execution, Execution::Complete);
        assert_eq!(cpu.stack(), &vec![Value::I64(20)]);
    }

    #[test]
    fn op_get_item_rejects_non_heap_refs() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(7));
        cpu.stack_push(Value::U32(0));

        let err = Execute::execute(&mut cpu, &GetItem, NoMessage)
            .err()
            .unwrap();

        assert!(err.to_string().contains("HeapRef"));
    }

    #[test]
    fn op_get_item_rejects_invalid_heap_ids() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::HeapRef(99));
        cpu.stack_push(Value::U32(0));

        let err = Execute::execute(&mut cpu, &GetItem, NoMessage)
            .err()
            .unwrap();

        assert!(err.to_string().contains("heap"));
    }

    #[test]
    fn op_get_item_rejects_out_of_bounds_indices() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(10));
        Execute::execute(&mut cpu, &HeapAlloc(1), NoMessage).unwrap();
        cpu.stack_push(Value::U32(3));

        let err = Execute::execute(&mut cpu, &GetItem, NoMessage)
            .err()
            .unwrap();

        assert!(err.to_string().contains("index"));
    }

    #[test]
    fn reset_clears_heap_allocations() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(10));
        Execute::execute(&mut cpu, &HeapAlloc(1), NoMessage).unwrap();

        cpu.reset();

        assert!(cpu.heap.get(0).is_err());
        assert!(cpu.stack().is_empty());
    }

    #[test]
    fn supply_function_info_reads_arity_and_start_address() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::FunctionRef(7));
        cpu.stack_push(Value::U32(2));
        cpu.stack_push(Value::U32(42));

        let message = Supply::<message::FunctionInfo>::supply(&mut cpu).unwrap();

        assert_eq!(
            message,
            message::FunctionInfo {
                arity: 2,
                start_address: 42
            }
        );
        assert_eq!(cpu.stack(), &[Value::FunctionRef(7)]);
    }

    #[test]
    fn print_emits_print_effect_without_popping_stack() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(42));

        let result = Execute::execute(&mut cpu, &Print, message::Print("hello".into())).unwrap();

        assert_eq!(
            vihaco::expect_exactly_one_effect(result.effects).unwrap(),
            PrintEffect("hello".into())
        );
        assert_eq!(cpu.stack(), &[Value::I64(42)]);
    }

    #[test]
    fn op_heap_dealloc_marks_slot_dead() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(42));
        Execute::execute(&mut cpu, &HeapAlloc(1), NoMessage).unwrap();
        cpu.stack_push(Value::HeapRef(0));

        Execute::execute(&mut cpu, &HeapDealloc, NoMessage).unwrap();

        assert!(
            cpu.heap
                .get(0)
                .unwrap_err()
                .to_string()
                .contains("deallocated")
        );
    }

    #[test]
    fn op_heap_dealloc_slot_is_reused_on_next_alloc() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(1));
        Execute::execute(&mut cpu, &HeapAlloc(1), NoMessage).unwrap();
        Execute::execute(&mut cpu, &HeapDealloc, NoMessage).unwrap();

        cpu.stack_push(Value::I64(2));
        Execute::execute(&mut cpu, &HeapAlloc(1), NoMessage).unwrap();

        assert_eq!(cpu.stack(), &vec![Value::HeapRef(0)]);
        assert_eq!(cpu.heap.get(0).unwrap(), &[Value::I64(2)]);
    }

    #[test]
    fn op_heap_dealloc_rejects_double_free() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(1));
        Execute::execute(&mut cpu, &HeapAlloc(1), NoMessage).unwrap();
        cpu.stack_push(Value::HeapRef(0));
        Execute::execute(&mut cpu, &HeapDealloc, NoMessage).unwrap();

        cpu.stack_push(Value::HeapRef(0));
        let err = Execute::execute(&mut cpu, &HeapDealloc, NoMessage)
            .err()
            .unwrap();

        assert!(err.to_string().contains("double-free"));
    }

    #[test]
    fn op_heap_dealloc_rejects_invalid_id() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::HeapRef(99));

        let err = Execute::execute(&mut cpu, &HeapDealloc, NoMessage)
            .err()
            .unwrap();

        assert!(err.to_string().contains("invalid heap object id"));
    }

    #[test]
    fn reset_clears_free_list() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(1));
        Execute::execute(&mut cpu, &HeapAlloc(1), NoMessage).unwrap();
        cpu.stack_push(Value::HeapRef(0));
        Execute::execute(&mut cpu, &HeapDealloc, NoMessage).unwrap();

        cpu.reset();

        assert!(cpu.heap.get(0).is_err());
    }
}

#[cfg(test)]
mod execute_tests {
    use super::*;
    use vihaco::{Execute, Execution, NoEffect, NoMessage, frame::Frame, traits::StackMemory};

    fn execute<I>(cpu: &mut CPU, instruction: &I) -> eyre::Result<StepResult<NoEffect>>
    where
        CPU: Execute<I, Message = NoMessage, Effect = NoEffect, Fault = eyre::Report>,
    {
        Execute::execute(cpu, instruction, NoMessage)
    }

    #[test]
    fn const_pushes_value() {
        let mut cpu = CPU::default();

        execute(&mut cpu, &Const(Value::I64(7))).unwrap();

        assert_eq!(cpu.stack(), &[Value::I64(7)]);
    }

    #[test]
    fn metadata_instructions_update_span_or_complete() {
        let mut cpu = CPU::default();

        execute(&mut cpu, &Span(1, 2, 3)).unwrap();
        execute(&mut cpu, &Label).unwrap();
        execute(&mut cpu, &FunctionStart).unwrap();
        execute(&mut cpu, &FunctionEnd).unwrap();

        assert_eq!(cpu.span, (1, 2, 3));
    }

    #[test]
    fn breakpoint_emits_breakpoint_effect() {
        let mut cpu = CPU::default();

        let result = Execute::execute(&mut cpu, &Breakpoint, NoMessage).unwrap();

        assert_eq!(
            vihaco::expect_exactly_one_effect(result.effects).unwrap(),
            StepOutcome::Breakpoint
        );
    }

    #[test]
    fn conditional_branch_selects_target_and_rejects_non_boolean_values() {
        let mut cpu = CPU::default();

        cpu.stack_push(Value::Bool(true));
        let result = Execute::execute(&mut cpu, &ConditionalBranch(10, 20), NoMessage).unwrap();
        assert_eq!(
            vihaco::expect_exactly_one_effect(result.effects).unwrap(),
            StepOutcome::Continue
        );
        assert_eq!(cpu.take_pending_pc(), Some(10));

        cpu.stack_push(Value::Bool(false));
        let result = Execute::execute(&mut cpu, &ConditionalBranch(10, 20), NoMessage).unwrap();
        assert_eq!(
            vihaco::expect_exactly_one_effect(result.effects).unwrap(),
            StepOutcome::Continue
        );
        assert_eq!(cpu.take_pending_pc(), Some(20));

        cpu.stack_push(Value::I64(1));
        let error = Execute::execute(&mut cpu, &ConditionalBranch(10, 20), NoMessage)
            .err()
            .unwrap();
        assert!(error.to_string().contains("expected bool"));
    }

    #[test]
    fn call_creates_frame_and_rejects_missing_arguments() {
        let mut cpu = CPU {
            current_pc: 10,
            ..Default::default()
        };
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });
        cpu.stack_push(Value::I64(7));

        let result = Execute::execute(&mut cpu, &Call(1, 100), NoMessage).unwrap();
        assert_eq!(
            vihaco::expect_exactly_one_effect(result.effects).unwrap(),
            StepOutcome::Continue
        );

        assert_eq!(cpu.take_pending_pc(), Some(100));
        assert_eq!(cpu.frames[1].base, 0);
        assert_eq!(cpu.frames[1].ret_pc, 11);

        let error = Execute::execute(&mut cpu, &Call(2, 100), NoMessage)
            .err()
            .unwrap();
        assert!(error.to_string().contains("not enough arguments"));
    }

    #[test]
    fn load_store_and_dup_operate_on_frame_and_stack_values() {
        let mut cpu = CPU::default();
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });

        cpu.stack_push(Value::I64(7));
        execute(&mut cpu, &Store(Type::I64, 0)).unwrap();
        execute(&mut cpu, &Load(Type::I64, 0)).unwrap();
        execute(&mut cpu, &Dup).unwrap();

        assert_eq!(cpu.stack(), &[Value::I64(7), Value::I64(7), Value::I64(7)]);
    }

    #[test]
    fn numeric_binary_instructions_compute_results() {
        let mut cpu = CPU::default();

        cpu.stack_push(Value::I64(2));
        cpu.stack_push(Value::I64(3));
        execute(&mut cpu, &Add(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::I64(5));

        cpu.stack_push(Value::I64(2));
        cpu.stack_push(Value::I64(5));
        execute(&mut cpu, &Sub(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::I64(3));

        cpu.stack_push(Value::I64(3));
        cpu.stack_push(Value::I64(4));
        execute(&mut cpu, &Mul(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::I64(12));

        cpu.stack_push(Value::I64(2));
        cpu.stack_push(Value::I64(6));
        execute(&mut cpu, &Div(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::I64(3));

        cpu.stack_push(Value::I64(3));
        cpu.stack_push(Value::I64(7));
        execute(&mut cpu, &Rem(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::I64(1));
    }

    #[test]
    fn shift_rotate_and_bitwise_instructions_compute_results() {
        let mut cpu = CPU::default();

        cpu.stack_push(Value::U32(1));
        cpu.stack_push(Value::U32(3));
        execute(&mut cpu, &Shl(Type::U32)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::U32(8));

        cpu.stack_push(Value::U32(8));
        cpu.stack_push(Value::U32(1));
        execute(&mut cpu, &Shr(Type::U32)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::U32(4));

        cpu.stack_push(Value::U32(1));
        cpu.stack_push(Value::U32(2));
        execute(&mut cpu, &Rol(Type::U32)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::U32(4));

        cpu.stack_push(Value::U32(4));
        cpu.stack_push(Value::U32(2));
        execute(&mut cpu, &Ror(Type::U32)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::U32(1));

        cpu.stack_push(Value::U32(0b1010));
        cpu.stack_push(Value::U32(0b1100));
        execute(&mut cpu, &BitAnd(Type::U32)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::U32(0b1000));

        cpu.stack_push(Value::U32(0b1010));
        cpu.stack_push(Value::U32(0b1100));
        execute(&mut cpu, &BitOr(Type::U32)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::U32(0b1110));

        cpu.stack_push(Value::U32(0b1010));
        cpu.stack_push(Value::U32(0b1100));
        execute(&mut cpu, &BitXor(Type::U32)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::U32(0b0110));
    }

    #[test]
    fn boolean_binary_instructions_compute_results() {
        let mut cpu = CPU::default();

        cpu.stack_push(Value::Bool(true));
        cpu.stack_push(Value::Bool(false));
        execute(&mut cpu, &And).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::Bool(false));

        cpu.stack_push(Value::Bool(false));
        cpu.stack_push(Value::Bool(true));
        execute(&mut cpu, &Or).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::Bool(true));

        cpu.stack_push(Value::Bool(true));
        cpu.stack_push(Value::Bool(true));
        execute(&mut cpu, &Xor).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::Bool(false));
    }

    #[test]
    fn equality_and_ordering_instructions_compute_results() {
        let mut cpu = CPU::default();

        cpu.stack_push(Value::I64(2));
        cpu.stack_push(Value::I64(2));
        execute(&mut cpu, &Eq(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::Bool(true));

        cpu.stack_push(Value::I64(2));
        cpu.stack_push(Value::I64(3));
        execute(&mut cpu, &Ne(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::Bool(true));

        cpu.stack_push(Value::I64(2));
        cpu.stack_push(Value::I64(3));
        execute(&mut cpu, &Lt(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::Bool(true));

        cpu.stack_push(Value::I64(2));
        cpu.stack_push(Value::I64(2));
        execute(&mut cpu, &Le(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::Bool(true));

        cpu.stack_push(Value::I64(3));
        cpu.stack_push(Value::I64(2));
        execute(&mut cpu, &Gt(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::Bool(true));

        cpu.stack_push(Value::I64(3));
        cpu.stack_push(Value::I64(3));
        execute(&mut cpu, &Ge(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::Bool(true));
    }

    #[test]
    fn heap_item_can_be_read_and_deallocated() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(10));
        cpu.stack_push(Value::I64(20));
        execute(&mut cpu, &HeapAlloc(2)).unwrap();
        let heap_ref = cpu.stack_pop().unwrap();
        cpu.stack_push(heap_ref);
        cpu.stack_push(Value::U32(1));

        execute(&mut cpu, &GetItem).unwrap();

        assert_eq!(cpu.stack(), &[Value::I64(20)]);
        cpu.stack_push(heap_ref);
        execute(&mut cpu, &HeapDealloc).unwrap();
        assert!(cpu.heap.get(0).is_err());
    }

    #[test]
    fn neg_and_not_transform_stack_values() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(7));
        execute(&mut cpu, &Neg(Type::I64)).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), Value::I64(-7));

        cpu.stack_push(Value::Bool(true));
        execute(&mut cpu, &Not).unwrap();
        assert_eq!(cpu.stack(), &[Value::Bool(false)]);
    }

    #[test]
    fn binary_execute_completes_and_pushes_result() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(2));
        cpu.stack_push(Value::I64(3));

        let result = execute(&mut cpu, &Add(Type::I64)).unwrap();

        assert_eq!(result.execution, Execution::Complete);
        assert_eq!(cpu.stack(), &[Value::I64(5)]);
    }

    #[test]
    fn branch_updates_pending_program_counter_and_emits_continue() {
        let mut cpu = CPU::default();

        let result = Execute::execute(&mut cpu, &Branch(42), NoMessage).unwrap();

        assert_eq!(
            vihaco::expect_exactly_one_effect(result.effects).unwrap(),
            StepOutcome::Continue
        );
        assert_eq!(cpu.take_pending_pc(), Some(42));
    }

    #[test]
    fn return_emits_return_effect() {
        let mut cpu = CPU::default();
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });
        cpu.stack_push(Value::I64(7));

        let result = Execute::execute(&mut cpu, &Return(1), NoMessage).unwrap();

        assert_eq!(result.execution, Execution::Complete);
        assert_eq!(
            vihaco::expect_exactly_one_effect(result.effects).unwrap(),
            StepOutcome::Return
        );
        assert_eq!(cpu.return_values(), &[Value::I64(7)]);
    }
}

macro_rules! impl_op_num_binary {
    ($instruction:ident, $op:ident) => {
        impl Execute<$instruction> for CPU {
            type Message = NoMessage;
            type Effect = NoEffect;
            type Fault = eyre::Report;

            fn execute(
                &mut self,
                instruction: &$instruction,
                _message: Self::Message,
            ) -> Result<StepResult<Self::Effect>, Self::Fault> {
                let lhs: Value = self.stack_pop()?;
                let rhs: Value = self.stack_pop()?;
                let ty = instruction.0;
                if lhs.type_of() != ty {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for lhs",
                        ty,
                        lhs.type_of()
                    ));
                }

                if rhs.type_of() != ty {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for rhs",
                        ty,
                        rhs.type_of()
                    ));
                }

                let output = match (lhs, rhs) {
                    (Value::I64(l), Value::I64(r)) => Value::I64(l.$op(r)),
                    (Value::U32(l), Value::U32(r)) => Value::U32(l.$op(r)),
                    (Value::U64(l), Value::U64(r)) => Value::U64(l.$op(r)),
                    (Value::F64(l), Value::F64(r)) => Value::F64(l.$op(r)),
                    _ => {
                        return Err(eyre::eyre!(
                            "cannot {} {} and {}",
                            stringify!($op),
                            lhs.type_of(),
                            rhs.type_of()
                        ));
                    }
                };
                self.stack.push(output);
                vihaco::complete!()
            }
        }
    };
}

impl_op_num_binary!(Add, add);
impl_op_num_binary!(Sub, sub);
impl_op_num_binary!(Mul, mul);
impl_op_num_binary!(Div, div);
impl_op_num_binary!(Rem, rem);

impl Execute<Neg> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        instruction: &Neg,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        let v: Value = self.stack_pop()?;
        if v.type_of() != instruction.0 {
            return Err(eyre::eyre!(format!(
                "Type mismatch, expected {:?} got {:?}",
                instruction.0,
                v.type_of()
            )));
        }

        let output = match v {
            Value::I64(i) => Value::I64(-i),
            Value::F64(f) => Value::F64(-f),
            _ => return Err(eyre::eyre!(format!("cannot negate {}", v.type_of()))),
        };
        self.stack.push(output);
        vihaco::complete!()
    }
}

macro_rules! impl_op_shift {
    ($instruction:ident, $op:ident) => {
        impl Execute<$instruction> for CPU {
            type Message = NoMessage;
            type Effect = NoEffect;
            type Fault = eyre::Report;

            fn execute(
                &mut self,
                instruction: &$instruction,
                _message: Self::Message,
            ) -> Result<StepResult<Self::Effect>, Self::Fault> {
                let rhs: Value = self.stack_pop()?;
                let lhs: Value = self.stack_pop()?;
                let ty = instruction.0;
                if lhs.type_of() != ty {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for lhs",
                        ty,
                        lhs.type_of()
                    ));
                }

                if rhs.type_of() != ty {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for rhs",
                        ty,
                        rhs.type_of()
                    ));
                }
                let output = match (lhs, rhs) {
                    (Value::I64(l), Value::I64(r)) => Value::I64(l.$op(r)),
                    (Value::U32(l), Value::U32(r)) => Value::U32(l.$op(r)),
                    (Value::U64(l), Value::U64(r)) => Value::U64(l.$op(r)),
                    _ => {
                        return Err(eyre::eyre!(format!(
                            "cannot {} {} and {}",
                            stringify!($op),
                            lhs.type_of(),
                            rhs.type_of()
                        )));
                    }
                };
                self.stack.push(output);
                vihaco::complete!()
            }
        }
    };
}

impl_op_shift!(Shl, shl);
impl_op_shift!(Shr, shr);

macro_rules! impl_op_rotate {
    ($instruction:ident, $op:ident) => {
        impl Execute<$instruction> for CPU {
            type Message = NoMessage;
            type Effect = NoEffect;
            type Fault = eyre::Report;

            fn execute(
                &mut self,
                instruction: &$instruction,
                _message: Self::Message,
            ) -> Result<StepResult<Self::Effect>, Self::Fault> {
                let rhs: Value = self.stack_pop()?;
                let lhs: Value = self.stack_pop()?;
                let ty = instruction.0;
                if lhs.type_of() != ty {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for lhs",
                        ty,
                        lhs.type_of()
                    ));
                }

                if rhs.type_of() != Type::U32 {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for rhs",
                        Type::U32,
                        rhs.type_of()
                    ));
                }
                let output = match (lhs, rhs) {
                    (Value::I64(l), Value::U32(r)) => Value::I64(l.$op(r)),
                    (Value::U32(l), Value::U32(r)) => Value::U32(l.$op(r)),
                    (Value::U64(l), Value::U32(r)) => Value::U64(l.$op(r)),
                    _ => {
                        return Err(eyre::eyre!(format!(
                            "cannot {} {} and {}",
                            stringify!($op),
                            lhs.type_of(),
                            rhs.type_of()
                        )));
                    }
                };
                self.stack.push(output);
                vihaco::complete!()
            }
        }
    };
}

impl_op_rotate!(Rol, rotate_left);
impl_op_rotate!(Ror, rotate_right);

macro_rules! impl_op_bitwise {
    ($instruction:ident, $op:ident) => {
        impl Execute<$instruction> for CPU {
            type Message = NoMessage;
            type Effect = NoEffect;
            type Fault = eyre::Report;

            fn execute(
                &mut self,
                instruction: &$instruction,
                _message: Self::Message,
            ) -> Result<StepResult<Self::Effect>, Self::Fault> {
                let rhs: Value = self.stack_pop()?;
                let lhs: Value = self.stack_pop()?;
                let ty = instruction.0;
                if lhs.type_of() != ty {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for lhs",
                        ty,
                        lhs.type_of()
                    ));
                }

                if rhs.type_of() != ty {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for rhs",
                        ty,
                        rhs.type_of()
                    ));
                }
                let output = match (lhs, rhs) {
                    (Value::I64(l), Value::I64(r)) => Value::I64(l.$op(r)),
                    (Value::U32(l), Value::U32(r)) => Value::U32(l.$op(r)),
                    (Value::U64(l), Value::U64(r)) => Value::U64(l.$op(r)),
                    _ => {
                        return Err(eyre::eyre!(format!(
                            "cannot {} {} and {}",
                            stringify!($op),
                            lhs.type_of(),
                            rhs.type_of()
                        )));
                    }
                };
                self.stack.push(output);
                vihaco::complete!()
            }
        }
    };
}

impl_op_bitwise!(BitAnd, bitand);
impl_op_bitwise!(BitOr, bitor);
impl_op_bitwise!(BitXor, bitxor);

macro_rules! impl_boolean_binary {
    ($instruction:ident, $op:ident) => {
        impl Execute<$instruction> for CPU {
            type Message = NoMessage;
            type Effect = NoEffect;
            type Fault = eyre::Report;

            fn execute(
                &mut self,
                _instruction: &$instruction,
                _message: Self::Message,
            ) -> Result<StepResult<Self::Effect>, Self::Fault> {
                let rhs: bool = self.stack_pop()?.try_into()?;
                let lhs: bool = self.stack_pop()?.try_into()?;
                let output = lhs.$op(rhs);
                self.stack_push(output);
                vihaco::complete!()
            }
        }
    };
}

impl Execute<Not> for CPU {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &Not,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        let v: bool = self.stack_pop()?.try_into()?;
        self.stack_push(!v);
        vihaco::complete!()
    }
}

impl_boolean_binary!(And, bitand);
impl_boolean_binary!(Or, bitor);
impl_boolean_binary!(Xor, bitxor);

macro_rules! impl_eq {
    ($instruction:ident, $op:ident) => {
        impl Execute<$instruction> for CPU {
            type Message = NoMessage;
            type Effect = NoEffect;
            type Fault = eyre::Report;

            fn execute(
                &mut self,
                instruction: &$instruction,
                _message: Self::Message,
            ) -> Result<StepResult<Self::Effect>, Self::Fault> {
                let rhs: Value = self.stack_pop()?;
                let lhs: Value = self.stack_pop()?;
                let ty = instruction.0;
                if lhs.type_of() != ty {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for lhs",
                        ty,
                        lhs.type_of()
                    ));
                }

                if rhs.type_of() != ty {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for rhs",
                        ty,
                        rhs.type_of()
                    ));
                }
                let output = lhs.$op(&rhs);
                self.stack_push(output);
                vihaco::complete!()
            }
        }
    };
}

impl_eq!(Eq, eq);
impl_eq!(Ne, ne);

macro_rules! impl_ordering {
    ($instruction:ident, $op:ident) => {
        impl Execute<$instruction> for CPU {
            type Message = NoMessage;
            type Effect = NoEffect;
            type Fault = eyre::Report;

            fn execute(
                &mut self,
                instruction: &$instruction,
                _message: Self::Message,
            ) -> Result<StepResult<Self::Effect>, Self::Fault> {
                let rhs: Value = self.stack_pop()?;
                let lhs: Value = self.stack_pop()?;
                let ty = instruction.0;
                if lhs.type_of() != ty {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for lhs",
                        ty,
                        lhs.type_of()
                    ));
                }

                if rhs.type_of() != ty {
                    return Err(eyre::eyre!(
                        "Type mismatch, expected {} got {} for rhs",
                        ty,
                        rhs.type_of()
                    ));
                }

                let output = match (lhs, rhs) {
                    (Value::Bool(l), Value::Bool(r)) => l.$op(&r),
                    (Value::I64(l), Value::I64(r)) => l.$op(&r),
                    (Value::U32(l), Value::U32(r)) => l.$op(&r),
                    (Value::U64(l), Value::U64(r)) => l.$op(&r),
                    (Value::F64(l), Value::F64(r)) => l.$op(&r),
                    _ => {
                        return Err(eyre::eyre!(format!(
                            "cannot compare {} and {}",
                            lhs.type_of(),
                            rhs.type_of()
                        )));
                    }
                };
                self.stack_push(output);
                vihaco::complete!()
            }
        }
    };
}

impl_ordering!(Lt, lt);
impl_ordering!(Le, le);
impl_ordering!(Gt, gt);
impl_ordering!(Ge, ge);
