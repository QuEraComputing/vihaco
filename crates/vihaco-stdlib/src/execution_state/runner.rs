// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::ExecutionState;
use super::instruction_out_of_bounds;
use vihaco_abi::heap::HeapError;
use vihaco_abi::word::Word;
use vihaco_module::module::LocalModule;
use vihaco_runtime::{Dispatch, HasProgram};

/// Enter main and execute through the composite's authored Steps.
///
/// This is the low-level, **unverified** runner: it accepts any plain module.
/// Load-time verification is enforced by [`Image`](crate::Image), which only
/// takes a `Verified` module, and so by [`Loadable`](crate::Loadable); use those
/// for programs from untrusted sources. Runtime checks (operand bounds, heap
/// tags, frame and call-depth limits) still apply here.
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
/// overflow, or a failure returned by the authored Dispatch/Step. A step's
/// failure is a [`StepError`] located at its instruction (`at @main+2
/// (address 2) `cpu::cpu::get_field @Point.y`: ...`), with heap types shown
/// by the module's names; the instruction text comes from
/// [`Dispatch::instruction_text`], so a hand-written machine without it shows
/// the location alone.
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
            result.map_err(|error| {
                let names = module.names();
                let text = M::instruction_text(instruction, &names);
                step_error(error, module, position, text)
            })?;
        }
        Ok(())
    })();
    machine.program_mut().executing = false;
    result
}

/// Locate a step's failure at its instruction, showing a heap error's types
/// by the module's names.
#[cold]
#[inline(never)]
fn step_error<I, V, Ty, Info>(
    error: eyre::Report,
    module: &LocalModule<I, V, Ty, Info>,
    position: u32,
    instruction: Option<String>,
) -> eyre::Report {
    let names = module.names();
    let message = match error.downcast_ref::<HeapError>() {
        Some(heap) => heap.display_with(&names).to_string(),
        None => format!("{error:#}"),
    };
    let location = match instruction {
        Some(text) => format!("{} `{text}`", names.location(position)),
        None => names.location(position).to_string(),
    };
    StepError {
        address: position,
        location,
        message,
        error,
    }
    .into()
}

/// An instruction failed while executing a program.
///
/// Displays as one line, the location, the instruction, then the failure:
/// `at @main+2 (address 2) `cpu::cpu::get_field @Point.x`: type error: heap
/// object (slot 1, generation 1) is struct Pair, expected struct Point`.
///
/// The step's original error is its [`source`](std::error::Error::source),
/// so `eyre::Report::chain` and `downcast_ref` on the chain reach it (for
/// example a [`HeapError`] or a device's own error type); [`StepError::error`]
/// returns it directly. `downcast_ref::<HeapError>()` on the returned report
/// itself finds the `StepError`, not the inner error: search the chain.
#[derive(Debug)]
pub struct StepError {
    address: u32,
    location: String,
    message: String,
    error: eyre::Report,
}

impl StepError {
    /// The section-local address of the failing instruction.
    pub fn address(&self) -> u32 {
        self.address
    }

    /// The error the instruction's step returned.
    pub fn error(&self) -> &eyre::Report {
        &self.error
    }

    /// Consume the location, returning the step's error.
    pub fn into_error(self) -> eyre::Report {
        self.error
    }
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at {}: {}", self.location, self.message)
    }
}

impl std::error::Error for StepError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}
