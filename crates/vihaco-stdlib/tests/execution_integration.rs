// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "program-image-execution")]

use vihaco_abi::{traits::Reset, word::*};
use vihaco_cpu::CPU;
use vihaco_module::module::LocalModule;
use vihaco_module::module::{FunctionInfo, Signature};
use vihaco_runtime::{Apply, BuildMessage, Execute, NoMessage, Step, field};
use vihaco_runtime::{HasInstructionSet, Lower, Parse, chumsky::Parser as _, composite};
use vihaco_stdlib::ExecutionState;
use vihaco_stdlib::execution_state::{execute, resume};

#[composite]
struct Machine {
    #[device(1)]
    #[program]
    program: vihaco_stdlib::ExecutionState,
    #[device(2)]
    cpu: vihaco_cpu::CPU,
    #[device(3)]
    collector: Collector,
    fail_after_effects: bool,
}

type Instruction = <Machine as HasInstructionSet>::Runtime;

type Program = LocalModule<Instruction, vihaco_abi::Value, vihaco_abi::Type>;

type Source = <Machine as HasInstructionSet>::Syntax;

// These are authored policies, not generated or framework blanket Steps.
impl<I> Step<field!(Machine::program), I, Program> for Machine
where
    ExecutionState: Execute<I, Effect = (), Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::program), I, Program>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
        self.assert_borrowed_instruction(instruction, module);
        let message = <Self as BuildMessage<field!(Machine::program), I, Program>>::build_message(
            self,
            instruction,
            module,
        )?;
        self.program.execute(instruction, message)
    }
}

impl<I> Step<field!(Machine::cpu), I, Program> for Machine
where
    CPU: Execute<I, Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::cpu), I, Program>,
    ExecutionState: Apply<<CPU as Execute<I>>::Effect>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
        self.assert_borrowed_instruction(instruction, module);
        let message = <Self as BuildMessage<field!(Machine::cpu), I, Program>>::build_message(
            self,
            instruction,
            module,
        )?;
        let effect = self.cpu.execute(instruction, message)?;
        self.program.apply(effect);
        if self.fail_after_effects {
            // Even after Halt inside an authored Step, its invocation remains
            // protected against reentrant execution.
            self.program
                .execute(&vihaco_stdlib::program::program::Halt, NoMessage::new())?;
            assert!(resume(module, self).is_err());
            eyre::bail!("authored effect handling failed");
        }
        Ok(())
    }
}

impl<I> Step<field!(Machine::collector), I, Program> for Machine
where
    Collector: Execute<I, Effect = vihaco_runtime::PushToStack<()>, Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::collector), I, Program>,
{
    fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
        let message =
            <Self as BuildMessage<field!(Machine::collector), I, Program>>::build_message(
                self,
                instruction,
                module,
            )?;
        self.collector.execute(instruction, message)?;
        Ok(())
    }
}

impl Machine {
    fn assert_borrowed_instruction<I>(&self, instruction: &I, module: &Program) {
        let code = module.code.as_slice();
        let start = code.as_ptr() as usize;
        let address = instruction as *const I as usize;
        assert!((start..start + std::mem::size_of_val(code)).contains(&address));
    }
}

struct NumericContext;

impl vihaco_stdlib::program::lower::LabelLookup for NumericContext {
    fn label_address(&self, _: &str) -> eyre::Result<u32> {
        Err(eyre::eyre!("numeric fixture"))
    }
}

impl vihaco_stdlib::program::lower::FunctionLookup for NumericContext {
    fn function_address(&self, _: &str) -> eyre::Result<u32> {
        Err(eyre::eyre!("numeric fixture"))
    }
    fn function_index(&self, _: &str) -> eyre::Result<u32> {
        Err(eyre::eyre!("numeric fixture"))
    }
}

impl vihaco_stdlib::program::lower::StringIntern for NumericContext {
    fn intern_string(&mut self, _: &str) -> eyre::Result<u32> {
        Err(eyre::eyre!("numeric fixture"))
    }
}

fn machine(code: &[&str]) -> (Program, Machine) {
    let module = LocalModule {
        code: code
            .iter()
            .map(|source| {
                Source::parser()
                    .parse(source)
                    .into_result()
                    .unwrap()
                    .lower(&mut NumericContext)
                    .unwrap()
            })
            .collect::<Vec<Instruction>>(),
        main_function: Some(0),
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![],
                ret: vec![],
            },
            local_count: 1,
            start_address: 0,
            end_address: code.len() as u32,
            file: 0,
        }],
        ..Default::default()
    };
    let machine = Machine {
        program: ExecutionState::new(),
        cpu: CPU::default(),
        collector: Collector::default(),
        fail_after_effects: false,
    };
    (module, machine)
}

#[test]
fn generated_execution_routes_calls_locals_computation_and_root_results() {
    let (mut module, mut vm) = machine(&[
        "program::program::const_u64 20",
        "program::program::store_u64 0",
        "program::program::load_u64 0",
        "program::program::call 1, 7",
        "program::program::const_u64 2",
        "cpu::cpu::mul_u64",
        "program::program::ret 1",
        "program::program::load_u64 0",
        "program::program::const_u64 1",
        "cpu::cpu::add_u64",
        "program::program::ret 1",
    ]);
    module.functions.push(FunctionInfo {
        name: 0,
        signature: Signature {
            params: vec![vihaco_module::module::Parameter {
                name: 0,
                ty: vihaco_abi::Type::U64,
            }],
            ret: vec![],
        },
        local_count: 1,
        start_address: 7,
        end_address: 11,
        file: 0,
    });
    execute(&module, &mut vm, &[]).unwrap();
    assert_eq!(vm.program.stack(), &[42]);
    assert!(vm.program.frames().is_empty());
    assert_eq!(vm.program.pc(), 6);
    vm.program.reset();
    execute(&module, &mut vm, &[]).unwrap();
    assert_eq!(vm.program.stack(), &[42]);
}

#[test]
fn recursive_execution_preserves_continuations_and_multiple_results() {
    let (mut module, mut vm) = machine(&[
        "program::program::const_u64 3",
        "program::program::const_fn_ref 1",
        "program::program::call_indirect",
        "program::program::ret 2",
        "program::program::load_u64 0",
        "program::program::cond_br 9, 6",
        "program::program::const_u64 10",
        "program::program::const_u64 20",
        "program::program::ret 2",
        "program::program::load_u64 0",
        "program::program::const_u64 1",
        "cpu::cpu::sub_u64",
        "program::program::call 1, 4",
        "program::program::load_u64 0",
        "cpu::cpu::add_u64",
        "program::program::ret 2",
    ]);
    module.functions[0].local_count = 0;
    module.functions[0].end_address = 4;
    module.functions[0].signature.ret = vec![vihaco_abi::Type::U64; 2];
    module.functions.push(FunctionInfo {
        name: 0,
        signature: Signature {
            params: vec![vihaco_module::module::Parameter {
                name: 0,
                ty: vihaco_abi::Type::U64,
            }],
            ret: vec![vihaco_abi::Type::U64; 2],
        },
        local_count: 1,
        start_address: 4,
        end_address: 16,
        file: 0,
    });
    execute(&module, &mut vm, &[]).unwrap();
    assert_eq!(vm.program.stack(), &[10, 26]); // 20 + each restored caller's 1, 2, 3.
    assert!(vm.program.frames().is_empty());
    assert!(vm.program.is_halted());
    assert_eq!(vm.program.pc(), 3);
}

#[test]
fn execution_failures_preserve_consumption_and_faulting_pc() {
    let (module, mut vm) = machine(&[
        "program::program::const_u64 4",
        "program::program::const_u64 0",
        "cpu::cpu::div_u64",
        "program::program::halt",
    ]);
    assert!(execute(&module, &mut vm, &[]).is_err());
    assert_eq!(vm.program.stack(), &[0]);
    assert_eq!(vm.program.pc(), 2);
    let (module, mut vm) = machine(&["program::program::const_u64 4", "cpu::cpu::add_u64"]);
    assert!(execute(&module, &mut vm, &[]).is_err());
    assert_eq!(vm.program.stack(), &[0]); // Partial tuple extraction protects locals.
    assert_eq!(vm.program.pc(), 1);
    let (module, mut vm) = machine(&["program::program::const_u64 4"]);
    assert!(execute(&module, &mut vm, &[]).is_err()); // Checked fetch at fallthrough, rather than implicit Halt.
    assert_eq!(vm.program.pc(), 1);
    assert_eq!(vm.program.stack(), &[0, 4]);
    let (module, mut vm) = machine(&[
        "program::program::const_u64 4",
        "program::program::const_u64 2",
        "cpu::cpu::add_u64",
        "program::program::halt",
    ]);
    vm.fail_after_effects = true;
    assert!(
        execute(&module, &mut vm, &[])
            .unwrap_err()
            .to_string()
            .contains("authored effect handling")
    );
    assert_eq!(vm.program.stack(), &[0, 6]);
    assert_eq!(vm.program.pc(), 2);
    // Error exit released the invocation guard.
    vm.fail_after_effects = false;
    execute(&module, &mut vm, &[]).unwrap();
    for instruction in ["br 99", "load_u64 1", "call 1, 0", "ret 1"] {
        let (module, mut vm) = machine(&[&format!("program::program::{instruction}")]);
        assert!(execute(&module, &mut vm, &[]).is_err(), "{instruction}");
        assert_eq!(vm.program.pc(), 0, "{instruction}");
        assert_eq!(vm.program.stack(), &[0], "{instruction}");
    }
    let (module, mut vm) = machine(&[
        "program::program::const_fn_ref 999",
        "program::program::call_indirect",
    ]);
    assert!(execute(&module, &mut vm, &[]).is_err());
    assert_eq!(vm.program.pc(), 1);
    assert_eq!(vm.program.stack(), &[0, 999]); // Invalid reference remains unconsumed.
}

#[test]
fn heap_transfers_use_program_operands_and_preserve_device_state() {
    let (mut module, mut vm) = machine(&[
        "program::program::const_u64 7",
        "program::program::const_u64 9",
        "cpu::cpu::heap_alloc 2",
        "program::program::const_u64 1",
        "cpu::cpu::get_item",
        "program::program::ret 1",
    ]);
    execute(&module, &mut vm, &[]).unwrap();
    assert_eq!(vm.program.stack(), &[9]);
    let (readback, _) = machine(&[
        "program::program::const_heap_ref 0",
        "program::program::const_u64 0",
        "cpu::cpu::get_item",
        "program::program::ret 1",
    ]);
    module = readback;
    vm.program.reset();
    execute(&module, &mut vm, &[]).unwrap();
    assert_eq!(vm.program.stack(), &[7]); // Existing device heap survives program reset/restart.
    vm.cpu.reset();
    vm.program.reset();
    assert!(execute(&module, &mut vm, &[]).is_err()); // CPU reset clears it.
    let (module, mut raw) = machine(&[
        "program::program::const_u64 1",
        "cpu::cpu::heap_reserve",
        "program::program::store_u64 0",
        "program::program::const_u64 4294967296",
        "program::program::const_u64 7",
        "cpu::cpu::heap_push",
        "program::program::ret 1",
    ]);
    execute(&module, &mut raw, &[]).unwrap();
    assert_eq!(raw.program.stack(), &[1_u64 << 32]); // Decode the id without rewriting the reference.
    for operation in ["heap_alloc 2", "heap_push"] {
        let operation = format!("cpu::cpu::{operation}");
        let (module, mut vm) = machine(&["program::program::const_u64 7", &operation]);
        assert!(execute(&module, &mut vm, &[]).is_err());
        assert_eq!(vm.program.stack(), &[0]);
        assert_eq!(vm.program.pc(), 1);
    }
    let (module, mut vm) = machine(&[
        "program::program::const_u64 7",
        "program::program::const_u64 9",
        "cpu::cpu::heap_alloc 2",
        "program::program::dup",
        "cpu::cpu::heap_len",
        "program::program::store_u64 0",
        "cpu::cpu::heap_dealloc",
        "program::program::const_heap_ref 0",
        "program::program::const_u64 0",
        "cpu::cpu::get_item",
    ]);
    let error = execute(&module, &mut vm, &[]).unwrap_err();
    assert!(error.to_string().contains("deallocated"));
    assert_eq!(vm.program.stack(), &[2]);
    assert_eq!(vm.program.pc(), 9);
}

#[vihaco_runtime::dialect]
mod batches {
    #[instruction]
    #[pattern = "'code_length"]
    pub struct CodeLength;
    #[instruction]
    pub struct Gather(pub u32);
    #[instruction]
    #[pattern = "'gather_counted"]
    pub struct GatherCounted;
}

#[vihaco_runtime::component]
#[instructions { batches }]
#[derive(Default)]
struct Collector {
    code_length: usize,
    batch: Option<vihaco_runtime::StackBatch<u64>>,
}

impl vihaco_runtime::StackInputCount<batches::Gather> for Collector {
    fn input_count(instruction: &batches::Gather) -> eyre::Result<vihaco_runtime::InputCount> {
        Ok(vihaco_runtime::InputCount::Immediate(usize::try_from(
            instruction.0,
        )?))
    }
}

impl vihaco_runtime::StackInputCount<batches::GatherCounted> for Collector {
    fn input_count(_: &batches::GatherCounted) -> eyre::Result<vihaco_runtime::InputCount> {
        Ok(vihaco_runtime::InputCount::FromStack)
    }
}

impl vihaco_runtime::Execute<batches::Gather> for Collector {
    type Message = vihaco_runtime::PopFromStack<vihaco_runtime::StackBatch<u64>>;
    type Effect = vihaco_runtime::PushToStack<()>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        _: &batches::Gather,
        vihaco_runtime::PopFromStack(batch): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        self.batch = Some(batch);
        Ok(vihaco_runtime::PushToStack(()))
    }
}

impl vihaco_runtime::Execute<batches::GatherCounted> for Collector {
    type Message = vihaco_runtime::PopFromStack<vihaco_runtime::StackBatch<u64>>;
    type Effect = vihaco_runtime::PushToStack<()>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        _: &batches::GatherCounted,
        vihaco_runtime::PopFromStack(batch): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        self.batch = Some(batch);
        Ok(vihaco_runtime::PushToStack(()))
    }
}

#[test]
fn variable_batches_keep_order_ownership_empty_inputs_and_partial_consumption() {
    use vihaco_runtime::{Apply, PushToStack};
    for count in [0, 3, 20] {
        for from_stack in [false, true] {
            let instruction = if from_stack {
                "collector::batches::gather_counted".to_owned()
            } else {
                format!("collector::batches::gather {count}")
            };
            let (module, mut vm) = machine(&[&instruction, "program::program::halt"]);
            vm.program.start(&module, &[]).unwrap();
            let expected = (1..=count as u64).collect::<Vec<_>>();
            for value in &expected {
                vm.program.apply(PushToStack((*value,)));
            }
            if from_stack {
                vm.program.apply(PushToStack((count as u64,)));
            }
            resume(&module, &mut vm).unwrap();
            assert_eq!(vm.program.stack(), &[0]);
            vm.program.reset(); // The component retains owned values independently.
            let batch = vm.collector.batch.take().unwrap();
            assert_eq!(batch.as_slice(), expected);
        }
    }
    let (module, mut vm) = machine(&[
        "collector::batches::gather_counted",
        "program::program::halt",
    ]);
    vm.program.start(&module, &[]).unwrap();
    vm.program.apply(PushToStack((7_u64, 9_u64, 3_u64)));
    assert!(resume(&module, &mut vm).is_err());
    assert_eq!(vm.program.stack(), &[0]); // Locals protected; count and prior values consumed.
    assert_eq!(vm.program.pc(), 0);
    assert!(vm.collector.batch.is_none());
}

#[test]
fn all_scalar_operations_execute_through_the_generated_route() {
    for removed in ["const_u64 1", "load_u64 0", "halt", "span 0 0 0", "print"] {
        let source = format!("cpu::cpu::{removed}");
        assert!(Source::parser().parse(source.as_str()).has_errors());
    }
    let cases: &[(&str, &[u64], u64)] = &[
        ("add_u32", &[u64::MAX, 1], 0),
        ("shr_i32", &[encode_i32(-8), 34], encode_i32(-2)),
        ("neg_i32", &[u64::MAX], 1),
        ("eq_f64", &[f64::NAN.to_bits(), f64::NAN.to_bits()], 0),
        ("not", &[u64::MAX], 0),
        ("add_i32", &[6, 2][..], 8),
        ("add_f32", &[1086324736, 1073741824][..], 1090519040),
        ("add_i64", &[6, 2][..], 8),
        ("add_u32", &[6, 2][..], 8),
        ("add_u64", &[6, 2][..], 8),
        (
            "add_f64",
            &[4618441417868443648, 4611686018427387904][..],
            4620693217682128896,
        ),
        ("sub_i32", &[6, 2][..], 4),
        ("sub_i64", &[6, 2][..], 4),
        ("sub_u32", &[6, 2][..], 4),
        ("sub_u64", &[6, 2][..], 4),
        ("sub_f32", &[1086324736, 1073741824][..], 1082130432),
        (
            "sub_f64",
            &[4618441417868443648, 4611686018427387904][..],
            4616189618054758400,
        ),
        ("mul_i32", &[6, 2][..], 12),
        ("mul_i64", &[6, 2][..], 12),
        ("mul_u32", &[6, 2][..], 12),
        ("mul_u64", &[6, 2][..], 12),
        ("mul_f32", &[1086324736, 1073741824][..], 1094713344),
        (
            "mul_f64",
            &[4618441417868443648, 4611686018427387904][..],
            4622945017495814144,
        ),
        ("div_i32", &[6, 2][..], 3),
        ("div_i64", &[6, 2][..], 3),
        ("div_u32", &[6, 2][..], 3),
        ("div_u64", &[6, 2][..], 3),
        ("div_f32", &[1086324736, 1073741824][..], 1077936128),
        (
            "div_f64",
            &[4618441417868443648, 4611686018427387904][..],
            4613937818241073152,
        ),
        ("rem_i32", &[6, 2][..], 0),
        ("rem_i64", &[6, 2][..], 0),
        ("rem_u32", &[6, 2][..], 0),
        ("rem_u64", &[6, 2][..], 0),
        ("rem_f32", &[1086324736, 1073741824][..], 0),
        (
            "rem_f64",
            &[4618441417868443648, 4611686018427387904][..],
            0,
        ),
        ("shl_i32", &[6, 2][..], 24),
        ("shl_i64", &[6, 2][..], 24),
        ("shl_u32", &[6, 2][..], 24),
        ("shl_u64", &[6, 2][..], 24),
        ("shr_i32", &[6, 2][..], 1),
        ("shr_i64", &[6, 2][..], 1),
        ("shr_u32", &[6, 2][..], 1),
        ("shr_u64", &[6, 2][..], 1),
        ("rol_i32", &[6, 2][..], 24),
        ("rol_i64", &[6, 2][..], 24),
        ("rol_u32", &[6, 2][..], 24),
        ("rol_u64", &[6, 2][..], 24),
        ("ror_i32", &[6, 2][..], 2147483649),
        ("ror_i64", &[6, 2][..], 9223372036854775809),
        ("ror_u32", &[6, 2][..], 2147483649),
        ("ror_u64", &[6, 2][..], 9223372036854775809),
        ("bitand_i32", &[6, 2][..], 2),
        ("bitand_i64", &[6, 2][..], 2),
        ("bitand_u32", &[6, 2][..], 2),
        ("bitand_u64", &[6, 2][..], 2),
        ("bitor_i32", &[6, 2][..], 6),
        ("bitor_i64", &[6, 2][..], 6),
        ("bitor_u32", &[6, 2][..], 6),
        ("bitor_u64", &[6, 2][..], 6),
        ("bitxor_i32", &[6, 2][..], 4),
        ("bitxor_i64", &[6, 2][..], 4),
        ("bitxor_u32", &[6, 2][..], 4),
        ("bitxor_u64", &[6, 2][..], 4),
        ("neg_i32", &[6][..], encode_i32(-6)),
        ("neg_i64", &[6][..], 18446744073709551610),
        ("neg_f32", &[1086324736][..], 3233808384),
        ("neg_f64", &[4618441417868443648][..], 13841813454723219456),
        ("eq_i32", &[6, 2][..], 0),
        ("eq_i64", &[6, 2][..], 0),
        ("eq_u32", &[6, 2][..], 0),
        ("eq_u64", &[6, 2][..], 0),
        ("eq_f32", &[1086324736, 1073741824][..], 0),
        ("eq_f64", &[4618441417868443648, 4611686018427387904][..], 0),
        ("ne_i32", &[6, 2][..], 1),
        ("ne_i64", &[6, 2][..], 1),
        ("ne_u32", &[6, 2][..], 1),
        ("ne_u64", &[6, 2][..], 1),
        ("ne_f32", &[1086324736, 1073741824][..], 1),
        ("ne_f64", &[4618441417868443648, 4611686018427387904][..], 1),
        ("lt_i32", &[6, 2][..], 0),
        ("lt_i64", &[6, 2][..], 0),
        ("lt_u32", &[6, 2][..], 0),
        ("lt_u64", &[6, 2][..], 0),
        ("lt_f32", &[1086324736, 1073741824][..], 0),
        ("lt_f64", &[4618441417868443648, 4611686018427387904][..], 0),
        ("gt_i32", &[6, 2][..], 1),
        ("gt_i64", &[6, 2][..], 1),
        ("gt_u32", &[6, 2][..], 1),
        ("gt_u64", &[6, 2][..], 1),
        ("gt_f32", &[1086324736, 1073741824][..], 1),
        ("gt_f64", &[4618441417868443648, 4611686018427387904][..], 1),
        ("le_i32", &[6, 2][..], 0),
        ("le_i64", &[6, 2][..], 0),
        ("le_u32", &[6, 2][..], 0),
        ("le_u64", &[6, 2][..], 0),
        ("le_f32", &[1086324736, 1073741824][..], 0),
        ("le_f64", &[4618441417868443648, 4611686018427387904][..], 0),
        ("ge_i32", &[6, 2][..], 1),
        ("ge_i64", &[6, 2][..], 1),
        ("ge_u32", &[6, 2][..], 1),
        ("ge_u64", &[6, 2][..], 1),
        ("ge_f32", &[1086324736, 1073741824][..], 1),
        ("ge_f64", &[4618441417868443648, 4611686018427387904][..], 1),
        ("not", &[6][..], 0),
        ("and", &[6, 2][..], 1),
        ("or", &[6, 2][..], 1),
        ("xor", &[6, 2][..], 0),
    ];
    let check = |operation: &str, operands: &[u64], expected: u64| {
        let mut source = operands
            .iter()
            .map(|value| format!("program::program::const_u64 {value}"))
            .collect::<Vec<_>>();
        source.push(format!("cpu::cpu::{operation}"));
        source.push("program::program::ret 1".into());
        let (module, mut vm) = machine(&source.iter().map(String::as_str).collect::<Vec<_>>());
        execute(&module, &mut vm, &[]).unwrap();
        assert_eq!(vm.program.stack(), &[expected], "{operation}");
    };
    for &(operation, operands, expected) in cases {
        check(operation, operands, expected);
    }
    // Equal operands distinguish strict comparisons from inclusive comparisons.
    for (ty, value) in [
        ("i32", 6),
        ("i64", 6),
        ("u32", 6),
        ("u64", 6),
        ("f32", encode_f32(6.0)),
        ("f64", encode_f64(6.0)),
    ] {
        for (operation, expected) in [("lt", 0), ("gt", 0), ("le", 1), ("ge", 1)] {
            check(&format!("{operation}_{ty}"), &[value, value], expected);
        }
    }
}

struct CodeLength(usize);

impl BuildMessage<field!(Machine::collector), batches::CodeLength, Program, CodeLength>
    for Machine
{
    fn build_message(
        &mut self,
        _: &batches::CodeLength,
        module: &Program,
    ) -> eyre::Result<CodeLength> {
        Ok(CodeLength(module.code.len()))
    }
}

impl Execute<batches::CodeLength> for Collector {
    type Message = CodeLength;
    type Effect = vihaco_runtime::PushToStack<()>;
    type Error = eyre::Report;
    fn execute(
        &mut self,
        _: &batches::CodeLength,
        message: CodeLength,
    ) -> eyre::Result<Self::Effect> {
        self.code_length = message.0;
        Ok(vihaco_runtime::PushToStack(()))
    }
}

#[test]
fn custom_builder_reads_one_module_shared_by_independent_machines() {
    let (module, mut first) =
        machine(&["collector::batches::code_length", "program::program::ret 0"]);
    let (_, mut second) = machine(&["program::program::halt"]);
    execute(&module, &mut first, &[]).unwrap();
    execute(&module, &mut second, &[]).unwrap();
    assert_eq!(first.collector.code_length, 2);
    assert_eq!(second.collector.code_length, 2);
    assert!(first.program.frames().is_empty());
    assert!(second.program.frames().is_empty());
}
