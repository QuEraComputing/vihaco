// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[test]
fn typed_cpu_parses_and_executes_with_this_dependency_graph() {
    use rt::{Execute, Lower, Parse, Syntax, chumsky::Parser as _};
    let instruction = host::cpu::Branch::<Syntax>::parser().parse("br 10").into_result().unwrap().lower(&mut NumericLabels).unwrap();
    assert_eq!(instruction, host::cpu::Branch(10));
    let mut cpu = host::CPU::default();
    assert_eq!(cpu.execute(&instruction, ()).unwrap(), host::StepOutcome::Continue);
    assert_eq!(cpu.take_pending_pc(), Some(10));
}

#[cfg(test)]
struct NumericLabels;
#[cfg(test)]
impl host::lower::LabelLookup for NumericLabels {
    fn label_address(&self, name: &str) -> rt::eyre::Result<u32> {
        Err(rt::eyre::eyre!("unknown label {name}"))
    }
}
