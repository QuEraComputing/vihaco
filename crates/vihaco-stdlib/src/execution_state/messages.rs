// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{ExecutionState, function_entry, program};
use vihaco_abi::word::{Word, decode_function_ref};
use vihaco_module::{PopStack, module::LocalModule};
use vihaco_runtime::{HasProgram, PrepareMessage};

#[derive(Debug)]
pub struct BranchMessage {
    pub(super) target: u32,
}

#[derive(Debug)]
pub struct CallMessage {
    pub(super) function: usize,
    pub(super) arity: usize,
    pub(super) locals: usize,
    pub(super) target: u32,
    pub(super) indirect: bool,
}

impl vihaco_runtime::Message for BranchMessage {}

impl vihaco_runtime::Message for CallMessage {}

impl vihaco_runtime::AutomaticMessage for BranchMessage {}

impl vihaco_runtime::AutomaticMessage for CallMessage {}

impl<P, F, Code, V, Ty, Info> PrepareMessage<P, F, program::Branch, LocalModule<Code, V, Ty, Info>>
    for BranchMessage
where
    P: HasProgram<Program = ExecutionState>,
{
    #[inline]
    fn prepare(
        parent: &mut P,
        instruction: &program::Branch,
        module: &LocalModule<Code, V, Ty, Info>,
    ) -> eyre::Result<Self> {
        parent.program().ensure_running()?;
        let target = instruction.0;
        eyre::ensure!(
            (target as usize) < module.code.len(),
            "branch target out of bounds"
        );
        Ok(Self { target })
    }
}

impl<P, F, Code, V, Ty, Info>
    PrepareMessage<P, F, program::ConditionalBranch, LocalModule<Code, V, Ty, Info>>
    for BranchMessage
where
    P: HasProgram<Program = ExecutionState>,
{
    #[inline]
    fn prepare(
        parent: &mut P,
        instruction: &program::ConditionalBranch,
        module: &LocalModule<Code, V, Ty, Info>,
    ) -> eyre::Result<Self> {
        parent.program().ensure_running()?;
        let condition = <ExecutionState as PopStack<Word>>::pop(parent.program_mut())?;
        let target = if condition != 0 {
            instruction.0
        } else {
            instruction.1
        };
        eyre::ensure!(
            (target as usize) < module.code.len(),
            "branch target out of bounds"
        );
        Ok(Self { target })
    }
}

impl<P, F, Code, V, Ty, Info> PrepareMessage<P, F, program::Call, LocalModule<Code, V, Ty, Info>>
    for CallMessage
where
    P: HasProgram<Program = ExecutionState>,
{
    #[inline(always)]
    fn prepare(
        parent: &mut P,
        instruction: &program::Call,
        module: &LocalModule<Code, V, Ty, Info>,
    ) -> eyre::Result<Self> {
        parent.program().ensure_running()?;
        let function = module
            .functions
            .iter()
            .position(|f| f.start_address == instruction.1)
            .ok_or_else(|| unknown_call_target(instruction.1))?;
        let (arity, locals, target) = function_entry(module, function)?;
        if instruction.0 as usize != arity {
            return Err(call_arity_mismatch());
        }
        Ok(Self {
            function,
            arity,
            locals,
            target,
            indirect: false,
        })
    }
}

impl<P, F, Code, V, Ty, Info>
    PrepareMessage<P, F, program::IndirectCall, LocalModule<Code, V, Ty, Info>> for CallMessage
where
    P: HasProgram<Program = ExecutionState>,
{
    #[inline]
    fn prepare(
        parent: &mut P,
        _: &program::IndirectCall,
        module: &LocalModule<Code, V, Ty, Info>,
    ) -> eyre::Result<Self> {
        parent.program().ensure_running()?;
        let function = decode_function_ref(parent.program().peek_operand()?) as usize;
        let (arity, locals, target) = function_entry(module, function)?;
        Ok(Self {
            function,
            arity,
            locals,
            target,
            indirect: true,
        })
    }
}

#[cold]
#[inline(never)]
fn unknown_call_target(target: u32) -> eyre::Report {
    eyre::eyre!("unknown call target {target}")
}

#[cold]
#[inline(never)]
fn call_arity_mismatch() -> eyre::Report {
    eyre::eyre!("call arity mismatch")
}
