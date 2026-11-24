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

/// Drive an invocation with caller-defined initialization and iteration policy.
///
/// After rejecting reentrant execution, calls `initialize` once, including for
/// halted or suspended invocations. The returned state is passed to `step` on
/// each iteration while the program is [`RunState::Runnable`]. Returning
/// [`ControlFlow::Break`] ends this call without changing the program's run
/// state. Use [`execute_instruction`] inside `step` for checked fetch/dispatch.
///
/// Call [`ExecutionState::start`] before running a new invocation. Initialization
/// can inspect a suspended invocation and call [`ExecutionState::resume_at`] to
/// continue it. The execution guard is released on both success and returned
/// errors; unwinding through a panic does not release it. Device mutations are
/// retained, so the caller chooses whether an error can be retried.
///
/// # Errors
/// Rejects reentrant execution and propagates initialization or step errors.
pub fn run<M, S, State>(
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

/// Fetch and dispatch one instruction with the standard PC and error semantics.
///
/// Requires a runnable invocation with an active frame. Fetch checks the PC
/// against `module.code`, then advances it before calling the authored dispatch.
/// On a dispatch error, halt, or suspension, the PC is restored to the fetched
/// position. Other stack, frame, and device mutations are retained.
///
/// `instruction_override` replaces the fetched instruction for this dispatch
/// only. The PC must still address a valid module instruction. The debugger
/// uses this to execute a gutter breakpoint's displaced instruction without
/// removing the installed trap. Pass `None` for ordinary instruction fetching.
///
/// This helper does not acquire the execution guard itself. Custom runners
/// should call it inside [`run`] and supply the invocation's original module.
///
/// # Errors
/// Rejects a non-runnable invocation, missing frame, invalid PC, or PC overflow,
/// and propagates dispatch errors.
pub fn execute_instruction<M, I, V, Ty, Info, S>(
    module: &LocalModule<I, V, Ty, Info>,
    machine: &mut M,
    instruction_override: Option<&I>,
) -> eyre::Result<()>
where
    M: HasProgram<Program = ExecutionState<S>>
        + Dispatch<LocalModule<I, V, Ty, Info>, Instruction = I>,
{
    execute_instruction_with_context(module, machine, instruction_override, module)
}

/// Fetch and dispatch using a context supplied by the program driver.
///
/// The module supplies the instruction image. The context can also expose
/// route execution services to the authored step. PC and error handling are
/// the same as in [`execute_instruction`].
/// Custom runners should call this helper inside [`run`].
///
/// # Errors
/// Rejects invalid invocation state or fetches and propagates dispatch errors.
pub fn execute_instruction_with_context<M, I, V, Ty, Info, S, C: ?Sized>(
    module: &LocalModule<I, V, Ty, Info>,
    machine: &mut M,
    instruction_override: Option<&I>,
    context: &C,
) -> eyre::Result<()>
where
    M: HasProgram<Program = ExecutionState<S>> + Dispatch<C, Instruction = I>,
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
    let result = machine.dispatch(instruction, context);
    let program = machine.program_mut();
    if result.is_err() || !matches!(program.run_state, RunState::Runnable) {
        program.pc = position;
    }
    result
}
