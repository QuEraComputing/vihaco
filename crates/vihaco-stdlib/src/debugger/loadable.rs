// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{DebugAction, Debugger, DebuggerState, HasBreakpointInstruction, IsBreakpointToggled};
use crate::{ExecutionState, Image, Loadable};
use vihaco_module::module::LocalModule;
use vihaco_runtime::{Composite, Dispatch, Field, HasProgram};

impl<Inst, V, Ty, Info, M> Loadable<Image<Inst, V, Ty, Info>, M>
where
    M: Composite + HasProgram<Program = ExecutionState<DebuggerState>>,
{
    /// Install a gutter breakpoint in the owned image.
    pub fn set_breakpoint<F>(&mut self, pc: u32) -> IsBreakpointToggled
    where
        M: Field<F, Child = Debugger<u32, Inst>>,
        Inst: HasBreakpointInstruction,
    {
        let (image, machine) = self.breakpoint_parts_mut();
        <M as Field<F>>::get_mut(machine).set_breakpoint(&mut image.module, pc)
    }

    /// Restore a gutter breakpoint's instruction in the owned image.
    pub fn clear_breakpoint<F>(&mut self, pc: u32) -> IsBreakpointToggled
    where
        M: Field<F, Child = Debugger<u32, Inst>>,
        Inst: HasBreakpointInstruction,
    {
        let (image, machine) = self.breakpoint_parts_mut();
        <M as Field<F>>::get_mut(machine).clear_breakpoint(&mut image.module, pc)
    }

    /// Continue or single-step an invocation through the selected debugger.
    pub fn resume_with_breaks<F>(&mut self, action: DebugAction) -> eyre::Result<()>
    where
        M: Field<F, Child = Debugger<u32, Inst>>
            + Dispatch<LocalModule<Inst, V, Ty, Info>, Instruction = Inst>,
        Inst: Clone,
    {
        let (image, machine) = self.parts_mut();
        super::resume_with_breaks::<F, _, _, _, _, _>(&image.module, machine, action)
    }
}
