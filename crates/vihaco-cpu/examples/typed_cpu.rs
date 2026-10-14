// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{Execute, Parse, chumsky::Parser as _, traits::StackMemory};
use vihaco_cpu::{CPU, StepOutcome, cpu, decode_i64};

fn main() -> eyre::Result<()> {
    let mut cpu = CPU::default();
    for source in ["const_i64 -42", "const_i64 7"] {
        let instruction = cpu::ConstI64::parser()
            .parse(source)
            .into_result()
            .map_err(|errors| eyre::eyre!("invalid instruction: {errors:?}"))?;
        cpu.execute(&instruction, ())?;
    }
    let add = cpu::AddI64::parser()
        .parse("add_i64")
        .into_result()
        .map_err(|errors| eyre::eyre!("invalid instruction: {errors:?}"))?;
    assert_eq!(cpu.execute(&add, ())?, StepOutcome::Continue);
    assert_eq!(decode_i64(*cpu.stack_top()?), -35);
    println!("{}", decode_i64(*cpu.stack_top()?));
    Ok(())
}
