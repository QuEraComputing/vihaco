// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[test]
fn legacy_cpu_uses_its_feature_off_declarations() {
    use vm::GeneratedComponent;
    let mut cpu = host::CPU::default();
    assert_eq!(cpu.execute_generated(&host::RuntimeInstruction::Halt, host::CPUMessage::None).unwrap(),
        vm::Effects::one(host::StepOutcome::Halt));
}
