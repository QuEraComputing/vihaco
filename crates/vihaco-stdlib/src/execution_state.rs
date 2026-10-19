// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Checked invocation state, independent of immutable program data.
use vihaco_abi::{frame::Frame, word::Word};
use vihaco_module::module::LocalModule;
mod storage;

/// Frames, locals, operands, and private execution state for one invocation.
///
/// The caller owns the module and supplies it at entry. No program reference or
/// instruction type is retained, so storage can be reused across modules.
#[derive(Debug)]
pub struct ExecutionState {
    stack: Vec<Word>,
    frames: Vec<Frame>,
    pc: u32,
    halted: bool,
    executing: bool,
}
impl Default for ExecutionState {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            frames: Vec::new(),
            pc: 0,
            halted: true,
            executing: false,
        }
    }
}
impl ExecutionState {
    pub fn new() -> Self {
        Self::default()
    }
}
impl Clone for ExecutionState {
    fn clone(&self) -> Self {
        Self {
            stack: self.stack.clone(),
            frames: self.frames.clone(),
            pc: self.pc,
            halted: self.halted,
            executing: false,
        }
    }
}

fn function_entry<I, V, Ty, Info>(
    module: &LocalModule<I, V, Ty, Info>,
    index: usize,
) -> eyre::Result<(usize, usize, u32)> {
    let function = module
        .functions
        .get(index)
        .ok_or_else(|| eyre::eyre!("unknown function reference {index}"))?;
    eyre::ensure!(
        function.local_count as usize >= function.signature.params.len(),
        "local count must be at least arity"
    );
    eyre::ensure!(
        (function.start_address as usize) < module.code.len(),
        "call target out of bounds"
    );
    Ok((
        function.signature.params.len(),
        function.local_count as usize,
        function.start_address,
    ))
}
