// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{
    GeneratedComponent, expect_exactly_one_effect,
    traits::{FrameMemory, StackFrame, StackMemory},
};
use vihaco_cpu::{CPU, CPUMessage, RuntimeInstruction as I, StepOutcome, encode_function_ref};

fn execute(cpu: &mut CPU, instruction: I) -> eyre::Result<StepOutcome> {
    expect_exactly_one_effect(cpu.execute_generated(&instruction, CPUMessage::None)?)
}

fn call(cpu: &mut CPU, instruction: I, arity: u32, target: u32, locals: u32) {
    cpu.execute_generated(
        &instruction,
        CPUMessage::FunctionInfo {
            arity,
            start_address: target,
            local_count: locals,
        },
    )
    .unwrap();
}

#[test]
fn entry_and_nested_calls_preserve_arguments_locals_and_caller_operands() {
    let mut cpu = CPU::default();
    cpu.stack_mut().extend([10, 20]);
    cpu.enter_function(2, 0, 4, Some(0)).unwrap();
    assert_eq!(cpu.stack(), &[10, 20, 0, 0]);
    assert_eq!(cpu.operand_count(), 0);
    assert_eq!(cpu.get_frame().unwrap().operands_index(), 4);

    cpu.stack_push(99_u64); // caller operand must survive the nested invocation
    execute(&mut cpu, I::LoadU64(0)).unwrap();
    execute(&mut cpu, I::LoadU64(1)).unwrap();
    cpu.set_current_pc(7);
    call(&mut cpu, I::Call(2, 50), 2, 50, 5);
    assert_eq!(cpu.stack(), &[10, 20, 0, 0, 99, 10, 20, 0, 0, 0]);
    let frame = cpu.get_frame().unwrap();
    assert_eq!(
        (frame.base, frame.local_count, frame.operands_index()),
        (5, 5, 10)
    );
    assert_eq!(cpu.operand_count(), 0);
    assert_eq!(cpu.take_pending_pc(), Some(50));

    execute(&mut cpu, I::LoadU64(0)).unwrap();
    execute(&mut cpu, I::LoadU64(1)).unwrap();
    execute(&mut cpu, I::AddU64).unwrap();
    execute(&mut cpu, I::StoreU64(4)).unwrap();
    cpu.stack_push(123_u64); // unused callee operand
    execute(&mut cpu, I::LoadU64(4)).unwrap();
    assert_eq!(
        execute(&mut cpu, I::Return(1)).unwrap(),
        StepOutcome::Continue
    );
    assert_eq!(cpu.stack(), &[10, 20, 0, 0, 99, 30]);
    assert_eq!(cpu.take_pending_pc(), Some(8));
    assert_eq!(cpu.get_frame().unwrap().function, Some(0));

    assert_eq!(
        execute(&mut cpu, I::Return(2)).unwrap(),
        StepOutcome::Return
    );
    assert_eq!(cpu.return_values(), &[99, 30]);
    assert!(cpu.get_frame().is_err());
}

#[test]
fn indirect_call_gets_metadata_from_message_without_stack_metadata_words() {
    let mut cpu = CPU::default();
    cpu.enter_function(0, 0, 1, Some(0)).unwrap();
    cpu.stack_mut().extend([41, encode_function_ref(3)]);
    call(&mut cpu, I::IndirectCall, 1, 42, 3);
    assert_eq!(cpu.stack(), &[0, 41, 0, 0]);
    let frame = cpu.get_frame().unwrap();
    assert_eq!(frame.function, Some(3));
    assert_eq!(frame.base, 1);
    assert_eq!(frame.local_count, 3);
    assert_eq!(cpu.take_pending_pc(), Some(42));
    assert_eq!(cpu.operand_count(), 0);
}

#[test]
fn reserved_locals_cannot_be_used_as_operands() {
    let mut cpu = CPU::default();
    cpu.stack_push(12_u64);
    cpu.enter_function(1, 0, 3, Some(0)).unwrap();
    let original_frame = *cpu.get_frame().unwrap();
    for instruction in [
        I::Dup,
        I::AddU64,
        I::ConditionalBranch(1, 2),
        I::HeapAlloc(1),
        I::Return(1),
        I::StoreU64(0),
    ] {
        assert!(execute(&mut cpu, instruction).is_err());
        assert_eq!(cpu.stack(), &[12, 0, 0]);
        assert_eq!(*cpu.get_frame().unwrap(), original_frame);
    }
    assert!(cpu.stack_pop().is_err());
    assert!(cpu.stack_top().is_err());
    assert!(cpu.stack_top_mut().is_err());
    assert!(cpu.op_call(1, 10, 1).is_err());
    assert_eq!(cpu.stack(), &[12, 0, 0]);
}

#[test]
fn load_store_cannot_address_operands_or_grow_locals() {
    let mut cpu = CPU::default();
    cpu.enter_function(0, 0, 2, Some(0)).unwrap();
    cpu.stack_push(99_u64);
    for index in [2, 10, u32::MAX] {
        assert!(execute(&mut cpu, I::LoadU64(index)).is_err());
        assert!(execute(&mut cpu, I::StoreU64(index)).is_err());
        assert!(cpu.get_local_mut(index as usize).is_err());
        assert_eq!(cpu.stack(), &[0, 0, 99]);
    }
    execute(&mut cpu, I::StoreU64(1)).unwrap();
    assert_eq!(cpu.stack(), &[0, 99]);
}

#[test]
fn subsequent_invocations_zero_reused_local_slots() {
    let mut cpu = CPU::default();
    cpu.enter_function(0, 0, 0, Some(0)).unwrap();
    for _ in 0..2 {
        cpu.stack_push(7_u64);
        call(&mut cpu, I::Call(1, 10), 1, 10, 3);
        assert_eq!(cpu.stack(), &[7, 0, 0]);
        cpu.stack_push(42_u64);
        execute(&mut cpu, I::StoreU64(2)).unwrap();
        execute(&mut cpu, I::Return(0)).unwrap();
        assert!(cpu.stack().is_empty());
    }
}

#[test]
fn call_requires_metadata_and_does_not_validate_encoded_arity_against_message() {
    let mut cpu = CPU::default();
    cpu.stack_push(42_u64);
    assert!(execute(&mut cpu, I::Call(1, 5)).is_err());
    assert!(execute(&mut cpu, I::IndirectCall).is_err());
    assert_eq!(cpu.stack(), &[42]);
    call(&mut cpu, I::Call(1, 5), 99, 100, 2);
    assert_eq!(cpu.stack(), &[42, 0]);
    assert_eq!(cpu.take_pending_pc(), Some(5));
}
