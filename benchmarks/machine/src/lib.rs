// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! This checkout's example machine and its benchmark adapter.

mod machine;
mod resolve;

use eyre::{Result, ensure};
use std::{fs, path::Path};
use vihaco::{GeneratedComponent, expect_exactly_one_effect, traits::StackMemory};
use vihaco_benchmark_api::{
    BenchmarkMachine, COMPOSITE, CPU_ENUM, CPU_OPERATION, ConstantBenchmark,
};
use vihaco_cpu::{CPUMessage, RuntimeInstruction, StepOutcome};

use machine::fixture;
pub use machine::{Fixture, Program};

/// The only concrete entry point imported by the shared harness.
pub struct Machine;

impl BenchmarkMachine for Machine {
    type Program = Program;
    type Invocation = Fixture;

    fn load(workload: &str) -> Result<Program> {
        ensure!(
            !workload.is_empty()
                && workload.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'-'
                    || b == b'_'),
            "invalid workload ID"
        );
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("programs")
            .join(format!("{workload}.sst"));
        Program::parse(&fs::read_to_string(path)?)
    }

    fn prepare(program: &Program, iterations: u64, seed: u64) -> Result<Fixture> {
        Ok(Fixture::for_program(program, iterations, seed))
    }

    fn execute<const COMPOSITE: bool>(invocation: &mut Fixture, program: &Program) -> Result<u64> {
        invocation.run::<COMPOSITE>(program)
    }
}

fn constant_result(state: &Fixture, outcome: StepOutcome) -> Result<u64> {
    ensure!(
        outcome == StepOutcome::Continue,
        "constant instruction did not continue"
    );
    Ok(*state.cpu.stack_top()?)
}

impl ConstantBenchmark<CPU_OPERATION> for Machine {
    type State = Fixture;
    type Instruction = u64;

    fn prepare() -> Result<Fixture> {
        Ok(Fixture::prepared(0, 0))
    }
    fn instruction(value: u64) -> u64 {
        value
    }
    fn execute(state: &mut Fixture, instruction: &u64) -> Result<u64> {
        let outcome = state.cpu.op_const(*instruction)?;
        constant_result(state, outcome)
    }
}

impl ConstantBenchmark<CPU_ENUM> for Machine {
    type State = Fixture;
    type Instruction = RuntimeInstruction;

    fn prepare() -> Result<Fixture> {
        Ok(Fixture::prepared(0, 0))
    }
    fn instruction(value: u64) -> RuntimeInstruction {
        RuntimeInstruction::ConstU64(value)
    }
    fn execute(state: &mut Fixture, instruction: &RuntimeInstruction) -> Result<u64> {
        let outcome =
            expect_exactly_one_effect(state.cpu.execute_generated(instruction, CPUMessage::None)?)?;
        constant_result(state, outcome)
    }
}

impl ConstantBenchmark<COMPOSITE> for Machine {
    type State = Fixture;
    type Instruction = fixture::runtime::Instruction;

    fn prepare() -> Result<Fixture> {
        Ok(Fixture::prepared(0, 0))
    }
    fn instruction(value: u64) -> Self::Instruction {
        fixture::runtime::Instruction::Cpu(RuntimeInstruction::ConstU64(value))
    }
    fn execute(state: &mut Fixture, instruction: &Self::Instruction) -> Result<u64> {
        let outcome = state.dispatch(instruction)?;
        constant_result(state, outcome)
    }
}
