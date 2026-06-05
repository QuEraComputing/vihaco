use eyre::Result;
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Rem, Shl, Shr, Sub};

use crate::StepOutcome;
use crate::data::CPU;
use crate::instruction::Instruction;
use vihaco::Effects;
use vihaco::value::Value;
use vihaco::{component, frame::Frame, traits::*, value::Type};

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
    pub fn execute_instruction(&mut self, inst: Instruction) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        use Instruction::*;
        match inst {
            Span(file, start, end) => self.op_span(file, start, end),
            Label | FunctionStart | FunctionEnd => Ok(StepOutcome::Continue),
            Breakpoint => Ok(StepOutcome::Breakpoint),
            Branch(target) => self.op_branch(target),
            ConditionalBranch(true_target, false_target) => {
                self.op_conditional_branch(true_target, false_target)
            }
            Return(keep) => self.op_return(keep),
            Call(arity, target) => self.op_call(arity, target),
            IndirectCall => self.op_indirect_call(),
            Halt => Ok(StepOutcome::Halt),
            Print => Err(eyre::eyre!(
                "Print must be handled via execute with CPUMessage::Print"
            )),
            Load(ty, addr) => self.op_load(ty, addr),
            Store(ty, addr) => self.op_store(ty, addr),
            Dup => self.op_dup(),
            HeapAlloc(n_elements) => self.op_heap_alloc(n_elements),
            GetItem => self.op_get_item(),
            HeapDealloc => self.op_dealloc_heap(),
            Const(v) => self.op_const(v),
            Add(ty) => self.op_add(ty),
            Sub(ty) => self.op_sub(ty),
            Mul(ty) => self.op_mul(ty),
            Div(ty) => self.op_div(ty),
            Rem(ty) => self.op_rem(ty),
            Neg(ty) => self.op_neg(ty),
            Shl(ty) => self.op_shl(ty),
            Shr(ty) => self.op_shr(ty),
            Rol(ty) => self.op_rol(ty),
            Ror(ty) => self.op_ror(ty),
            BitAnd(ty) => self.op_bitand(ty),
            BitOr(ty) => self.op_bitor(ty),
            BitXor(ty) => self.op_bitxor(ty),
            Not => self.op_not(),
            And => self.op_and(),
            Or => self.op_or(),
            Xor => self.op_xor(),
            Eq(ty) => self.op_eq(ty),
            Ne(ty) => self.op_ne(ty),
            Lt(ty) => self.op_lt(ty),
            Gt(ty) => self.op_gt(ty),
            Le(ty) => self.op_le(ty),
            Ge(ty) => self.op_ge(ty),
        }
    }
}

#[derive(Debug, Clone, PartialEq, vihaco::Message)]
pub enum CPUMessage {
    None,
    FunctionInfo { arity: u32, start_address: u32 },
    Print(String),
}

#[component(instruction = Instruction, message = CPUMessage, effect = StepOutcome)]
impl CPU {
    fn execute(
        &mut self,
        inst: Instruction,
        msg: CPUMessage,
    ) -> eyre::Result<Effects<StepOutcome>> {
        use Instruction::*;
        match (inst, msg) {
            (Print, CPUMessage::Print(text)) => {
                self.stack_pop()?;
                drop(text);
                Ok(Effects::one(StepOutcome::Continue))
            }
            (Print, _) => Err(eyre::eyre!("Print requires CPUMessage::Print")),
            (_, CPUMessage::Print(_)) => Err(eyre::eyre!(
                "CPUMessage::Print is only valid for Print instruction"
            )),
            (
                inst,
                CPUMessage::FunctionInfo {
                    arity,
                    start_address,
                },
            ) => {
                self.stack_push(arity);
                self.stack_push(start_address);
                self.execute_instruction(inst).map(Effects::one)
            }
            (inst, CPUMessage::None) => self.execute_instruction(inst).map(Effects::one),
        }
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
        let cond = self.stack.pop().ok_or(eyre::eyre!("stack underflow"))?;
        match cond {
            Value::Bool(true) => {
                self.set_pending_pc(true_target);
                Ok(StepOutcome::Continue)
            }
            Value::Bool(false) => {
                self.set_pending_pc(false_target);
                Ok(StepOutcome::Continue)
            }
            _ => Err(eyre::eyre!("type error: expected bool on stack")),
        }
    }

    pub fn op_return(&mut self, keep: u32) -> eyre::Result<StepOutcome> {
        let frame = self.pop_frame()?;
        if self.stack.len() - frame.base < (keep as usize) {
            return Err(eyre::eyre!("not enough values to return"));
        }

        // Collect return values before truncating
        let top = self.stack.len() - keep as usize;
        let return_values: Vec<Value> = self.stack[top..].to_vec();
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
        let f = self.stack_pop()?.get_function_ref()?;

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

    fn op_load(&mut self, ty: Type, addr: u32) -> eyre::Result<StepOutcome> {
        // addr should be local to frame.
        let value = self.get_local(addr as usize)?;
        if value.type_of() != ty {
            return Err(eyre::eyre!(format!(
                "type error: expected {:?} at address {}, got {:?}",
                ty,
                addr,
                value.type_of()
            )));
        }
        self.stack_push(*value);
        Ok(StepOutcome::Continue)
    }

    pub fn op_store(&mut self, ty: Type, addr: u32) -> Result<StepOutcome> {
        let v: Value = self.stack_pop()?;
        log::debug!("store value {:?} at addr {}", v, addr);
        if !v.is_undefined() && v.type_of() != ty {
            return Err(eyre::eyre!("Type mismatch"));
        }
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
        let values: Box<[Value]> = self.stack.drain(start..).collect();
        let heap_id = self.push_heap_object(values);
        self.stack_push(Value::HeapRef(heap_id));
        Ok(StepOutcome::Continue)
    }

    pub fn op_get_item(&mut self) -> Result<StepOutcome> {
        let index = Self::heap_index(self.stack_pop()?)?;
        let heap_id = self.stack_pop()?.get_heap_ref()?;
        let value = *self
            .heap_object(heap_id)?
            .get(index)
            .ok_or_else(|| eyre::eyre!("heap index {} out of bounds", index))?;
        self.stack_push(value);
        Ok(StepOutcome::Continue)
    }

    pub fn op_dealloc_heap(&mut self) -> Result<StepOutcome> {
        let id = self.stack_pop()?.get_heap_ref()?;
        self.dealloc_heap_object(id)?;
        Ok(StepOutcome::Continue)
    }

    pub fn op_const(&mut self, v: Value) -> Result<StepOutcome> {
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }

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
    use vihaco::{
        Effects, GeneratedComponent, frame::Frame, instruction::OpCode, traits::StackMemory,
    };

    #[test]
    fn cpu_generated_component_executes_instruction_without_message() {
        let mut cpu = CPU::default();

        GeneratedComponent::execute_generated(
            &mut cpu,
            Instruction::Const(Value::I64(7)),
            CPUMessage::None,
        )
        .unwrap();

        assert_eq!(cpu.stack(), &vec![Value::I64(7)]);
    }

    #[test]
    fn execute_instruction_applies_control_flow_without_action() {
        let mut cpu = CPU::default();

        let branch = cpu.execute_instruction(Instruction::Branch(9)).unwrap();
        assert_eq!(branch, StepOutcome::Continue);
        assert_eq!(cpu.take_pending_pc(), Some(9));

        let halt = cpu.execute_instruction(Instruction::Halt).unwrap();
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
        cpu.stack_push(Value::I64(7));

        let outcome = cpu.execute_instruction(Instruction::Return(1)).unwrap();

        assert_eq!(outcome, StepOutcome::Return);
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
        cpu.execute_instruction(Instruction::Call(0, 100)).unwrap();
        assert_eq!(cpu.take_pending_pc(), Some(100));
        assert_eq!(cpu.frames[1].ret_pc, 11);

        // Callee returns immediately. pending_pc should be restored to the
        // instruction after the call.
        let outcome = cpu.execute_instruction(Instruction::Return(0)).unwrap();
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
        cpu.stack_push(Value::FunctionRef(7));
        cpu.stack_push(Value::U32(0));
        cpu.stack_push(Value::U32(100));

        cpu.execute_instruction(Instruction::IndirectCall).unwrap();
        assert_eq!(cpu.take_pending_pc(), Some(100));
        assert_eq!(cpu.frames[1].ret_pc, 11);

        let outcome = cpu.execute_instruction(Instruction::Return(0)).unwrap();
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
        cpu.stack_push(Value::I64(111)); // scratch — bottom of callee frame
        cpu.stack_push(Value::I64(222)); // scratch — middle
        cpu.stack_push(Value::I64(999)); // intended return value — top

        let outcome = cpu.execute_instruction(Instruction::Return(1)).unwrap();
        assert_eq!(outcome, StepOutcome::Continue);

        assert_eq!(cpu.stack(), &vec![Value::I64(999)],);
    }

    #[test]
    fn op_heap_alloc_preserves_natural_push_order_and_returns_heap_ref() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(10));
        cpu.stack_push(Value::I64(20));
        cpu.stack_push(Value::I64(30));

        let outcome = cpu.execute_instruction(Instruction::HeapAlloc(3)).unwrap();

        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.stack(), &vec![Value::HeapRef(0)]);
        assert_eq!(
            cpu.heap.get(0).unwrap(),
            &[Value::I64(10), Value::I64(20), Value::I64(30)]
        );
    }

    #[test]
    fn op_heap_alloc_supports_empty_heap_objects() {
        let mut cpu = CPU::default();

        let outcome = cpu.execute_instruction(Instruction::HeapAlloc(0)).unwrap();

        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.stack(), &vec![Value::HeapRef(0)]);
        assert_eq!(cpu.heap.get(0).unwrap(), &[] as &[Value]);
    }

    #[test]
    fn op_get_item_reads_heap_value() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(10));
        cpu.stack_push(Value::I64(20));
        cpu.stack_push(Value::I64(30));
        cpu.execute_instruction(Instruction::HeapAlloc(3)).unwrap();
        cpu.stack_push(Value::U32(1));

        let outcome = cpu.execute_instruction(Instruction::GetItem).unwrap();

        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.stack(), &vec![Value::I64(20)]);
    }

    #[test]
    fn op_get_item_rejects_non_heap_refs() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(7));
        cpu.stack_push(Value::U32(0));

        let err = cpu.execute_instruction(Instruction::GetItem).unwrap_err();

        assert!(err.to_string().contains("HeapRef"));
    }

    #[test]
    fn op_get_item_rejects_invalid_heap_ids() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::HeapRef(99));
        cpu.stack_push(Value::U32(0));

        let err = cpu.execute_instruction(Instruction::GetItem).unwrap_err();

        assert!(err.to_string().contains("heap"));
    }

    #[test]
    fn op_get_item_rejects_out_of_bounds_indices() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(10));
        cpu.execute_instruction(Instruction::HeapAlloc(1)).unwrap();
        cpu.stack_push(Value::U32(3));

        let err = cpu.execute_instruction(Instruction::GetItem).unwrap_err();

        assert!(err.to_string().contains("index"));
    }

    #[test]
    fn reset_clears_heap_allocations() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(10));
        cpu.execute_instruction(Instruction::HeapAlloc(1)).unwrap();

        cpu.reset();

        assert!(cpu.heap.is_empty());
        assert!(cpu.stack().is_empty());
    }

    #[test]
    fn cpu_instruction_opcodes_follow_variant_order_without_explicit_attributes() {
        assert_eq!(Instruction::Span(0, 0, 0).opcode(), 0);
        assert_eq!(Instruction::Label.opcode(), 1);
        assert_eq!(Instruction::FunctionStart.opcode(), 2);
        assert_eq!(Instruction::HeapAlloc(1).opcode(), 15);
        assert_eq!(Instruction::Const(Value::I64(1)).opcode(), 18);
        assert_eq!(Instruction::Ge(Type::I64).opcode(), 41);
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
            Instruction::Const(Value::I64(99)),
            CPUMessage::None,
        )
        .unwrap();

        assert_eq!(outcome, Effects::one(StepOutcome::Continue));
        assert_eq!(cpu.stack(), &vec![Value::I64(99)]);
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
            Instruction::Label,
            CPUMessage::FunctionInfo {
                arity: 2,
                start_address: 42,
            },
        )
        .unwrap();

        assert_eq!(outcome, Effects::one(StepOutcome::Continue));
        // arity pushed first, then start_address
        assert_eq!(cpu.stack(), &vec![Value::U32(2), Value::U32(42)]);
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
        cpu.stack_push(Value::I64(42));

        let outcome = GeneratedComponent::execute_generated(
            &mut cpu,
            Instruction::Print,
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
        cpu.stack_push(Value::I64(42));

        let err =
            GeneratedComponent::execute_generated(&mut cpu, Instruction::Print, CPUMessage::None)
                .unwrap_err();

        assert!(err.to_string().contains("Print requires"));
    }

    #[test]
    fn op_dealloc_heap_marks_slot_dead() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(42));
        cpu.execute_instruction(Instruction::HeapAlloc(1)).unwrap();
        cpu.stack_push(Value::HeapRef(0));

        cpu.execute_instruction(Instruction::HeapDealloc).unwrap();

        assert!(cpu.heap.get(0).unwrap_err().to_string().contains("deallocated"));
    }

    #[test]
    fn op_dealloc_heap_slot_is_reused_on_next_alloc() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(1));
        cpu.execute_instruction(Instruction::HeapAlloc(1)).unwrap();
        cpu.execute_instruction(Instruction::HeapDealloc).unwrap();

        cpu.stack_push(Value::I64(2));
        cpu.execute_instruction(Instruction::HeapAlloc(1)).unwrap();

        assert_eq!(cpu.stack(), &vec![Value::HeapRef(0)]);
        assert_eq!(cpu.heap.get(0).unwrap(), &[Value::I64(2)]);
    }

    #[test]
    fn op_dealloc_heap_rejects_double_free() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(1));
        cpu.execute_instruction(Instruction::HeapAlloc(1)).unwrap();
        cpu.stack_push(Value::HeapRef(0));
        cpu.execute_instruction(Instruction::HeapDealloc).unwrap();

        cpu.stack_push(Value::HeapRef(0));
        let err = cpu.execute_instruction(Instruction::HeapDealloc).unwrap_err();

        assert!(err.to_string().contains("double-free"));
    }

    #[test]
    fn op_dealloc_heap_rejects_invalid_id() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::HeapRef(99));

        let err = cpu.execute_instruction(Instruction::HeapDealloc).unwrap_err();

        assert!(err.to_string().contains("invalid heap object id"));
    }

    #[test]
    fn reset_clears_free_list() {
        let mut cpu = CPU::default();
        cpu.stack_push(Value::I64(1));
        cpu.execute_instruction(Instruction::HeapAlloc(1)).unwrap();
        cpu.stack_push(Value::HeapRef(0));
        cpu.execute_instruction(Instruction::HeapDealloc).unwrap();

        cpu.reset();

        assert!(cpu.heap.is_empty());
    }
}

macro_rules! impl_op_num_binary {
    ($name:ident, $op:ident) => {
        pub fn $name(&mut self, ty: Type) -> Result<StepOutcome> {
            let lhs: Value = self.stack_pop()?;
            let rhs: Value = self.stack_pop()?;
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
                    ))
                }
            };
            self.stack.push(output);
            Ok(StepOutcome::Continue)
        }
    };
}

impl CPU {
    impl_op_num_binary!(op_add, add);
    impl_op_num_binary!(op_sub, sub);
    impl_op_num_binary!(op_mul, mul);
    impl_op_num_binary!(op_div, div);
    impl_op_num_binary!(op_rem, rem);

    pub fn op_neg(&mut self, ty: Type) -> Result<StepOutcome> {
        let v: Value = self.stack_pop()?;
        if v.type_of() != ty {
            return Err(eyre::eyre!(format!(
                "Type mismatch, expected {:?} got {:?}",
                ty,
                v.type_of()
            )));
        }

        let output = match v {
            Value::I64(i) => Value::I64(-i),
            Value::F64(f) => Value::F64(-f),
            _ => return Err(eyre::eyre!(format!("cannot negate {}", v.type_of()))),
        };
        self.stack.push(output);
        Ok(StepOutcome::Continue)
    }
}

macro_rules! impl_op_shift {
    ($name:ident, $op:ident) => {
        pub fn $name(&mut self, ty: Type) -> Result<StepOutcome> {
            let rhs: Value = self.stack_pop()?;
            let lhs: Value = self.stack_pop()?;
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
                    )))
                }
            };
            self.stack.push(output);
            Ok(StepOutcome::Continue)
        }
    };
}

impl CPU {
    impl_op_shift!(op_shl, shl);
    impl_op_shift!(op_shr, shr);
}

macro_rules! impl_op_rotate {
    ($name:ident, $op:ident) => {
        pub fn $name(&mut self, ty: Type) -> Result<StepOutcome> {
            let rhs: Value = self.stack_pop()?;
            let lhs: Value = self.stack_pop()?;
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
            Ok(StepOutcome::Continue)
        }
    };
}

impl CPU {
    impl_op_rotate!(op_rol, rotate_left);
    impl_op_rotate!(op_ror, rotate_right);
}

macro_rules! impl_op_bitwise {
    ($name:ident, $op:ident) => {
        pub fn $name(&mut self, ty: Type) -> Result<StepOutcome> {
            let rhs: Value = self.stack_pop()?;
            let lhs: Value = self.stack_pop()?;
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
                    )))
                }
            };
            self.stack.push(output);
            Ok(StepOutcome::Continue)
        }
    };
}

impl CPU {
    impl_op_bitwise!(op_bitand, bitand);
    impl_op_bitwise!(op_bitor, bitor);
    impl_op_bitwise!(op_bitxor, bitxor);
}

macro_rules! impl_boolean_binary {
    ($name:ident, $op:ident) => {
        pub fn $name(&mut self) -> Result<StepOutcome> {
            let rhs: bool = self.stack_pop()?.try_into()?;
            let lhs: bool = self.stack_pop()?.try_into()?;
            let output = lhs.$op(rhs);
            self.stack_push(output);
            Ok(StepOutcome::Continue)
        }
    };
}

impl CPU {
    pub fn op_not(&mut self) -> Result<StepOutcome> {
        let v: bool = self.stack_pop()?.try_into()?;
        self.stack_push(!v);
        Ok(StepOutcome::Continue)
    }

    impl_boolean_binary!(op_and, bitand);
    impl_boolean_binary!(op_or, bitor);
    impl_boolean_binary!(op_xor, bitxor);
}

macro_rules! impl_eq {
    ($name:ident, $op:ident) => {
        pub fn $name(&mut self, ty: Type) -> Result<StepOutcome> {
            let rhs: Value = self.stack_pop()?;
            let lhs: Value = self.stack_pop()?;
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
            Ok(StepOutcome::Continue)
        }
    };
}

impl CPU {
    impl_eq!(op_eq, eq);
    impl_eq!(op_ne, ne);
}

macro_rules! impl_ordering {
    ($name:ident, $op:ident) => {
        pub fn $name(&mut self, ty: Type) -> Result<StepOutcome> {
            let rhs: Value = self.stack_pop()?;
            let lhs: Value = self.stack_pop()?;
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
                    )))
                }
            };
            self.stack_push(output);
            Ok(StepOutcome::Continue)
        }
    };
}

impl CPU {
    impl_ordering!(op_lt, lt);
    impl_ordering!(op_le, le);
    impl_ordering!(op_gt, gt);
    impl_ordering!(op_ge, ge);
}
