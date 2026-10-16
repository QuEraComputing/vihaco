// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]

use vihaco::{
    Dispatch, Execute, HasInstructionSet, Parse, Step,
    chumsky::Parser as _,
    composite, field,
    traits::{FrameMemory, StackFrame, StackMemory},
};
use vihaco_cpu::{
    CPU, StepOutcome, cpu, encode_f32, encode_f64, encode_function_ref, encode_i64,
    lower::{FunctionLookup, LabelLookup, StringIntern},
    message,
    operand::Target,
};

// These tests exercise resolved numeric operands. Capability methods are never
// needed for them, but aggregate lowering still requires the complete contract.
struct NumericContext;
impl LabelLookup for NumericContext {
    fn label_address(&self, name: &str) -> eyre::Result<u32> {
        Err(eyre::eyre!("unknown label {name}"))
    }
}
impl FunctionLookup for NumericContext {
    fn function_address(&self, name: &str) -> eyre::Result<u32> {
        Err(eyre::eyre!("unknown function {name}"))
    }
    fn function_index(&self, name: &str) -> eyre::Result<u32> {
        Err(eyre::eyre!("unknown function {name}"))
    }
}
impl StringIntern for NumericContext {
    fn intern_string(&mut self, _: &str) -> eyre::Result<u32> {
        Err(eyre::eyre!("numeric-only fixture cannot intern strings"))
    }
}

pub use vihaco_cpu::{__vihaco_fields_CPU, __vihaco_inventory_CPU};

#[composite]
struct Controller {
    #[device(7, alias = "host")]
    cpu: CPU,
    last: Option<eyre::Result<StepOutcome>>,
}

#[composite]
struct Machine {
    #[device(2, alias = "ctl")]
    controller: Controller,
    #[device(42)]
    spare: CPU,
    last: Option<eyre::Result<StepOutcome>>,
}

macro_rules! steps {
    ($machine:ty, $route:ty, $($device:ident).+) => {
        ordinary_steps!(
            $machine, $route, ($($device).+);
            Span, Breakpoint, Branch, ConditionalBranch, Return, Halt,
            LoadI32, LoadI64, LoadU32, LoadU64, LoadF32, LoadF64,
            LoadBool, StoreI32, StoreI64, StoreU32, StoreU64, StoreF32,
            StoreF64, StoreBool, Dup, HeapAlloc, GetItem, HeapDealloc,
            HeapReserve, HeapPush, HeapLen,
            ConstI32, ConstI64, ConstU32, ConstU64, ConstF32, ConstF64,
            ConstBool, ConstString, ConstFunctionRef, ConstHeapRef, AddI32, AddF32,
            AddI64, AddU32, AddU64, AddF64, SubI32, SubI64,
            SubU32, SubU64, SubF32, SubF64, MulI32, MulI64,
            MulU32, MulU64, MulF32, MulF64, DivI32, DivI64,
            DivU32, DivU64, DivF32, DivF64, RemI32, RemI64,
            RemU32, RemU64, RemF32, RemF64, NegI32, NegI64,
            NegF32, NegF64, ShlI32, ShlI64, ShlU32, ShlU64,
            ShrI32, ShrI64, ShrU32, ShrU64, RolI32, RolI64,
            RolU32, RolU64, RorI32, RorI64, RorU32, RorU64,
            BitAndI32, BitAndI64, BitAndU32, BitAndU64, BitOrI32, BitOrI64,
            BitOrU32, BitOrU64, BitXorI32, BitXorI64, BitXorU32, BitXorU64,
            Not, And, Or, Xor, EqI32, EqI64,
            EqU32, EqU64, EqF32, EqF64, NeI32, NeI64,
            NeU32, NeU64, NeF32, NeF64, LtI32, LtI64,
            LtU32, LtU64, LtF32, LtF64, GtI32, GtI64,
            GtU32, GtU64, GtF32, GtF64, LeI32, LeI64,
            LeU32, LeU64, LeF32, LeF64, GeI32, GeI64,
            GeU32, GeU64, GeF32, GeF64
        );
        impl Step<$route, cpu::Call> for $machine {
            fn step(&mut self, instruction: cpu::Call) {
                self.last = Some(self.$($device).+.execute(&instruction, message::FunctionInfo {
                    arity: instruction.0, start_address: instruction.1, local_count: instruction.0,
                }));
            }
        }
        impl Step<$route, cpu::IndirectCall> for $machine {
            fn step(&mut self, instruction: cpu::IndirectCall) {
                self.last = Some(self.$($device).+.execute(&instruction, message::FunctionInfo {
                    arity: 0, start_address: 0, local_count: 0,
                }));
            }
        }
        impl Step<$route, cpu::Print> for $machine {
            fn step(&mut self, instruction: cpu::Print) {
                self.last = Some(self.$($device).+.execute(&instruction, message::Print("text".into())));
            }
        }
    };
}
macro_rules! ordinary_steps {
    ($machine:ty, $route:ty, $device:tt; $($instruction:ident),*) => {$(
        ordinary_step!($machine, $route, $device, $instruction);
    )*};
}
macro_rules! ordinary_step {
    ($machine:ty, $route:ty, ($($device:ident).+), $instruction:ident) => {
        impl Step<$route, cpu::$instruction> for $machine {
            fn step(&mut self, instruction: cpu::$instruction) {
                self.last = Some(self.$($device).+.execute(&instruction, ()));
            }
        }
    };
}
steps!(Controller, field!(Controller::cpu), cpu);
steps!(Machine, field!(Machine::controller::cpu), controller.cpu);
steps!(Machine, field!(Machine::spare), spare);

fn machine() -> Machine {
    Machine {
        controller: Controller {
            cpu: CPU::default(),
            last: None,
        },
        spare: CPU::default(),
        last: None,
    }
}

fn step(machine: &mut Machine, source: &str) -> eyre::Result<StepOutcome> {
    let instruction = <Machine as HasInstructionSet>::Syntax::parser()
        .parse(source)
        .into_result()
        .unwrap();
    machine.dispatch(vihaco::Lower::lower(instruction, &mut NumericContext).unwrap());
    machine.last.take().unwrap()
}

#[test]
fn direct_runtime_parser_dispatches_to_nested_aliased_and_repeated_cpu_fields() {
    let mut machine = machine();
    step(&mut machine, "ctl::host::cpu::const_i64 -42").unwrap();
    step(&mut machine, "controller::cpu::cpu::const_i64 7").unwrap();
    step(&mut machine, "ctl::host::cpu::add_i64").unwrap();
    step(&mut machine, "spare::cpu::const_f32 1.5").unwrap();
    assert_eq!(machine.controller.cpu.stack(), &[encode_i64(-35)]);
    assert_eq!(machine.spare.stack(), &[encode_f32(1.5)]);
    assert_eq!(
        step(&mut machine, "ctl::host::cpu::br 10").unwrap(),
        StepOutcome::Continue
    );
    assert_eq!(machine.controller.cpu.take_pending_pc(), Some(10));
    assert_eq!(
        step(&mut machine, "spare::cpu::halt").unwrap(),
        StepOutcome::Halt
    );
    assert_eq!(
        step(&mut machine, "spare::cpu::breakpoint").unwrap(),
        StepOutcome::Breakpoint
    );
}

#[test]
fn readable_constants_parse_to_encoded_runtime_words() {
    assert_eq!(
        vihaco::Lower::lower(
            cpu::ConstI32::<vihaco::Syntax>::parser()
                .parse("const_i32 -7")
                .into_result()
                .unwrap(),
            &mut NumericContext
        )
        .unwrap(),
        cpu::ConstI32(vihaco_cpu::encode_i32(-7))
    );
    assert_eq!(
        vihaco::Lower::lower(
            cpu::ConstF32::<vihaco::Syntax>::parser()
                .parse("const_f32 -0.5")
                .into_result()
                .unwrap(),
            &mut NumericContext
        )
        .unwrap(),
        cpu::ConstF32(encode_f32(-0.5))
    );
    assert!(
        cpu::ConstString::<vihaco::Syntax>::parser()
            .parse("const_string 4294967296")
            .has_errors()
    );
    assert_eq!(
        vihaco::Lower::lower(
            cpu::ConstI64::<vihaco::Syntax>::parser()
                .parse("const_i64 -42")
                .into_result()
                .unwrap(),
            &mut NumericContext
        )
        .unwrap(),
        cpu::ConstI64(encode_i64(-42))
    );
    assert_eq!(
        vihaco::Lower::lower(
            cpu::ConstF64::<vihaco::Syntax>::parser()
                .parse("const_f64 -1.5")
                .into_result()
                .unwrap(),
            &mut NumericContext
        )
        .unwrap(),
        cpu::ConstF64(encode_f64(-1.5))
    );
    assert_eq!(
        vihaco::Lower::lower(
            cpu::ConstBool::<vihaco::Syntax>::parser()
                .parse("const_bool true")
                .into_result()
                .unwrap(),
            &mut NumericContext
        )
        .unwrap(),
        cpu::ConstBool(1)
    );
    assert_eq!(
        vihaco::Lower::lower(
            cpu::ConstString::<vihaco::Syntax>::parser()
                .parse("const_string 3")
                .into_result()
                .unwrap(),
            &mut NumericContext
        )
        .unwrap(),
        cpu::ConstString(3)
    );
    assert_eq!(
        vihaco::Lower::lower(
            cpu::ConstFunctionRef::<vihaco::Syntax>::parser()
                .parse("const_fn_ref 4")
                .into_result()
                .unwrap(),
            &mut NumericContext
        )
        .unwrap(),
        cpu::ConstFunctionRef(encode_function_ref(4))
    );
    let branch = cpu::Branch::<vihaco::Syntax>::parser()
        .parse("br @later")
        .into_result()
        .unwrap();
    assert_eq!(
        branch.0,
        Target::Symbol(vihaco_parser::Ident("later".to_owned()))
    );
    assert!(vihaco::Lower::lower(branch, &mut NumericContext).is_err());
    assert!(
        cpu::ConstI32::<vihaco::Syntax>::parser()
            .parse("const_i32 2147483648")
            .has_errors()
    );
    assert!(cpu::ConstU64::parser().parse("const_u64 -1").has_errors());
    assert!(
        cpu::ConstString::<vihaco::Syntax>::parser()
            .parse("const_string \"hello\"")
            .has_output()
    );
}

#[test]
fn sst_labels_are_readability_metadata_and_do_not_occupy_runtime_slots() {
    type Function =
        vihaco::syntax::ParsedFunction<<Machine as HasInstructionSet>::Syntax, vihaco::Type>;
    let parsed = Function::parser()
        .parse("fn @main() { label @entry ctl::host::cpu::br 1 label @later spare::cpu::halt }")
        .into_result()
        .unwrap();
    assert_eq!(parsed.body.len(), 2);
    assert_eq!(parsed.labels[0].name.as_str(), "entry");
    assert_eq!(parsed.labels[0].instruction_index, 0);
    assert_eq!(parsed.labels[1].name.as_str(), "later");
    assert_eq!(parsed.labels[1].instruction_index, 1);
    assert!(matches!(
        &parsed.body[0],
        __vihaco_MachineInstruction::V_controller__cpu__cpu__Branch(cpu::Branch(Target::Resolved(
            1
        )))
    ));
    let mut machine = machine();
    for instruction in parsed.body {
        machine.dispatch(vihaco::Lower::lower(instruction, &mut NumericContext).unwrap());
    }
    assert_eq!(machine.controller.cpu.take_pending_pc(), Some(1));
    assert_eq!(machine.last.take().unwrap().unwrap(), StepOutcome::Halt);
    assert!(
        Function::parser()
            .parse("fn @main() { ctl::host::cpu::function_start }")
            .has_errors()
    );
    assert!(
        Function::parser()
            .parse("fn @main() { ctl::host::cpu::function_end }")
            .has_errors()
    );
}

#[test]
fn typed_messages_preserve_direct_indirect_calls_and_returns() {
    let mut cpu = CPU::default();
    cpu.stack_mut().extend([10, 20]);
    cpu.start_function(2, 0, 4, 0).unwrap();
    cpu.stack_push(99_u64);
    cpu.stack_push(30_u64);
    cpu.set_current_pc(7);
    cpu.execute(
        &cpu::Call(1, 100),
        message::FunctionInfo {
            arity: 999,
            start_address: 999,
            local_count: 3,
        },
    )
    .unwrap();
    assert_eq!(cpu.get_frame().unwrap().local_count, 3);
    assert_eq!(cpu.take_pending_pc(), Some(100));
    cpu.stack_push(44_u64);
    assert_eq!(
        cpu.execute(&cpu::Return(1), ()).unwrap(),
        StepOutcome::Continue
    );
    assert_eq!(cpu.take_pending_pc(), Some(8));
    assert_eq!(cpu.stack(), &[10, 20, 0, 0, 99, 44]);
    cpu.stack_push(encode_function_ref(2));
    cpu.execute(
        &cpu::IndirectCall,
        message::FunctionInfo {
            arity: 1,
            start_address: 200,
            local_count: 2,
        },
    )
    .unwrap();
    assert_eq!(cpu.get_frame().unwrap().function, Some(2));
    assert_eq!(cpu.get_local(0).unwrap(), &44);
    assert_eq!(cpu.take_pending_pc(), Some(200));
    cpu.execute(&cpu::Return(0), ()).unwrap();
    assert_eq!(cpu.stack(), &[10, 20, 0, 0, 99]);
    cpu.execute(&cpu::Print, message::Print("99".into()))
        .unwrap();
    assert_eq!(cpu.operand_count(), 0);
    assert_eq!(
        cpu.execute(&cpu::Return(0), ()).unwrap(),
        StepOutcome::Return
    );
}

#[test]
fn checked_execution_keeps_operand_boundaries_and_runtime_errors() {
    let mut cpu = CPU::default();
    assert!(cpu.execute(&cpu::AddI64, ()).is_err());
    cpu.start_function(0, 0, 2, 0).unwrap();
    assert!(cpu.execute(&cpu::Dup, ()).is_err());
    assert_eq!(cpu.stack(), &[0, 0]);
    cpu.stack_push(5_u64);
    assert!(cpu.execute(&cpu::StoreI64(2), ()).is_err());
    assert_eq!(cpu.stack(), &[0, 0, 5]);
    cpu.execute(&cpu::StoreI64(1), ()).unwrap();
    cpu.execute(&cpu::LoadI64(1), ()).unwrap();
    assert_eq!(cpu.stack(), &[0, 5, 5]);
    cpu.stack_push(0_u64);
    assert!(cpu.execute(&cpu::DivI64, ()).is_err());
    cpu.stack_push(2_u64);
    assert!(cpu.execute(&cpu::Not, ()).is_err());
    assert!(cpu.execute(&cpu::LoadI64(2), ()).is_err());
    assert!(cpu.execute(&cpu::Return(1), ()).is_err());
    assert!(
        cpu.execute(
            &cpu::Call(1, 10),
            message::FunctionInfo {
                arity: 1,
                start_address: 10,
                local_count: 1
            }
        )
        .is_err()
    );
}

#[test]
fn heap_and_boolean_execution_retain_checked_behavior() {
    let mut cpu = CPU::default();
    cpu.stack_mut().extend([10, 20]);
    cpu.execute(&cpu::HeapAlloc(2), ()).unwrap();
    cpu.execute(&cpu::Dup, ()).unwrap();
    cpu.stack_push(1_u64);
    cpu.execute(&cpu::GetItem, ()).unwrap();
    assert_eq!(cpu.stack_pop().unwrap(), 20);
    cpu.execute(&cpu::HeapDealloc, ()).unwrap();
    cpu.stack_mut().extend([vihaco_cpu::encode_heap_ref(0), 0]);
    assert!(cpu.execute(&cpu::GetItem, ()).is_err());
    cpu.stack_mut().extend([1, 0]);
    cpu.execute(&cpu::Or, ()).unwrap();
    cpu.execute(&cpu::ConditionalBranch(10, 20), ()).unwrap();
    assert_eq!(cpu.take_pending_pc(), Some(10));
    cpu.stack_mut().extend([0, 0]);
    cpu.execute(&cpu::EqI64, ()).unwrap();
    assert_eq!(cpu.stack_pop().unwrap(), 1);
}

#[test]
fn reserved_heap_instructions_parse_and_dispatch_through_nested_cpu() {
    let mut machine = machine();
    let cpu = &mut machine.controller.cpu;
    cpu.stack_mut().extend([42, 2]);
    assert_eq!(
        step(&mut machine, "ctl::host::cpu::heap_reserve").unwrap(),
        StepOutcome::Continue
    );
    let heap_ref = *machine.controller.cpu.stack_top().unwrap();
    step(&mut machine, "ctl::host::cpu::dup").unwrap();
    step(&mut machine, "ctl::host::cpu::heap_len").unwrap();
    assert_eq!(machine.controller.cpu.stack_pop().unwrap(), 0);
    for value in [10_u64, 20] {
        machine.controller.cpu.stack_push(value);
        step(&mut machine, "ctl::host::cpu::br 99").unwrap();
        step(&mut machine, "ctl::host::cpu::heap_push").unwrap();
        assert_eq!(machine.controller.cpu.take_pending_pc(), None);
        assert_eq!(machine.controller.cpu.stack(), &[42, heap_ref]);
    }
    step(&mut machine, "ctl::host::cpu::dup").unwrap();
    step(&mut machine, "ctl::host::cpu::heap_len").unwrap();
    assert_eq!(machine.controller.cpu.stack_pop().unwrap(), 2);
    machine.controller.cpu.stack_push(30_u64);
    assert!(
        step(&mut machine, "ctl::host::cpu::heap_push")
            .unwrap_err()
            .to_string()
            .contains("full")
    );
    assert_eq!(machine.controller.cpu.stack(), &[42]);
    machine.controller.cpu.stack_mut().extend([heap_ref, 1]);
    step(&mut machine, "ctl::host::cpu::get_item").unwrap();
    assert_eq!(machine.controller.cpu.stack(), &[42, 20]);
}

#[test]
fn heap_append_instructions_respect_frame_operand_boundaries() {
    let mut cpu = CPU::default();
    cpu.start_function(0, 0, 2, 0).unwrap();
    assert!(cpu.execute(&cpu::HeapReserve, ()).is_err());
    assert!(cpu.execute(&cpu::HeapLen, ()).is_err());
    assert!(cpu.execute(&cpu::HeapPush, ()).is_err());
    assert_eq!(cpu.stack(), &[0, 0]);
    cpu.stack_push(0_u64);
    assert!(cpu.execute(&cpu::HeapPush, ()).is_err());
    assert_eq!(cpu.stack(), &[0, 0, 0]);
}

#[test]
fn heap_append_instructions_reject_invalid_and_deallocated_objects() {
    let mut cpu = CPU::default();
    cpu.stack_push(1_u64);
    cpu.execute(&cpu::HeapReserve, ()).unwrap();
    let heap_ref = *cpu.stack_top().unwrap();
    cpu.execute(&cpu::HeapDealloc, ()).unwrap();
    for (reference, message) in [
        (heap_ref, "deallocated"),
        (vihaco_cpu::encode_heap_ref(42), "invalid heap object id"),
    ] {
        cpu.stack_push(reference);
        assert!(
            cpu.execute(&cpu::HeapLen, ())
                .unwrap_err()
                .to_string()
                .contains(message)
        );
        cpu.stack_mut().extend([reference, 7]);
        assert!(
            cpu.execute(&cpu::HeapPush, ())
                .unwrap_err()
                .to_string()
                .contains(message)
        );
    }
    cpu.stack_push(1_u64);
    cpu.execute(&cpu::HeapReserve, ()).unwrap();
    assert_eq!(*cpu.stack_top().unwrap(), heap_ref);
    cpu.stack_push(7_u64);
    cpu.execute(&cpu::HeapPush, ()).unwrap();
    cpu.execute(&cpu::HeapLen, ()).unwrap();
    assert_eq!(cpu.stack(), &[1]);
}

#[test]
fn heap_push_rejects_full_allocations_and_get_item_checks_current_length() {
    let mut cpu = CPU::default();
    cpu.stack_push(0_u64);
    cpu.execute(&cpu::HeapReserve, ()).unwrap();
    cpu.stack_push(7_u64);
    assert!(
        cpu.execute(&cpu::HeapPush, ())
            .unwrap_err()
            .to_string()
            .contains("full")
    );
    cpu.stack_push(7_u64);
    cpu.execute(&cpu::HeapAlloc(1), ()).unwrap();
    cpu.stack_push(8_u64);
    assert!(
        cpu.execute(&cpu::HeapPush, ())
            .unwrap_err()
            .to_string()
            .contains("full")
    );
    cpu.stack_push(2_u64);
    cpu.execute(&cpu::HeapReserve, ()).unwrap();
    cpu.stack_push(7_u64);
    cpu.execute(&cpu::HeapPush, ()).unwrap();
    cpu.stack_push(1_u64);
    assert!(
        cpu.execute(&cpu::GetItem, ())
            .unwrap_err()
            .to_string()
            .contains("out of bounds")
    );
}

#[test]
fn every_executable_leaf_parses_directly_and_metadata_is_excluded() {
    fn check<T>(source: &str, expected: T)
    where
        T: vihaco::__private::InstructionSpec + PartialEq + std::fmt::Debug,
        T::Syntax: for<'src> Parse<'src> + vihaco::Lower<NumericContext, Runtime = T>,
    {
        assert_eq!(
            vihaco::Lower::lower(
                T::Syntax::parser()
                    .parse(source)
                    .into_result()
                    .unwrap_or_else(|errors| panic!("parse({source:?}): {errors:?}")),
                &mut NumericContext
            )
            .unwrap(),
            expected
        );
    }
    check("span 0 0 0", cpu::Span(0, 0, 0));
    check("breakpoint", cpu::Breakpoint);
    check("br 0", cpu::Branch(0));
    check("cond_br 0, 0", cpu::ConditionalBranch(0, 0));
    check("ret 0", cpu::Return(0));
    check("call_indirect", cpu::IndirectCall);
    check("call 0, 0", cpu::Call(0, 0));
    check("halt", cpu::Halt);
    check("print", cpu::Print);
    check("load_i32 0", cpu::LoadI32(0));
    check("load_i64 0", cpu::LoadI64(0));
    check("load_u32 0", cpu::LoadU32(0));
    check("load_u64 0", cpu::LoadU64(0));
    check("load_f32 0", cpu::LoadF32(0));
    check("load_f64 0", cpu::LoadF64(0));
    check("load_bool 0", cpu::LoadBool(0));
    check("store_i32 0", cpu::StoreI32(0));
    check("store_i64 0", cpu::StoreI64(0));
    check("store_u32 0", cpu::StoreU32(0));
    check("store_u64 0", cpu::StoreU64(0));
    check("store_f32 0", cpu::StoreF32(0));
    check("store_f64 0", cpu::StoreF64(0));
    check("store_bool 0", cpu::StoreBool(0));
    check("dup", cpu::Dup);
    check("heap_alloc 0", cpu::HeapAlloc(0));
    check("get_item", cpu::GetItem);
    check("heap_dealloc", cpu::HeapDealloc);
    check("heap_reserve", cpu::HeapReserve);
    check("heap_push", cpu::HeapPush);
    check("heap_len", cpu::HeapLen);
    check("const_i32 0", cpu::ConstI32(0));
    check("const_i64 0", cpu::ConstI64(0));
    check("const_u32 0", cpu::ConstU32(0));
    check("const_u64 0", cpu::ConstU64(0));
    check("const_f32 0", cpu::ConstF32(0));
    check("const_f64 0", cpu::ConstF64(0));
    check("const_bool false", cpu::ConstBool(0));
    check("const_string 0", cpu::ConstString(0));
    check("const_fn_ref 0", cpu::ConstFunctionRef(0));
    check("const_heap_ref 0", cpu::ConstHeapRef(0));
    check("add_i32", cpu::AddI32);
    check("add_f32", cpu::AddF32);
    check("add_i64", cpu::AddI64);
    check("add_u32", cpu::AddU32);
    check("add_u64", cpu::AddU64);
    check("add_f64", cpu::AddF64);
    check("sub_i32", cpu::SubI32);
    check("sub_i64", cpu::SubI64);
    check("sub_u32", cpu::SubU32);
    check("sub_u64", cpu::SubU64);
    check("sub_f32", cpu::SubF32);
    check("sub_f64", cpu::SubF64);
    check("mul_i32", cpu::MulI32);
    check("mul_i64", cpu::MulI64);
    check("mul_u32", cpu::MulU32);
    check("mul_u64", cpu::MulU64);
    check("mul_f32", cpu::MulF32);
    check("mul_f64", cpu::MulF64);
    check("div_i32", cpu::DivI32);
    check("div_i64", cpu::DivI64);
    check("div_u32", cpu::DivU32);
    check("div_u64", cpu::DivU64);
    check("div_f32", cpu::DivF32);
    check("div_f64", cpu::DivF64);
    check("rem_i32", cpu::RemI32);
    check("rem_i64", cpu::RemI64);
    check("rem_u32", cpu::RemU32);
    check("rem_u64", cpu::RemU64);
    check("rem_f32", cpu::RemF32);
    check("rem_f64", cpu::RemF64);
    check("neg_i32", cpu::NegI32);
    check("neg_i64", cpu::NegI64);
    check("neg_f32", cpu::NegF32);
    check("neg_f64", cpu::NegF64);
    check("shl_i32", cpu::ShlI32);
    check("shl_i64", cpu::ShlI64);
    check("shl_u32", cpu::ShlU32);
    check("shl_u64", cpu::ShlU64);
    check("shr_i32", cpu::ShrI32);
    check("shr_i64", cpu::ShrI64);
    check("shr_u32", cpu::ShrU32);
    check("shr_u64", cpu::ShrU64);
    check("rol_i32", cpu::RolI32);
    check("rol_i64", cpu::RolI64);
    check("rol_u32", cpu::RolU32);
    check("rol_u64", cpu::RolU64);
    check("ror_i32", cpu::RorI32);
    check("ror_i64", cpu::RorI64);
    check("ror_u32", cpu::RorU32);
    check("ror_u64", cpu::RorU64);
    check("bitand_i32", cpu::BitAndI32);
    check("bitand_i64", cpu::BitAndI64);
    check("bitand_u32", cpu::BitAndU32);
    check("bitand_u64", cpu::BitAndU64);
    check("bitor_i32", cpu::BitOrI32);
    check("bitor_i64", cpu::BitOrI64);
    check("bitor_u32", cpu::BitOrU32);
    check("bitor_u64", cpu::BitOrU64);
    check("bitxor_i32", cpu::BitXorI32);
    check("bitxor_i64", cpu::BitXorI64);
    check("bitxor_u32", cpu::BitXorU32);
    check("bitxor_u64", cpu::BitXorU64);
    check("not", cpu::Not);
    check("and", cpu::And);
    check("or", cpu::Or);
    check("xor", cpu::Xor);
    check("eq_i32", cpu::EqI32);
    check("eq_i64", cpu::EqI64);
    check("eq_u32", cpu::EqU32);
    check("eq_u64", cpu::EqU64);
    check("eq_f32", cpu::EqF32);
    check("eq_f64", cpu::EqF64);
    check("ne_i32", cpu::NeI32);
    check("ne_i64", cpu::NeI64);
    check("ne_u32", cpu::NeU32);
    check("ne_u64", cpu::NeU64);
    check("ne_f32", cpu::NeF32);
    check("ne_f64", cpu::NeF64);
    check("lt_i32", cpu::LtI32);
    check("lt_i64", cpu::LtI64);
    check("lt_u32", cpu::LtU32);
    check("lt_u64", cpu::LtU64);
    check("lt_f32", cpu::LtF32);
    check("lt_f64", cpu::LtF64);
    check("gt_i32", cpu::GtI32);
    check("gt_i64", cpu::GtI64);
    check("gt_u32", cpu::GtU32);
    check("gt_u64", cpu::GtU64);
    check("gt_f32", cpu::GtF32);
    check("gt_f64", cpu::GtF64);
    check("le_i32", cpu::LeI32);
    check("le_i64", cpu::LeI64);
    check("le_u32", cpu::LeU32);
    check("le_u64", cpu::LeU64);
    check("le_f32", cpu::LeF32);
    check("le_f64", cpu::LeF64);
    check("ge_i32", cpu::GeI32);
    check("ge_i64", cpu::GeI64);
    check("ge_u32", cpu::GeU32);
    check("ge_u64", cpu::GeU64);
    check("ge_f32", cpu::GeF32);
    check("ge_f64", cpu::GeF64);
    assert_eq!(vihaco::__private::capabilities::<CPU>().count(), 142);
    assert_eq!(vihaco::__private::capabilities::<Machine>().count(), 284);
    assert!(
        !vihaco::__private::capabilities::<CPU>()
            .any(|leaf| matches!(leaf.instruction, "label" | "func_start" | "func_end"))
    );
}

#[test]
fn arithmetic_shift_comparison_and_reset_semantics_are_preserved() {
    use vihaco::{Reset, traits::StackMemory};
    use vihaco_cpu::{encode_heap_ref, encode_i32, encode_u32};
    let mut cpu = CPU::default();
    cpu.stack_mut()
        .extend([encode_i32(i32::MAX), encode_i32(1)]);
    cpu.execute(&cpu::AddI32, ()).unwrap();
    assert_eq!(cpu.stack_pop().unwrap(), encode_i32(i32::MIN));
    cpu.stack_mut().extend([encode_i32(-8), encode_u32(33)]);
    cpu.execute(&cpu::ShrI32, ()).unwrap();
    assert_eq!(cpu.stack_pop().unwrap(), encode_i32(-4));
    cpu.stack_mut().extend([encode_f64(1.5), encode_f64(2.0)]);
    cpu.execute(&cpu::MulF64, ()).unwrap();
    assert_eq!(cpu.stack_pop().unwrap(), encode_f64(3.0));
    cpu.stack_mut().extend([encode_i64(-1), encode_i64(1)]);
    cpu.execute(&cpu::LtI64, ()).unwrap();
    assert_eq!(cpu.stack_pop().unwrap(), 1);
    cpu.stack_mut()
        .extend([encode_i64(i64::MIN), encode_i64(-1)]);
    assert!(cpu.execute(&cpu::DivI64, ()).is_err());
    cpu.stack_push(7_u64);
    cpu.execute(&cpu::HeapAlloc(1), ()).unwrap();
    assert_eq!(cpu.stack_pop().unwrap(), encode_heap_ref(0));
    cpu.execute(&cpu::Branch(99), ()).unwrap();
    cpu.execute(&cpu::ConstU64(1), ()).unwrap();
    assert_eq!(cpu.take_pending_pc(), None);
    cpu.start_function(0, 10, 2, 0).unwrap();
    cpu.reset();
    assert!(cpu.stack().is_empty());
    assert!(cpu.get_frame().is_err());
    assert_eq!(cpu.take_pending_pc(), None);
    cpu.stack_mut().extend([encode_heap_ref(0), 0]);
    assert!(cpu.execute(&cpu::GetItem, ()).is_err());
}

#[test]
fn sst_signatures_parse_runtime_types_without_surface_types() {
    type Function =
        vihaco::syntax::ParsedFunction<<Machine as HasInstructionSet>::Syntax, vihaco::Type>;
    let parsed = Function::parser()
        .parse("fn @main(value: i64, target: fn_ref, text: str) -> bool { spare::cpu::halt }")
        .into_result()
        .unwrap();
    assert_eq!(
        parsed
            .params
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>(),
        [
            vihaco::Type::I64,
            vihaco::Type::FunctionRef,
            vihaco::Type::String
        ]
    );
    assert_eq!(parsed.return_ty, Some(vihaco::Type::Bool));
    assert_eq!(parsed.body.len(), 1);
    assert_eq!(
        vihaco::Type::parser()
            .parse("heap_ref")
            .into_result()
            .unwrap(),
        vihaco::Type::HeapRef
    );
    assert_eq!(
        vihaco::Type::parser().parse("undef").into_result().unwrap(),
        vihaco::Type::Undefined
    );
}

// A consumer loader supplies capabilities once; generated composite lowering
// reuses CPU's instruction implementations at every device destination.
#[derive(Default)]
struct SymbolLoader {
    labels: std::collections::HashMap<String, u32>,
    functions: std::collections::HashMap<String, (u32, u32)>,
    image: vihaco::ProgramImage<<Machine as HasInstructionSet>::Runtime, vihaco::NoContext>,
}
impl LabelLookup for SymbolLoader {
    fn label_address(&self, name: &str) -> eyre::Result<u32> {
        self.labels
            .get(name)
            .copied()
            .ok_or_else(|| eyre::eyre!("unknown label {name}"))
    }
}
impl FunctionLookup for SymbolLoader {
    fn function_address(&self, name: &str) -> eyre::Result<u32> {
        self.functions
            .get(name)
            .map(|(_, address)| *address)
            .ok_or_else(|| eyre::eyre!("unknown function {name}"))
    }
    fn function_index(&self, name: &str) -> eyre::Result<u32> {
        self.functions
            .get(name)
            .map(|(index, _)| *index)
            .ok_or_else(|| eyre::eyre!("unknown function {name}"))
    }
}
impl StringIntern for SymbolLoader {
    fn intern_string(&mut self, text: &str) -> eyre::Result<u32> {
        self.image.intern_string(text)
    }
}

struct Header;
impl vihaco::traits::FromText for Header {
    fn from_text(_: &str) -> eyre::Result<Self> {
        Ok(Self)
    }
}
impl vihaco::SstHeader for Header {}
impl vihaco::LoadOwnSstSection<vihaco::NoContext> for SymbolLoader
where
    <Machine as HasInstructionSet>::Syntax:
        vihaco::Lower<Self, Runtime = <Machine as HasInstructionSet>::Runtime>,
{
    fn load_own_sst_section<'src>(
        &mut self,
        section: vihaco::SstSectionView<'src, vihaco::NoContext>,
    ) -> eyre::Result<()> {
        type Module = vihaco::syntax::ParsedModule<
            <Machine as HasInstructionSet>::Syntax,
            vihaco::Type,
            Header,
        >;
        let parsed = Module::parse_section(section)?;
        // Gather all function starts before lowering any body, including forward calls.
        self.functions.clear();
        let mut address = 0u32;
        for (index, function) in parsed.functions.iter().enumerate() {
            if self
                .functions
                .insert(
                    function.name.as_str().to_owned(),
                    (u32::try_from(index)?, address),
                )
                .is_some()
            {
                return Err(eyre::eyre!("duplicate function {}", function.name));
            }
            address = address
                .checked_add(u32::try_from(function.body.len())?)
                .ok_or_else(|| eyre::eyre!("program address overflow"))?;
        }
        let mut code = Vec::new();
        for function in parsed.functions {
            // Label names are scoped to each function, but their addresses are
            // relative to the complete executable section.
            self.labels.clear();
            let base = u32::try_from(code.len())?;
            for label in function.labels {
                let address = base
                    .checked_add(u32::try_from(label.instruction_index)?)
                    .ok_or_else(|| eyre::eyre!("label address overflow"))?;
                if self
                    .labels
                    .insert(label.name.as_str().to_owned(), address)
                    .is_some()
                {
                    return Err(eyre::eyre!("duplicate label {}", label.name));
                }
            }
            code.extend(self.lower_instructions(function.body)?);
        }
        self.image.module.code = code;
        Ok(())
    }
}
fn load_symbolic(loader: &mut SymbolLoader, functions: &str) -> eyre::Result<()> {
    use vihaco::LoadOwnSstSection;
    let text = format!(
        "sst v{}\n.section(root):\n.text(root):\n{functions}\n.text(root).\n.section(root).\n",
        vihaco::VERSION
    );
    let file = vihaco::SstFile::<vihaco::NoContext>::from_text(&text)?;
    loader.load_own_sst_section(file.root())
}

#[test]
fn symbolic_lowering_interns_strings_and_resolves_scoped_forward_targets() {
    let mut loader = SymbolLoader::default();
    load_symbolic(
        &mut loader,
        r#"
fn @main() {
    label @entry
    ctl::host::cpu::const_string "hello\nworld"
    ctl::host::cpu::const_string "hello\nworld"
    ctl::host::cpu::const_fn_ref @helper
    ctl::host::cpu::br @done
    ctl::host::cpu::cond_br @done, @entry
    ctl::host::cpu::call 0, @helper
    label @done
    spare::cpu::halt
}
fn @helper() {
    label @entry
    spare::cpu::br @entry
    spare::cpu::ret 0
}
"#,
    )
    .unwrap();
    assert_eq!(loader.image.module.strings, ["hello\nworld"]);
    assert_eq!(
        loader.image.module.code,
        [
            __vihaco_MachineInstruction::V_controller__cpu__cpu__ConstString(cpu::ConstString(0)),
            __vihaco_MachineInstruction::V_controller__cpu__cpu__ConstString(cpu::ConstString(0)),
            __vihaco_MachineInstruction::V_controller__cpu__cpu__ConstFunctionRef(
                cpu::ConstFunctionRef(encode_function_ref(1))
            ),
            __vihaco_MachineInstruction::V_controller__cpu__cpu__Branch(cpu::Branch(6)),
            __vihaco_MachineInstruction::V_controller__cpu__cpu__ConditionalBranch(
                cpu::ConditionalBranch(6, 0)
            ),
            __vihaco_MachineInstruction::V_controller__cpu__cpu__Call(cpu::Call(0, 7)),
            __vihaco_MachineInstruction::V_spare__cpu__Halt(cpu::Halt),
            __vihaco_MachineInstruction::V_spare__cpu__Branch(cpu::Branch(7)),
            __vihaco_MachineInstruction::V_spare__cpu__Return(cpu::Return(0)),
        ]
    );
    let mut machine = machine();
    for instruction in &loader.image.module.code[..3] {
        machine.dispatch(instruction.clone());
    }
    assert_eq!(
        machine.controller.cpu.stack(),
        &[0, 0, encode_function_ref(1)]
    );
    machine.dispatch(loader.image.module.code[3].clone());
    assert_eq!(machine.controller.cpu.take_pending_pc(), Some(6));
    machine.dispatch(loader.image.module.code[5].clone());
    assert_eq!(machine.controller.cpu.take_pending_pc(), Some(7));
}

#[test]
fn failed_symbol_resolution_keeps_executable_code_and_rejects_duplicates() {
    let mut loader = SymbolLoader::default();
    load_symbolic(&mut loader, "fn @main() { spare::cpu::halt }").unwrap();
    let prior = loader.image.module.code.clone();
    for (instruction, message) in [
        ("br @missing", "unknown label missing"),
        ("cond_br 0, @missing", "unknown label missing"),
        ("call 0, @missing", "unknown function missing"),
        ("const_fn_ref @missing", "unknown function missing"),
    ] {
        let functions = format!("fn @main() {{ spare::cpu::{instruction} }}");
        assert!(
            load_symbolic(&mut loader, &functions)
                .unwrap_err()
                .to_string()
                .contains(message)
        );
        assert_eq!(loader.image.module.code, prior);
    }
    assert!(
        load_symbolic(
            &mut loader,
            "fn @main() { label @same spare::cpu::halt label @same }"
        )
        .unwrap_err()
        .to_string()
        .contains("duplicate label same")
    );
    assert!(
        load_symbolic(&mut loader, "fn @main() {} fn @main() {}")
            .unwrap_err()
            .to_string()
            .contains("duplicate function main")
    );
}

#[test]
fn numeric_operands_bypass_capability_calls_and_targets_reject_malformed_input() {
    for text in ["br -1", "br 4294967296", "br @", "br later", "br @ later"] {
        assert!(
            cpu::Branch::<vihaco::Syntax>::parser()
                .parse(text)
                .has_errors(),
            "{text}"
        );
    }
    let runtime = cpu::Branch::<vihaco::Syntax>::parser()
        .parse("br 10")
        .into_result()
        .unwrap();
    assert_eq!(
        vihaco::Lower::lower(runtime, &mut NumericContext).unwrap(),
        cpu::Branch(10)
    );
    let mut loader = SymbolLoader::default();
    loader.image.module.strings.push("existing".to_owned());
    let source = cpu::ConstString::<vihaco::Syntax>::parser()
        .parse("const_string \"existing\"")
        .into_result()
        .unwrap();
    assert_eq!(
        vihaco::Lower::lower(source, &mut loader).unwrap(),
        cpu::ConstString(0)
    );
    assert_eq!(loader.image.module.strings, ["existing"]);
}

#[test]
fn cpu_lowering_requires_loader_capabilities_at_compile_time() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/lowering/*.rs");
}
