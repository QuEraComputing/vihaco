// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Minimal CPU composite. The current composite macro generates the ISA;
//! execution routing is explicit, as it is in consumers of the framework.

use eyre::{Result, ensure, eyre};
use vihaco::{
    GeneratedComponent, expect_exactly_one_effect,
    frame::Frame,
    traits::{GetProgramInfo, ProgramCounter, StackFrame, StackMemory},
};
use vihaco_cpu::{
    CPU, CPUMessage, RuntimeInstruction as Inst, StepOutcome, decode_function_ref, decode_string_id,
};

mod program;
pub use program::Program;

#[cfg(test)]
mod tests;

/// One CPU and its program counter; preparation is separate from execution.
#[vihaco::composite]
pub struct Fixture {
    #[device(0x01)]
    pub cpu: CPU,
    pc: u32,
}

impl Fixture {
    /// Prepare an invocation of the module's declared entry point before timing.
    pub fn for_program(program: &Program, iterations: u64, seed: u64) -> Self {
        let mut machine = Self::prepared(iterations, seed);
        let main = &program.image.module.functions[program.main];
        machine.pc = program.image.pc();
        machine.cpu.stack_mut().resize(main.local_count as usize, 0);
        machine
            .cpu
            .get_frame_mut()
            .expect("prepared frame")
            .function = Some(program.main);
        machine
    }

    /// Allocate the frame and working stack before a timed invocation.
    pub fn prepared(iterations: u64, seed: u64) -> Self {
        let mut cpu = CPU::default();
        // Reserve working storage before timing; program execution may still
        // legitimately allocate if the workload itself requires it.
        cpu.stack_mut().reserve(16);
        cpu.push_frame(Frame {
            base: 0,
            span: (0, 0, 0),
            function: None,
            ret_pc: 0,
        });
        for value in [iterations, seed, 0, 0] {
            cpu.stack_push(value);
        }
        Self { cpu, pc: 0 }
    }

    /// Route one instruction for the isolated composite-dispatch measurement.
    pub fn dispatch(&mut self, inst: &fixture::runtime::Instruction) -> Result<StepOutcome> {
        match inst {
            fixture::runtime::Instruction::Cpu(inst) => {
                expect_exactly_one_effect(self.cpu.execute_generated(inst, CPUMessage::None)?)
            }
        }
    }

    fn message(&self, program: &Program, instruction: &Inst) -> Result<CPUMessage> {
        match instruction {
            Inst::IndirectCall => {
                let function = program
                    .image
                    .get_function(decode_function_ref(*self.cpu.stack_top()?) as usize)?;
                Ok(CPUMessage::FunctionInfo {
                    arity: u32::try_from(function.signature.params.len())?,
                    start_address: function.start_address,
                })
            }
            Inst::Print => Ok(CPUMessage::Print(
                program
                    .image
                    .get_string(decode_string_id(*self.cpu.stack_top()?) as usize)?
                    .clone(),
            )),
            _ => Ok(CPUMessage::None),
        }
    }

    /// Execute through either the CPU enum or the generated composite enum.
    /// The const parameter keeps route selection outside the timed step loop.
    pub fn run<const COMPOSITE: bool>(&mut self, program: &Program) -> Result<u64> {
        loop {
            self.cpu.set_current_pc(self.pc);
            let outcome = if COMPOSITE {
                let fixture::runtime::Instruction::Cpu(inst) =
                    program.image.get_instruction(self.pc)?;
                let message = self.message(program, inst)?;
                expect_exactly_one_effect(self.cpu.execute_generated(inst, message)?)?
            } else {
                let inst = program
                    .direct
                    .get(self.pc as usize)
                    .ok_or_else(|| eyre!("PC outside program"))?;
                let message = self.message(program, inst)?;
                expect_exactly_one_effect(self.cpu.execute_generated(inst, message)?)?
            };
            match outcome {
                StepOutcome::Halt => {
                    ensure!(
                        self.cpu.get_frame()?.function == Some(program.main)
                            && self.cpu.stack_len()
                                == program.image.module.functions[program.main].local_count
                                    as usize
                                    + 1,
                        "program must leave exactly one result above its locals"
                    );
                    return Ok(*self.cpu.stack_top()?);
                }
                StepOutcome::Return => {
                    let [result] = self.cpu.return_values() else {
                        return Err(eyre!("program must return exactly one result"));
                    };
                    return Ok(*result);
                }
                StepOutcome::Continue => {
                    self.pc = self.cpu.take_pending_pc().unwrap_or(self.pc + 1)
                }
                other => return Err(eyre!("unexpected completion: {other:?}")),
            }
        }
    }
}
