// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::ExecutionState;
use super::instruction_out_of_bounds;
use vihaco_abi::word::Word;
use vihaco_module::module::LocalModule;
use vihaco_runtime::{Dispatch, HasProgram};

/// Enter main and execute through the composite's authored Steps.
///
/// Devices remain in place. Instructions borrow the caller-owned module; there
/// is no instruction clone, generated Step, or imposed effect-application policy.
///
/// # Errors
/// Returns entry, checked-fetch, or authored Dispatch/Step errors. Operand and
/// component mutations made before an error are retained.
pub fn execute<M, I, V, Ty, Info>(
    module: &LocalModule<I, V, Ty, Info>,
    machine: &mut M,
    arguments: &[Word],
) -> eyre::Result<()>
where
    M: HasProgram<Program = ExecutionState>
        + Dispatch<LocalModule<I, V, Ty, Info>, Instruction = I>,
{
    machine.program_mut().start(module, arguments)?;
    resume(module, machine)
}

/// Continue an invocation until Halt/root return or an error.
///
/// Fetch establishes the sequential continuation before the Step runs. Program
/// instructions can replace that continuation. A fault or terminal instruction
/// leaves its original position available through [`ExecutionState::pc()`].
///
/// The caller must supply the same unchanged module used to start the active
/// invocation. This checked API validates accesses, but does not enforce module
/// identity. After an error, earlier operand consumption and effects remain;
/// the caller is responsible for deciding whether the invocation can resume.
///
/// # Errors
/// Rejects reentrant execution, invalid running state, invalid fetches, PC
/// overflow, or a failure returned by the authored Dispatch/Step.
pub fn resume<M, I, V, Ty, Info>(
    module: &LocalModule<I, V, Ty, Info>,
    machine: &mut M,
) -> eyre::Result<()>
where
    M: HasProgram<Program = ExecutionState>
        + Dispatch<LocalModule<I, V, Ty, Info>, Instruction = I>,
{
    let program = machine.program_mut();
    eyre::ensure!(
        !program.executing,
        "program execution is already in progress"
    );
    program.executing = true;
    // Capture code once from the independent module borrow.
    let code = &module.code;
    let result = (|| {
        while !machine.program().halted {
            let program = machine.program_mut();
            program.ensure_running()?;
            let position = program.pc;
            let instruction = code
                .get(position as usize)
                .ok_or_else(|| instruction_out_of_bounds(position, code.len()))?;
            program.pc = position
                .checked_add(1)
                .ok_or_else(|| eyre::eyre!("program counter overflow"))?;
            // `instruction` borrows the external module, never machine.
            let result = machine.dispatch(instruction, module);
            let program = machine.program_mut();
            if result.is_err() || program.halted {
                program.pc = position;
            }
            result?;
        }
        Ok(())
    })();
    machine.program_mut().executing = false;
    result
}
