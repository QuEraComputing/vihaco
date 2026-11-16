// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::instruction_out_of_bounds;
use super::{ExecutionState, RunState};
use std::ops::ControlFlow;
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
pub fn execute<M, I, V, Ty, Info, S>(
    module: &LocalModule<I, V, Ty, Info>,
    machine: &mut M,
    arguments: &[Word],
) -> eyre::Result<()>
where
    M: HasProgram<Program = ExecutionState<S>>
        + Dispatch<LocalModule<I, V, Ty, Info>, Instruction = I>,
{
    machine.program_mut().start(module, arguments)?;
    resume(module, machine)
}

/// Continue a runnable invocation until halt, root return, suspension, or error.
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
/// A halted or suspended invocation returns successfully without executing any
/// instructions. For a debugger breakpoint, use
/// [`crate::debugger::resume_with_breaks`] to resume the suspended invocation.
///
/// # Errors
/// Rejects reentrant execution, invalid running state, invalid fetches, PC
/// overflow, or a failure returned by the authored Dispatch/Step.
pub fn resume<M, I, V, Ty, Info, S>(
    module: &LocalModule<I, V, Ty, Info>,
    machine: &mut M,
) -> eyre::Result<()>
where
    M: HasProgram<Program = ExecutionState<S>>
        + Dispatch<LocalModule<I, V, Ty, Info>, Instruction = I>,
{
    run(
        machine,
        |_| Ok(()),
        |machine, ()| {
            execute_instruction(module, machine, None)?;
            Ok(ControlFlow::Continue(()))
        },
    )
}

pub(crate) fn run<M, S, State>(
    machine: &mut M,
    initialize: impl FnOnce(&mut M) -> eyre::Result<State>,
    mut step: impl FnMut(&mut M, &mut State) -> eyre::Result<ControlFlow<()>>,
) -> eyre::Result<()>
where
    M: HasProgram<Program = ExecutionState<S>>,
{
    let program = machine.program_mut();
    eyre::ensure!(
        !program.executing,
        "program execution is already in progress"
    );
    program.executing = true;
    let result = (|| {
        let mut state = initialize(machine)?;
        while matches!(machine.program().run_state, RunState::Runnable) {
            if step(machine, &mut state)?.is_break() {
                break;
            }
        }
        Ok(())
    })();
    machine.program_mut().executing = false;
    result
}

pub(crate) fn execute_instruction<M, I, V, Ty, Info, S>(
    module: &LocalModule<I, V, Ty, Info>,
    machine: &mut M,
    instruction_override: Option<&I>,
) -> eyre::Result<()>
where
    M: HasProgram<Program = ExecutionState<S>>
        + Dispatch<LocalModule<I, V, Ty, Info>, Instruction = I>,
{
    let program = machine.program_mut();
    program.ensure_running()?;
    let position = program.pc;
    let fetched = module
        .code
        .get(position as usize)
        .ok_or_else(|| instruction_out_of_bounds(position, module.code.len()))?;
    let instruction = instruction_override.unwrap_or(fetched);
    program.pc = position
        .checked_add(1)
        .ok_or_else(|| eyre::eyre!("program counter overflow"))?;
    let result = machine.dispatch(instruction, module);
    let program = machine.program_mut();
    if result.is_err() || !matches!(program.run_state, RunState::Runnable) {
        program.pc = position;
    }
    result
}
