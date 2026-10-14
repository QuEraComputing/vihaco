// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[test]
fn typed_cpu_parses_and_executes_with_this_dependency_graph() {
    use rt::{Execute, Parse, chumsky::Parser as _};
    let instruction = host::cpu::Branch::parser().parse("br 10").into_result().unwrap();
    assert_eq!(instruction, host::cpu::Branch(10));
    let mut cpu = host::CPU::default();
    assert_eq!(cpu.execute(&instruction, ()).unwrap(), host::StepOutcome::Continue);
    assert_eq!(cpu.take_pending_pc(), Some(10));
}

#[test]
fn dependency_activates_cpu_local_feature() { let _ = activation::cpu(); }
