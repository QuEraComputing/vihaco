// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::RuntimeInstruction;
use crate::StepOutcome;
use crate::Word;
use crate::data::CPU;
use crate::word::*;
use eyre::Result;
use vihaco::Effects;
use vihaco::{dispatch, frame::Frame, traits::*};

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

impl CPU {
    #[inline(always)]
    fn execute_generated(
        &mut self,
        inst: &RuntimeInstruction,
        msg: CPUMessage,
    ) -> eyre::Result<Effects<StepOutcome>> {
        use RuntimeInstruction::*;

        self.clear_pending_pc();
        match (inst, msg) {
            (Print, CPUMessage::Print(text)) => {
                self.stack_pop()?;
                drop(text);
                return Ok(Effects::one(StepOutcome::Continue));
            }
            (Print, _) => return Err(eyre::eyre!("Print requires CPUMessage::Print")),
            (_, CPUMessage::Print(_)) => {
                return Err(eyre::eyre!(
                    "CPUMessage::Print is only valid for Print instruction"
                ));
            }
            (
                _,
                CPUMessage::FunctionInfo {
                    arity,
                    start_address,
                },
            ) => {
                self.stack_push(arity);
                self.stack_push(start_address);
            }
            (_, CPUMessage::None) => {}
        }

        let outcome = match inst {
            Span(file, start, end) => self.op_span(*file, *start, *end),
            Label(_) | FunctionStart | FunctionEnd => Ok(StepOutcome::Continue),
            Breakpoint => Ok(StepOutcome::Breakpoint),
            Branch(target) => self.op_branch(*target),
            ConditionalBranch(true_target, false_target) => {
                self.op_conditional_branch(*true_target, *false_target)
            }
            Return(keep) => self.op_return(*keep),
            Call(arity, target) => self.op_call(*arity, *target),
            IndirectCall => self.op_indirect_call(),
            Halt => Ok(StepOutcome::Halt),
            Print => Err(eyre::eyre!(
                "Print must be handled via execute with CPUMessage::Print"
            )),
            LoadI32(addr) => self.op_load(*addr),
            LoadI64(addr) => self.op_load(*addr),
            LoadU32(addr) => self.op_load(*addr),
            LoadU64(addr) => self.op_load(*addr),
            LoadF32(addr) => self.op_load(*addr),
            LoadF64(addr) => self.op_load(*addr),
            LoadBool(addr) => self.op_load(*addr),
            StoreI32(addr) => self.op_store(*addr),
            StoreI64(addr) => self.op_store(*addr),
            StoreU32(addr) => self.op_store(*addr),
            StoreU64(addr) => self.op_store(*addr),
            StoreF32(addr) => self.op_store(*addr),
            StoreF64(addr) => self.op_store(*addr),
            StoreBool(addr) => self.op_store(*addr),
            Dup => self.op_dup(),
            HeapAlloc(n_elements) => self.op_heap_alloc(*n_elements),
            GetItem => self.op_get_item(),
            HeapDealloc => self.op_heap_dealloc(),
            ConstI32(v) | ConstI64(v) | ConstU32(v) | ConstU64(v) | ConstF32(v) | ConstF64(v)
            | ConstBool(v) | ConstString(v) | ConstFunctionRef(v) | ConstHeapRef(v) => {
                self.op_const(*v)
            }
            AddI32 => self.add_i32(),
            AddI64 => self.add_i64(),
            AddU32 => self.add_u32(),
            AddU64 => self.add_u64(),
            AddF32 => self.add_f32(),
            AddF64 => self.add_f64(),
            SubI32 => self.sub_i32(),
            SubI64 => self.sub_i64(),
            SubU32 => self.sub_u32(),
            SubU64 => self.sub_u64(),
            SubF32 => self.sub_f32(),
            SubF64 => self.sub_f64(),
            MulI32 => self.mul_i32(),
            MulI64 => self.mul_i64(),
            MulU32 => self.mul_u32(),
            MulU64 => self.mul_u64(),
            MulF32 => self.mul_f32(),
            MulF64 => self.mul_f64(),
            DivI32 => self.div_i32(),
            DivI64 => self.div_i64(),
            DivU32 => self.div_u32(),
            DivU64 => self.div_u64(),
            DivF32 => self.div_f32(),
            DivF64 => self.div_f64(),
            RemI32 => self.rem_i32(),
            RemI64 => self.rem_i64(),
            RemU32 => self.rem_u32(),
            RemU64 => self.rem_u64(),
            RemF32 => self.rem_f32(),
            RemF64 => self.rem_f64(),
            NegI32 => self.neg_i32(),
            NegI64 => self.neg_i64(),
            NegF32 => self.neg_f32(),
            NegF64 => self.neg_f64(),
            ShlI32 => self.shl_i32(),
            ShlI64 => self.shl_i64(),
            ShlU32 => self.shl_u32(),
            ShlU64 => self.shl_u64(),
            ShrI32 => self.shr_i32(),
            ShrI64 => self.shr_i64(),
            ShrU32 => self.shr_u32(),
            ShrU64 => self.shr_u64(),
            RolI32 => self.rol_i32(),
            RolI64 => self.rol_i64(),
            RolU32 => self.rol_u32(),
            RolU64 => self.rol_u64(),
            RorI32 => self.ror_i32(),
            RorI64 => self.ror_i64(),
            RorU32 => self.ror_u32(),
            RorU64 => self.ror_u64(),
            BitAndI32 => self.bitand_i32(),
            BitAndI64 => self.bitand_i64(),
            BitAndU32 => self.bitand_u32(),
            BitAndU64 => self.bitand_u64(),
            BitOrI32 => self.bitor_i32(),
            BitOrI64 => self.bitor_i64(),
            BitOrU32 => self.bitor_u32(),
            BitOrU64 => self.bitor_u64(),
            BitXorI32 => self.bitxor_i32(),
            BitXorI64 => self.bitxor_i64(),
            BitXorU32 => self.bitxor_u32(),
            BitXorU64 => self.bitxor_u64(),
            Not => self.op_not(),
            And => self.op_and(),
            Or => self.op_or(),
            Xor => self.op_xor(),
            EqI32 => self.eq_i32(),
            EqI64 => self.eq_i64(),
            EqU32 => self.eq_u32(),
            EqU64 => self.eq_u64(),
            EqF32 => self.eq_f32(),
            EqF64 => self.eq_f64(),
            NeI32 => self.ne_i32(),
            NeI64 => self.ne_i64(),
            NeU32 => self.ne_u32(),
            NeU64 => self.ne_u64(),
            NeF32 => self.ne_f32(),
            NeF64 => self.ne_f64(),
            LtI32 => self.lt_i32(),
            LtI64 => self.lt_i64(),
            LtU32 => self.lt_u32(),
            LtU64 => self.lt_u64(),
            LtF32 => self.lt_f32(),
            LtF64 => self.lt_f64(),
            GtI32 => self.gt_i32(),
            GtI64 => self.gt_i64(),
            GtU32 => self.gt_u32(),
            GtU64 => self.gt_u64(),
            GtF32 => self.gt_f32(),
            GtF64 => self.gt_f64(),
            LeI32 => self.le_i32(),
            LeI64 => self.le_i64(),
            LeU32 => self.le_u32(),
            LeU64 => self.le_u64(),
            LeF32 => self.le_f32(),
            LeF64 => self.le_f64(),
            GeI32 => self.ge_i32(),
            GeI64 => self.ge_i64(),
            GeU32 => self.ge_u32(),
            GeU64 => self.ge_u64(),
            GeF32 => self.ge_f32(),
            GeF64 => self.ge_f64(),
        }?;
        Ok(Effects::one(outcome))
    }
}

#[derive(Debug, Clone, PartialEq, vihaco::Message)]
pub enum CPUMessage {
    None,
    FunctionInfo { arity: u32, start_address: u32 },
    Print(String),
}

#[dispatch(instruction = RuntimeInstruction, message = CPUMessage, effect = StepOutcome)]
impl CPU {
    fn execute(
        &mut self,
        inst: &RuntimeInstruction,
        msg: CPUMessage,
    ) -> eyre::Result<Effects<StepOutcome>> {
        self.execute_generated(inst, msg)
    }
}

impl CPU {
    pub fn op_span(&mut self, file: u32, start: u32, end: u32) -> eyre::Result<StepOutcome> {
        self.span = (file, start, end);
        Ok(StepOutcome::Continue)
    }

    pub fn op_branch(&mut self, target: u32) -> eyre::Result<StepOutcome> {
        self.set_pending_pc(target);
        Ok(StepOutcome::Continue)
    }

    pub fn op_conditional_branch(
        &mut self,
        true_target: u32,
        false_target: u32,
    ) -> eyre::Result<StepOutcome> {
        let cond = self
            .stack
            .pop()
            .ok_or_else(|| eyre::eyre!("stack underflow"))?;
        match canonical_bool(cond)? {
            true => {
                self.set_pending_pc(true_target);
                Ok(StepOutcome::Continue)
            }
            false => {
                self.set_pending_pc(false_target);
                Ok(StepOutcome::Continue)
            }
        }
    }

    pub fn op_return(&mut self, keep: u32) -> eyre::Result<StepOutcome> {
        let frame = self.pop_frame()?;
        let frame_len = self
            .stack
            .len()
            .checked_sub(frame.base)
            .ok_or_else(|| eyre::eyre!("frame base out of bounds"))?;
        if frame_len < keep as usize {
            return Err(eyre::eyre!("not enough values to return"));
        }

        // Collect return values before truncating
        let top = self.stack.len() - keep as usize;
        let return_values: Vec<Word> = self.stack[top..].to_vec();
        self.stack.drain(frame.base..top);

        if self.get_frame().is_err() {
            // No more frames - program is returning
            self.set_return_values(return_values);
            Ok(StepOutcome::Return)
        } else {
            self.set_pending_pc(frame.ret_pc);
            Ok(StepOutcome::Continue)
        }
    }

    pub fn op_call(&mut self, arity: u32, target: u32) -> eyre::Result<StepOutcome> {
        if self.stack.len() < (arity as usize) {
            return Err(eyre::eyre!(
                "not enough arguments on stack to call function"
            ));
        }

        let base = self.stack.len() - (arity as usize);
        let frame = Frame {
            base,
            span: self.span,
            function: None,
            ret_pc: self.current_pc + 1,
        };
        self.push_frame(frame);
        self.set_pending_pc(target);
        Ok(StepOutcome::Continue)
    }

    pub fn op_indirect_call(&mut self) -> eyre::Result<StepOutcome> {
        // simliar order to op_call but from the stack
        let target: u32 = self.stack_pop()?.try_into()?;
        let arity: u32 = self.stack_pop()?.try_into()?;
        let f = decode_function_ref(self.stack_pop()?);

        if self.stack.len() < (arity as usize) {
            return Err(eyre::eyre!(
                "not enough arguments on stack to call function"
            ));
        }

        let base = self.stack.len() - (arity as usize);
        let frame = Frame {
            base,
            span: self.span,
            function: Some(f as usize),
            ret_pc: self.current_pc + 1,
        };
        self.push_frame(frame);
        self.set_pending_pc(target);
        Ok(StepOutcome::Continue)
    }

    fn op_load(&mut self, addr: u32) -> eyre::Result<StepOutcome> {
        // addr should be local to frame.
        let value = self.get_local(addr as usize)?;
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }

    pub fn op_store(&mut self, addr: u32) -> Result<StepOutcome> {
        let v: Word = self.stack_pop()?;
        log::debug!("store value {:?} at addr {}", v, addr);
        *self.get_local_mut(addr as usize)? = v;
        Ok(StepOutcome::Continue)
    }

    pub fn op_dup(&mut self) -> Result<StepOutcome> {
        let v = *self.stack_top()?;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }

    pub fn op_heap_alloc(&mut self, n_elements: u32) -> Result<StepOutcome> {
        let n: usize = n_elements as usize;
        if self.stack.len() < n {
            return Err(eyre::eyre!("stack underflow"));
        }
        let start = self.stack.len() - n;
        let values: Box<[Word]> = self.stack.drain(start..).collect();
        let heap_id = self.push_heap_object(values);
        self.stack_push(encode_heap_ref(heap_id));
        Ok(StepOutcome::Continue)
    }

    pub fn op_get_item(&mut self) -> Result<StepOutcome> {
        let index = Self::heap_index(self.stack_pop()?)?;
        let heap_id = decode_heap_ref(self.stack_pop()?);
        let value = *self
            .heap_object(heap_id)?
            .get(index)
            .ok_or_else(|| eyre::eyre!("heap index {} out of bounds", index))?;
        self.stack_push(value);
        Ok(StepOutcome::Continue)
    }

    pub fn op_heap_dealloc(&mut self) -> Result<StepOutcome> {
        let id = decode_heap_ref(self.stack_pop()?);
        self.dealloc_heap_object(id)?;
        Ok(StepOutcome::Continue)
    }

    pub fn op_const(&mut self, v: Word) -> Result<StepOutcome> {
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }

    fn heap_index(value: Word) -> Result<usize> {
        match decode_i64(value) {
            index if index >= 0 => usize::try_from(index)
                .map_err(|_| eyre::eyre!("heap index {} does not fit in usize", index)),
            index => Err(eyre::eyre!(
                "heap index must be non-negative, got {}",
                index
            )),
        }
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use vihaco::{Effects, GeneratedComponent, frame::Frame, traits::StackMemory};
    use vihaco_parser::Ident;

    trait ExecuteInstruction {
        fn execute_instruction(&mut self, instruction: RuntimeInstruction) -> Result<StepOutcome>;
    }

    impl ExecuteInstruction for CPU {
        fn execute_instruction(&mut self, instruction: RuntimeInstruction) -> Result<StepOutcome> {
            vihaco::expect_exactly_one_effect(GeneratedComponent::execute_generated(
                self,
                &instruction,
                CPUMessage::None,
            )?)
        }
    }

    #[test]
    fn cpu_generated_component_executes_instruction_without_message() {
        let mut cpu = CPU::default();

        GeneratedComponent::execute_generated(
            &mut cpu,
            &RuntimeInstruction::ConstI64(encode_i64(7)),
            CPUMessage::None,
        )
        .unwrap();

        assert_eq!(cpu.stack(), &vec![encode_i64(7)]);
    }

    #[test]
    fn execute_instruction_applies_control_flow_without_action() {
        let mut cpu = CPU::default();

        let branch = cpu
            .execute_instruction(RuntimeInstruction::Branch(9))
            .unwrap();
        assert_eq!(branch, StepOutcome::Continue);
        assert_eq!(cpu.take_pending_pc(), Some(9));

        let halt = cpu.execute_instruction(RuntimeInstruction::Halt).unwrap();
        assert_eq!(halt, StepOutcome::Halt);
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
        cpu.stack_push(encode_i64(7));

        let outcome = cpu
            .execute_instruction(RuntimeInstruction::Return(1))
            .unwrap();

        assert_eq!(outcome, StepOutcome::Return);
        assert_eq!(cpu.return_values(), &[encode_i64(7)]);
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
        cpu.execute_instruction(RuntimeInstruction::Call(0, 100))
            .unwrap();
        assert_eq!(cpu.take_pending_pc(), Some(100));
        assert_eq!(cpu.frames[1].ret_pc, 11);

        // Callee returns immediately. pending_pc should be restored to the
        // instruction after the call.
        let outcome = cpu
            .execute_instruction(RuntimeInstruction::Return(0))
            .unwrap();
        assert_eq!(outcome, StepOutcome::Continue);
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

        // IndirectCall pops (top → bottom): target, arity, FunctionRef.
        cpu.stack_push(encode_function_ref(7));
        cpu.stack_push(encode_u32(0));
        cpu.stack_push(encode_u32(100));

        cpu.execute_instruction(RuntimeInstruction::IndirectCall)
            .unwrap();
        assert_eq!(cpu.take_pending_pc(), Some(100));
        assert_eq!(cpu.frames[1].ret_pc, 11);

        let outcome = cpu
            .execute_instruction(RuntimeInstruction::Return(0))
            .unwrap();
        assert_eq!(outcome, StepOutcome::Continue);
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
        cpu.stack_push(encode_i64(111)); // scratch — bottom of callee frame
        cpu.stack_push(encode_i64(222)); // scratch — middle
        cpu.stack_push(encode_i64(999)); // intended return value — top

        let outcome = cpu
            .execute_instruction(RuntimeInstruction::Return(1))
            .unwrap();
        assert_eq!(outcome, StepOutcome::Continue);

        assert_eq!(cpu.stack(), &vec![encode_i64(999)],);
    }

    #[test]
    fn op_heap_alloc_preserves_natural_push_order_and_returns_heap_ref() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i64(10));
        cpu.stack_push(encode_i64(20));
        cpu.stack_push(encode_i64(30));

        let outcome = cpu
            .execute_instruction(RuntimeInstruction::HeapAlloc(3))
            .unwrap();

        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.stack(), &vec![encode_heap_ref(0)]);
        assert_eq!(
            cpu.heap.get(0).unwrap(),
            &[encode_i64(10), encode_i64(20), encode_i64(30)]
        );
    }

    #[test]
    fn op_heap_alloc_supports_empty_heap_objects() {
        let mut cpu = CPU::default();

        let outcome = cpu
            .execute_instruction(RuntimeInstruction::HeapAlloc(0))
            .unwrap();

        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.stack(), &vec![encode_heap_ref(0)]);
        assert_eq!(cpu.heap.get(0).unwrap(), &[] as &[Word]);
    }

    #[test]
    fn op_get_item_reads_heap_value() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i64(10));
        cpu.stack_push(encode_i64(20));
        cpu.stack_push(encode_i64(30));
        cpu.execute_instruction(RuntimeInstruction::HeapAlloc(3))
            .unwrap();
        cpu.stack_push(encode_u32(1));

        let outcome = cpu
            .execute_instruction(RuntimeInstruction::GetItem)
            .unwrap();

        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.stack(), &vec![encode_i64(20)]);
    }

    #[test]
    fn op_get_item_rejects_non_heap_refs() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i64(7));
        cpu.stack_push(encode_u32(0));

        let err = cpu
            .execute_instruction(RuntimeInstruction::GetItem)
            .unwrap_err();

        assert!(err.to_string().contains("heap"));
    }

    #[test]
    fn op_get_item_rejects_invalid_heap_ids() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_heap_ref(99));
        cpu.stack_push(encode_u32(0));

        let err = cpu
            .execute_instruction(RuntimeInstruction::GetItem)
            .unwrap_err();

        assert!(err.to_string().contains("heap"));
    }

    #[test]
    fn op_get_item_rejects_out_of_bounds_indices() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i64(10));
        cpu.execute_instruction(RuntimeInstruction::HeapAlloc(1))
            .unwrap();
        cpu.stack_push(encode_u32(3));

        let err = cpu
            .execute_instruction(RuntimeInstruction::GetItem)
            .unwrap_err();

        assert!(err.to_string().contains("index"));
    }

    #[test]
    fn reset_clears_heap_allocations() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i64(10));
        cpu.execute_instruction(RuntimeInstruction::HeapAlloc(1))
            .unwrap();

        cpu.reset();

        assert!(cpu.heap.is_empty());
        assert!(cpu.stack().is_empty());
    }

    #[test]
    fn execute_generated_dispatches_instruction_without_message() {
        let mut cpu = CPU::default();
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });

        let outcome = GeneratedComponent::execute_generated(
            &mut cpu,
            &RuntimeInstruction::ConstI64(encode_i64(99)),
            CPUMessage::None,
        )
        .unwrap();

        assert_eq!(outcome, Effects::one(StepOutcome::Continue));
        assert_eq!(cpu.stack(), &vec![encode_i64(99)]);
    }

    #[test]
    fn execute_generated_function_info_pushes_arity_and_start_address() {
        let mut cpu = CPU::default();
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });

        let outcome = GeneratedComponent::execute_generated(
            &mut cpu,
            &RuntimeInstruction::Label(Ident("label".to_owned())),
            CPUMessage::FunctionInfo {
                arity: 2,
                start_address: 42,
            },
        )
        .unwrap();

        assert_eq!(outcome, Effects::one(StepOutcome::Continue));
        // arity pushed first, then start_address
        assert_eq!(cpu.stack(), &vec![encode_u32(2), encode_u32(42)]);
    }

    #[test]
    fn execute_generated_print_returns_control_effect_and_pops_stack() {
        let mut cpu = CPU::default();
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });
        cpu.stack_push(encode_i64(42));

        let outcome = GeneratedComponent::execute_generated(
            &mut cpu,
            &RuntimeInstruction::Print,
            CPUMessage::Print("hello".into()),
        )
        .unwrap();

        assert_eq!(outcome, Effects::one(StepOutcome::Continue));
        assert!(cpu.stack().is_empty());
    }

    #[test]
    fn execute_generated_print_rejects_wrong_message() {
        let mut cpu = CPU::default();
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });
        cpu.stack_push(encode_i64(42));

        let err = GeneratedComponent::execute_generated(
            &mut cpu,
            &RuntimeInstruction::Print,
            CPUMessage::None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("Print requires"));
    }

    #[test]
    fn op_heap_dealloc_marks_slot_dead() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i64(42));
        cpu.execute_instruction(RuntimeInstruction::HeapAlloc(1))
            .unwrap();
        cpu.stack_push(encode_heap_ref(0));

        cpu.execute_instruction(RuntimeInstruction::HeapDealloc)
            .unwrap();

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
        cpu.stack_push(encode_i64(1));
        cpu.execute_instruction(RuntimeInstruction::HeapAlloc(1))
            .unwrap();
        cpu.execute_instruction(RuntimeInstruction::HeapDealloc)
            .unwrap();

        cpu.stack_push(encode_i64(2));
        cpu.execute_instruction(RuntimeInstruction::HeapAlloc(1))
            .unwrap();

        assert_eq!(cpu.stack(), &vec![encode_heap_ref(0)]);
        assert_eq!(cpu.heap.get(0).unwrap(), &[encode_i64(2)]);
    }

    #[test]
    fn op_heap_dealloc_rejects_double_free() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i64(1));
        cpu.execute_instruction(RuntimeInstruction::HeapAlloc(1))
            .unwrap();
        cpu.stack_push(encode_heap_ref(0));
        cpu.execute_instruction(RuntimeInstruction::HeapDealloc)
            .unwrap();

        cpu.stack_push(encode_heap_ref(0));
        let err = cpu
            .execute_instruction(RuntimeInstruction::HeapDealloc)
            .unwrap_err();

        assert!(err.to_string().contains("double-free"));
    }

    #[test]
    fn op_heap_dealloc_rejects_invalid_id() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_heap_ref(99));

        let err = cpu
            .execute_instruction(RuntimeInstruction::HeapDealloc)
            .unwrap_err();

        assert!(err.to_string().contains("invalid heap object id"));
    }

    #[test]
    fn reset_clears_free_list() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i64(1));
        cpu.execute_instruction(RuntimeInstruction::HeapAlloc(1))
            .unwrap();
        cpu.stack_push(encode_heap_ref(0));
        cpu.execute_instruction(RuntimeInstruction::HeapDealloc)
            .unwrap();

        cpu.reset();

        assert!(cpu.heap.is_empty());
    }

    #[test]
    fn typed_word_arithmetic_canonicalizes_narrow_results() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i32(i32::MAX));
        cpu.stack_push(encode_i32(1));
        cpu.execute_instruction(RuntimeInstruction::AddI32).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), encode_i32(i32::MIN));

        cpu.stack_push(encode_u32(u32::MAX));
        cpu.stack_push(encode_u32(1));
        cpu.execute_instruction(RuntimeInstruction::AddU32).unwrap();
        assert_eq!(cpu.stack_pop().unwrap(), 0);

        cpu.stack_push(encode_f32(1.5));
        cpu.stack_push(encode_f32(2.0));
        cpu.execute_instruction(RuntimeInstruction::MulF32).unwrap();
        assert_eq!(decode_f32(cpu.stack_pop().unwrap()), 3.0);
    }

    #[test]
    fn integer_division_and_remainder_report_errors() {
        let mut cpu = CPU::default();
        cpu.stack_push(encode_i64(7));
        cpu.stack_push(encode_i64(0));
        assert!(cpu.execute_instruction(RuntimeInstruction::DivI64).is_err());

        cpu.stack_push(encode_u32(7));
        cpu.stack_push(encode_u32(0));
        assert!(cpu.execute_instruction(RuntimeInstruction::RemU32).is_err());
    }

    #[test]
    fn boolean_words_must_be_canonical() {
        let mut cpu = CPU::default();
        cpu.stack_push(2u64);
        assert!(cpu.execute_instruction(RuntimeInstruction::Not).is_err());

        cpu.stack_push(2u64);
        assert!(
            cpu.execute_instruction(RuntimeInstruction::ConditionalBranch(1, 2))
                .is_err()
        );
    }
}

fn canonical_bool(value: Word) -> Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(eyre::eyre!("invalid boolean word {}", other)),
    }
}

macro_rules! int_wrapping {
    ($($name:ident: $decode:ident -> $encode:ident .$op:ident);+ $(;)?) => {$ (
        #[inline(always)]
        fn $name(&mut self) -> Result<StepOutcome> {
            let rhs = $decode(self.stack_pop()?);
            let lhs = $decode(self.stack_pop()?);
            self.stack_push($encode(lhs.$op(rhs)));
            Ok(StepOutcome::Continue)
        }
    )+ };
}

macro_rules! int_checked {
    ($($name:ident: $decode:ident -> $encode:ident .$op:ident, $message:literal);+ $(;)?) => {$ (
        #[inline(always)]
        fn $name(&mut self) -> Result<StepOutcome> {
            let rhs = $decode(self.stack_pop()?);
            let lhs = $decode(self.stack_pop()?);
            let value = lhs.$op(rhs).ok_or_else(|| eyre::eyre!($message))?;
            self.stack_push($encode(value));
            Ok(StepOutcome::Continue)
        }
    )+ };
}

macro_rules! float_binary {
    ($($name:ident: $decode:ident -> $encode:ident $op:tt);+ $(;)?) => {$ (
        #[inline(always)]
        fn $name(&mut self) -> Result<StepOutcome> {
            let rhs = $decode(self.stack_pop()?);
            let lhs = $decode(self.stack_pop()?);
            self.stack_push($encode(lhs $op rhs));
            Ok(StepOutcome::Continue)
        }
    )+ };
}

macro_rules! shift {
    ($($name:ident: $decode:ident -> $encode:ident .$op:ident, $mask:expr);+ $(;)?) => {$ (
        #[inline(always)]
        fn $name(&mut self) -> Result<StepOutcome> {
            let rhs = decode_u32(self.stack_pop()?);
            let lhs = $decode(self.stack_pop()?);
            self.stack_push($encode(lhs.$op(rhs & $mask)));
            Ok(StepOutcome::Continue)
        }
    )+ };
}

macro_rules! rotate {
    ($($name:ident: $decode:ident -> $encode:ident .$op:ident);+ $(;)?) => {$ (
        #[inline(always)]
        fn $name(&mut self) -> Result<StepOutcome> {
            let rhs = decode_u32(self.stack_pop()?);
            let lhs = $decode(self.stack_pop()?);
            self.stack_push($encode(lhs.$op(rhs)));
            Ok(StepOutcome::Continue)
        }
    )+ };
}

macro_rules! bitwise {
    ($($name:ident: $decode:ident -> $encode:ident $op:tt);+ $(;)?) => {$ (
        #[inline(always)]
        fn $name(&mut self) -> Result<StepOutcome> {
            let rhs = $decode(self.stack_pop()?);
            let lhs = $decode(self.stack_pop()?);
            self.stack_push($encode(lhs $op rhs));
            Ok(StepOutcome::Continue)
        }
    )+ };
}

macro_rules! compare {
    ($($name:ident: $decode:ident, $op:tt);+ $(;)?) => {$ (
        #[inline(always)]
        fn $name(&mut self) -> Result<StepOutcome> {
            let rhs = $decode(self.stack_pop()?);
            let lhs = $decode(self.stack_pop()?);
            self.stack_push(encode_bool(lhs $op rhs));
            Ok(StepOutcome::Continue)
        }
    )+ };
}

impl CPU {
    int_wrapping! {
        add_i32: decode_i32 -> encode_i32 .wrapping_add;
        add_i64: decode_i64 -> encode_i64 .wrapping_add;
        add_u32: decode_u32 -> encode_u32 .wrapping_add;
        add_u64: decode_u64 -> encode_u64 .wrapping_add;
        sub_i32: decode_i32 -> encode_i32 .wrapping_sub;
        sub_i64: decode_i64 -> encode_i64 .wrapping_sub;
        sub_u32: decode_u32 -> encode_u32 .wrapping_sub;
        sub_u64: decode_u64 -> encode_u64 .wrapping_sub;
        mul_i32: decode_i32 -> encode_i32 .wrapping_mul;
        mul_i64: decode_i64 -> encode_i64 .wrapping_mul;
        mul_u32: decode_u32 -> encode_u32 .wrapping_mul;
        mul_u64: decode_u64 -> encode_u64 .wrapping_mul;
    }
    int_checked! {
        div_i32: decode_i32 -> encode_i32 .checked_div, "integer division error";
        div_i64: decode_i64 -> encode_i64 .checked_div, "integer division error";
        div_u32: decode_u32 -> encode_u32 .checked_div, "integer division error";
        div_u64: decode_u64 -> encode_u64 .checked_div, "integer division error";
        rem_i32: decode_i32 -> encode_i32 .checked_rem, "integer remainder error";
        rem_i64: decode_i64 -> encode_i64 .checked_rem, "integer remainder error";
        rem_u32: decode_u32 -> encode_u32 .checked_rem, "integer remainder error";
        rem_u64: decode_u64 -> encode_u64 .checked_rem, "integer remainder error";
    }
    float_binary! {
        add_f32: decode_f32 -> encode_f32 +;
        add_f64: decode_f64 -> encode_f64 +;
        sub_f32: decode_f32 -> encode_f32 -;
        sub_f64: decode_f64 -> encode_f64 -;
        mul_f32: decode_f32 -> encode_f32 *;
        mul_f64: decode_f64 -> encode_f64 *;
        div_f32: decode_f32 -> encode_f32 /;
        div_f64: decode_f64 -> encode_f64 /;
        rem_f32: decode_f32 -> encode_f32 %;
        rem_f64: decode_f64 -> encode_f64 %;
    }

    #[inline(always)]
    fn neg_i32(&mut self) -> Result<StepOutcome> {
        let value = decode_i32(self.stack_pop()?).wrapping_neg();
        self.stack_push(encode_i32(value));
        Ok(StepOutcome::Continue)
    }
    #[inline(always)]
    fn neg_i64(&mut self) -> Result<StepOutcome> {
        let value = decode_i64(self.stack_pop()?).wrapping_neg();
        self.stack_push(encode_i64(value));
        Ok(StepOutcome::Continue)
    }
    #[inline(always)]
    fn neg_f32(&mut self) -> Result<StepOutcome> {
        let value = -decode_f32(self.stack_pop()?);
        self.stack_push(encode_f32(value));
        Ok(StepOutcome::Continue)
    }
    #[inline(always)]
    fn neg_f64(&mut self) -> Result<StepOutcome> {
        let value = -decode_f64(self.stack_pop()?);
        self.stack_push(encode_f64(value));
        Ok(StepOutcome::Continue)
    }

    shift! {
        shl_i32: decode_i32 -> encode_i32 .wrapping_shl, 31;
        shl_i64: decode_i64 -> encode_i64 .wrapping_shl, 63;
        shl_u32: decode_u32 -> encode_u32 .wrapping_shl, 31;
        shl_u64: decode_u64 -> encode_u64 .wrapping_shl, 63;
        shr_i32: decode_i32 -> encode_i32 .wrapping_shr, 31;
        shr_i64: decode_i64 -> encode_i64 .wrapping_shr, 63;
        shr_u32: decode_u32 -> encode_u32 .wrapping_shr, 31;
        shr_u64: decode_u64 -> encode_u64 .wrapping_shr, 63;
    }
    rotate! {
        rol_i32: decode_i32 -> encode_i32 .rotate_left;
        rol_i64: decode_i64 -> encode_i64 .rotate_left;
        rol_u32: decode_u32 -> encode_u32 .rotate_left;
        rol_u64: decode_u64 -> encode_u64 .rotate_left;
        ror_i32: decode_i32 -> encode_i32 .rotate_right;
        ror_i64: decode_i64 -> encode_i64 .rotate_right;
        ror_u32: decode_u32 -> encode_u32 .rotate_right;
        ror_u64: decode_u64 -> encode_u64 .rotate_right;
    }
    bitwise! {
        bitand_i32: decode_i32 -> encode_i32 &; bitand_i64: decode_i64 -> encode_i64 &;
        bitand_u32: decode_u32 -> encode_u32 &; bitand_u64: decode_u64 -> encode_u64 &;
        bitor_i32: decode_i32 -> encode_i32 |; bitor_i64: decode_i64 -> encode_i64 |;
        bitor_u32: decode_u32 -> encode_u32 |; bitor_u64: decode_u64 -> encode_u64 |;
        bitxor_i32: decode_i32 -> encode_i32 ^; bitxor_i64: decode_i64 -> encode_i64 ^;
        bitxor_u32: decode_u32 -> encode_u32 ^; bitxor_u64: decode_u64 -> encode_u64 ^;
    }
    compare! {
        eq_i32: decode_i32, ==; eq_i64: decode_i64, ==; eq_u32: decode_u32, ==; eq_u64: decode_u64, ==; eq_f32: decode_f32, ==; eq_f64: decode_f64, ==;
        ne_i32: decode_i32, !=; ne_i64: decode_i64, !=; ne_u32: decode_u32, !=; ne_u64: decode_u64, !=; ne_f32: decode_f32, !=; ne_f64: decode_f64, !=;
        lt_i32: decode_i32, <; lt_i64: decode_i64, <; lt_u32: decode_u32, <; lt_u64: decode_u64, <; lt_f32: decode_f32, <; lt_f64: decode_f64, <;
        gt_i32: decode_i32, >; gt_i64: decode_i64, >; gt_u32: decode_u32, >; gt_u64: decode_u64, >; gt_f32: decode_f32, >; gt_f64: decode_f64, >;
        le_i32: decode_i32, <=; le_i64: decode_i64, <=; le_u32: decode_u32, <=; le_u64: decode_u64, <=; le_f32: decode_f32, <=; le_f64: decode_f64, <=;
        ge_i32: decode_i32, >=; ge_i64: decode_i64, >=; ge_u32: decode_u32, >=; ge_u64: decode_u64, >=; ge_f32: decode_f32, >=; ge_f64: decode_f64, >=;
    }

    fn op_not(&mut self) -> Result<StepOutcome> {
        let value = !canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(value));
        Ok(StepOutcome::Continue)
    }
    fn op_and(&mut self) -> Result<StepOutcome> {
        let rhs = canonical_bool(self.stack_pop()?)?;
        let lhs = canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(lhs && rhs));
        Ok(StepOutcome::Continue)
    }
    fn op_or(&mut self) -> Result<StepOutcome> {
        let rhs = canonical_bool(self.stack_pop()?)?;
        let lhs = canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(lhs || rhs));
        Ok(StepOutcome::Continue)
    }
    fn op_xor(&mut self) -> Result<StepOutcome> {
        let rhs = canonical_bool(self.stack_pop()?)?;
        let lhs = canonical_bool(self.stack_pop()?)?;
        self.stack_push(encode_bool(lhs ^ rhs));
        Ok(StepOutcome::Continue)
    }
}
