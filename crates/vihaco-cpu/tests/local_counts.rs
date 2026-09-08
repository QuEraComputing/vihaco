// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{
    DeviceInstruction, GeneratedComponent,
    module::{FunctionInfo, Parameter, Signature},
    traits::{StackFrame, StackMemory},
};
use vihaco_cpu::{
    CPU, CPUMessage, RuntimeInstruction, SurfaceInstruction as I, required_local_count,
};

vihaco::component! {
    pub component Pulse {}
    instruction {
        Fire,
    }
}

type SecondCpu = CPU;
use pulse::Pulse;

#[vihaco::composite]
#[allow(dead_code)]
struct Machine {
    #[device(1)]
    first: CPU,
    #[device(2)]
    second: SecondCpu,
    #[device(3)]
    pulse: Pulse,
}

impl<'a> From<&'a machine::syntax::Instruction> for Option<DeviceInstruction<'a, I, 1>> {
    fn from(instruction: &'a machine::syntax::Instruction) -> Self {
        match instruction {
            machine::syntax::Instruction::First(instruction) => {
                Some(DeviceInstruction { instruction })
            }
            _ => None,
        }
    }
}

impl<'a> From<&'a machine::syntax::Instruction> for Option<DeviceInstruction<'a, I, 2>> {
    fn from(instruction: &'a machine::syntax::Instruction) -> Self {
        match instruction {
            machine::syntax::Instruction::Second(instruction) => {
                Some(DeviceInstruction { instruction })
            }
            _ => None,
        }
    }
}

#[test]
fn composite_routes_resolved_counts_to_only_the_executing_cpu() {
    use chumsky::Parser as _;
    use vihaco::{Parse, syntax::ParsedFunction};
    use vihaco_cpu::SurfaceType;

    let parsed = ParsedFunction::<machine::syntax::Instruction, SurfaceType>::parser()
        .parse("fn @f(x: u64) { first::cpu.load_u64 3 second::cpu.store_u64 7 }")
        .into_result()
        .unwrap();
    let arity = u32::try_from(parsed.params.len()).unwrap();
    let function = FunctionInfo {
        name: 0,
        signature: Signature {
            params: vec![Parameter { name: 1, ty: () }],
            ret: vec![],
        },
        local_counts_by_device: [
            (
                1,
                required_local_count::<1, _>(arity, &parsed.body).unwrap(),
            ),
            (
                2,
                required_local_count::<2, _>(arity, &parsed.body).unwrap(),
            ),
        ]
        .into_iter()
        .collect(),
        start_address: 10,
        end_address: 12,
        file: 0,
    };
    let mut machine = Machine {
        first: CPU::default(),
        second: CPU::default(),
        pulse: Pulse {},
    };
    machine.first.stack_push(42_u64);
    machine
        .first
        .execute_generated(
            &RuntimeInstruction::Call(arity, function.start_address),
            CPUMessage::FunctionInfo {
                arity,
                start_address: function.start_address,
                local_count: function.local_count_for(1).unwrap(),
            },
        )
        .unwrap();
    assert_eq!(machine.first.stack(), &[42, 0, 0, 0]);
    assert!(machine.second.get_frame().is_err());
    assert!(machine.second.stack().is_empty());

    machine.second.stack_push(17_u64);
    machine
        .second
        .execute_generated(
            &RuntimeInstruction::Call(arity, function.start_address),
            CPUMessage::FunctionInfo {
                arity,
                start_address: function.start_address,
                local_count: function.local_count_for(2).unwrap(),
            },
        )
        .unwrap();
    assert_eq!(machine.second.stack(), &[17, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(machine.first.stack(), &[42, 0, 0, 0]);
}

#[test]
fn composite_analysis_preserves_device_identity_and_ignores_other_instruction_sets() {
    use machine::syntax::Instruction as M;
    let body = [
        M::First(I::LoadU64(0)),
        M::Second(I::StoreU64(7)),
        M::Pulse(pulse::syntax::Instruction::Fire),
        M::First(I::Halt),
        M::First(I::LoadU64(4)), // unreachable references still count
    ];
    let first = required_local_count::<1, _>(2, &body).unwrap();
    let second = required_local_count::<2, _>(2, &body).unwrap();
    assert_eq!((first, second), (5, 8));
    assert!(matches!(body[0], M::First(I::LoadU64(0)))); // input remains available
}

#[test]
fn all_typed_loads_and_stores_contribute_and_counts_include_parameters() {
    let instructions = [
        I::LoadI32(6),
        I::LoadI64(6),
        I::LoadU32(6),
        I::LoadU64(6),
        I::LoadF32(6),
        I::LoadF64(6),
        I::LoadBool(6),
        I::StoreI32(6),
        I::StoreI64(6),
        I::StoreU32(6),
        I::StoreU64(6),
        I::StoreF32(6),
        I::StoreF64(6),
        I::StoreBool(6),
    ];
    for instruction in &instructions {
        let count = required_local_count::<1, _>(2, [DeviceInstruction { instruction }]).unwrap();
        assert_eq!(count, 7);
        let count = required_local_count::<1, _>(10, [DeviceInstruction { instruction }]).unwrap();
        assert_eq!(count, 10);
    }
    let count = required_local_count::<1, _>(
        0,
        [DeviceInstruction {
            instruction: &I::LoadU64(0),
        }],
    )
    .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn absent_references_require_exactly_arity_and_overflow_is_an_error() {
    use machine::syntax::Instruction as M;
    let count = required_local_count::<1, _>(2, [M::First(I::Halt)].iter()).unwrap();
    assert_eq!(count, 2);
    let empty = required_local_count::<1, _>(2, std::iter::empty::<&M>()).unwrap();
    assert_eq!(empty, 2);
    let function = FunctionInfo {
        name: 0,
        signature: Signature {
            params: vec![Parameter { name: 1, ty: () }, Parameter { name: 2, ty: () }],
            ret: vec![],
        },
        local_counts_by_device: [(1, count)].into(),
        start_address: 0,
        end_address: 1,
        file: 0,
    };
    assert_eq!(function.local_count_for(1).unwrap(), 2);
    assert_eq!(function.local_count_for(2).unwrap(), 2);
    assert!(required_local_count::<1, _>(0, [M::First(I::StoreU64(u32::MAX))].iter()).is_err());
}
