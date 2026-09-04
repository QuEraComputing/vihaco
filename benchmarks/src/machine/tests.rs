// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::*;

fn sst(functions: &str) -> String {
    format!(
        "sst v1\n.section(root):\n.header(root):\n.header(root).\n.text(root):\n{functions}\n.text(root).\n.section(root).\n"
    )
}

fn main_sst(body: &str) -> String {
    sst(&format!(
        "fn @main(iterations: u64, seed: u64) -> u64 {{ {body} }}"
    ))
}

fn assert_runs(source: &str, expected: u64) -> Program {
    let program = Program::parse(source).unwrap();
    assert!(program.image.context().is_ok());
    assert_eq!(
        Fixture::for_program(&program, 3, 21)
            .run::<true>(&program)
            .unwrap(),
        expected
    );
    assert_eq!(
        Fixture::for_program(&program, 3, 21)
            .run::<false>(&program)
            .unwrap(),
        expected
    );
    program
}

#[test]
fn loading_rejects_oversized_sources_and_allocations() {
    assert!(Program::parse(&" ".repeat(1_048_577)).is_err());
    for body in [
        "cpu::cpu.load_u64 4096 cpu::cpu.ret 1",
        "cpu::cpu.store_u64 4294967295 cpu::cpu.ret 1",
        "cpu::cpu.heap_alloc 65537 cpu::cpu.ret 1",
    ] {
        assert!(Program::parse(&main_sst(body)).is_err(), "{body}");
    }
    assert!(Program::parse(&main_sst("cpu::cpu.load_u64 4095 cpu::cpu.ret 1")).is_ok());
    assert!(Program::parse(&main_sst("cpu::cpu.heap_alloc 65536 cpu::cpu.ret 1")).is_ok());
}

#[test]
fn isolated_const_routes_agree() {
    for value in [0_u64, 42, 65535] {
        for route in 0..3 {
            let mut machine = Fixture::prepared(0, 0);
            let outcome = match route {
                0 => machine.cpu.op_const(value).unwrap(),
                1 => expect_exactly_one_effect(
                    machine
                        .cpu
                        .execute_generated(&Inst::ConstU64(value), CPUMessage::None)
                        .unwrap(),
                )
                .unwrap(),
                _ => machine
                    .dispatch(&fixture::runtime::Instruction::Cpu(Inst::ConstU64(value)))
                    .unwrap(),
            };
            assert_eq!(outcome, StepOutcome::Continue);
            assert_eq!(*machine.cpu.stack_top().unwrap(), value);
            assert_eq!(machine.cpu.stack_len(), 5);
        }
    }
}

#[test]
fn malformed_programs_fail_explicitly() {
    for body in [
        "cpu::cpu.br @missing",
        "cpu::cpu.label @x\ncpu::cpu.label @x",
        "cpu::cpu.call 1, missing",
        "cpu::cpu.const_u64 -1",
        "",
    ] {
        assert!(Program::parse(&main_sst(body)).is_err(), "{body}");
    }
    assert!(Program::parse("cpu::cpu.halt").is_err());
    assert!(Program::parse(&sst("fn @helper() { cpu::cpu.halt }")).is_err());
    assert!(Program::parse(&sst("fn @main() { cpu::cpu.halt }")).is_err());
    assert!(Program::parse(&main_sst("cpu::cpu.halt").replace(".section(root).", "")).is_err());
    let program = Program::parse(&main_sst("cpu::cpu.halt")).unwrap();
    assert!(
        Fixture::for_program(&program, 0, 0)
            .run::<true>(&program)
            .is_err()
    );
}

#[test]
fn normal_sst_syntax_handles_comments_and_multiple_instructions_per_line() {
    let program = assert_runs(
        &main_sst(
            r#"
        // Semicolons inside quoted values must survive parsing.
        cpu::cpu.const_string "a;b" cpu::cpu.print
        cpu::cpu.const_u64 21 cpu::cpu.const_u64 2 cpu::cpu.mul_u64
        cpu::cpu.ret 1
    "#,
        ),
        42,
    );
    assert!(
        program
            .image
            .module
            .strings
            .iter()
            .any(|value| value == "a;b")
    );
}

#[test]
fn entry_point_and_function_local_labels_use_resolved_metadata() {
    let program = assert_runs(
        &sst(r#"
        fn @double(value: u64) -> u64 {
            cpu::cpu.label @entry
            cpu::cpu.load_u64 0 cpu::cpu.const_u64 2 cpu::cpu.mul_u64 cpu::cpu.ret 1
        }
        fn @main(iterations: u64, seed: u64) -> u64 {
            cpu::cpu.br @entry
            cpu::cpu.const_u64 99
            cpu::cpu.label @entry
            cpu::cpu.load_u64 1 cpu::cpu.call 1, double cpu::cpu.ret 1
        }
    "#),
        42,
    );
    assert_eq!(program.image.module.main_function, Some(1));
    assert!(program.image.pc() > 0);
    assert_eq!(program.image.module.labels.len(), 2);
    assert_eq!(program.image.module.functions[1].signature.params.len(), 2);
}

#[test]
fn forward_function_reference_resolves_to_an_index_not_an_address() {
    let program = assert_runs(
        &sst(r#"
        fn @main(iterations: u64, seed: u64) -> u64 {
            cpu::cpu.load_u64 1 cpu::cpu.const_fn_ref @double cpu::cpu.call_indirect cpu::cpu.ret 1
        }
        fn @double(value: u64) -> u64 {
            cpu::cpu.load_u64 0 cpu::cpu.const_u64 2 cpu::cpu.mul_u64 cpu::cpu.ret 1
        }
    "#),
        42,
    );
    assert!(program.image.module.functions[1].start_address > 1);
}

#[test]
fn resolution_rejects_ambiguous_functions_and_cross_function_branches() {
    let main = "fn @main(iterations: u64, seed: u64) -> u64 { cpu::cpu.halt }";
    assert!(Program::parse(&sst(&format!("{main}\n{main}"))).is_err());
    let source = sst(r#"
        fn @helper() { cpu::cpu.label @private cpu::cpu.ret 0 }
        fn @main(iterations: u64, seed: u64) -> u64 { cpu::cpu.br @private }
    "#);
    assert!(Program::parse(&source).is_err());
    let source = sst(r#"
        fn @helper(value: u64) -> u64 { cpu::cpu.load_u64 0 cpu::cpu.ret 1 }
        fn @main(iterations: u64, seed: u64) -> u64 { cpu::cpu.call 0, helper }
    "#);
    assert!(
        Program::parse(&source)
            .err()
            .unwrap()
            .to_string()
            .contains("call arity mismatch")
    );
}
