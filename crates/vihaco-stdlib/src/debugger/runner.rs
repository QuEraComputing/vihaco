// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{DebugAction, Debugger, DebuggerState, PausedBreakpoint};
use crate::execution_state::runner::{execute_instruction, run};
use crate::{ExecutionState, RunState};
use std::ops::ControlFlow;
use vihaco_module::module::LocalModule;
use vihaco_runtime::{Dispatch, Field, HasProgram};

/// Continue or single-step execution through the selected debugger.
pub fn resume_with_breaks<F, M, I, V, Ty, Info>(
    module: &LocalModule<I, V, Ty, Info>,
    machine: &mut M,
    action: DebugAction,
) -> eyre::Result<()>
where
    M: HasProgram<Program = ExecutionState<DebuggerState>>
        + Field<F, Child = Debugger<u32, I>>
        + Dispatch<LocalModule<I, V, Ty, Info>, Instruction = I>,
    I: Clone,
{
    run(
        machine,
        |machine| {
            let paused = <M as Field<F>>::get_mut(machine).paused.take();
            match *machine.program().run_state() {
                RunState::Halted => Ok(None),
                RunState::Other(DebuggerState::Breakpoint { resume_pc }) => {
                    let position = machine.program().pc();
                    let paused = paused.unwrap_or_else(|| {
                        if <M as Field<F>>::get(machine).contains(&position) {
                            PausedBreakpoint::Gutter { key: position }
                        } else {
                            PausedBreakpoint::Explicit
                        }
                    });
                    match paused {
                        PausedBreakpoint::Explicit => {
                            machine.program_mut().resume_at(resume_pc);
                            Ok(None)
                        }
                        PausedBreakpoint::Gutter { key } => {
                            machine.program_mut().resume_at(position);
                            Ok(Some((key, resume_pc)))
                        }
                    }
                }
                RunState::Runnable => Ok(None),
            }
        },
        |machine, original_at| {
            let position = machine.program().pc();
            let displaced = original_at
                .and_then(|(key, _)| <M as Field<F>>::get(machine).displaced(&key))
                .cloned();
            let result = execute_instruction(module, machine, displaced.as_ref());
            if matches!(
                machine.program().run_state(),
                RunState::Other(DebuggerState::Breakpoint { .. })
            ) {
                let debugger = <M as Field<F>>::get_mut(machine);
                let paused = if original_at.is_none() && debugger.contains(&position) {
                    PausedBreakpoint::Gutter { key: position }
                } else {
                    PausedBreakpoint::Explicit
                };
                debugger.paused = Some(paused);
            } else if result.is_err()
                && matches!(machine.program().run_state(), RunState::Runnable)
                && let Some((key, resume_pc)) = *original_at
            {
                <M as Field<F>>::get_mut(machine).paused = Some(PausedBreakpoint::Gutter { key });
                machine
                    .program_mut()
                    .suspend(DebuggerState::Breakpoint { resume_pc });
            }
            result?;
            *original_at = None;
            Ok(match action {
                DebugAction::Step => ControlFlow::Break(()),
                DebugAction::Continue => ControlFlow::Continue(()),
            })
        },
    )
}
