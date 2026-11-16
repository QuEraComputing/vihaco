// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Checked word backing for frame locals and typed operands.

use super::{ExecutionState, RunState, function_entry};
use vihaco_abi::{frame::Frame, traits::Reset, word::*};
use vihaco_module::module::LocalModule;
use vihaco_module::{PopStack, PushStack};

impl<S> ExecutionState<S> {
    /// Inspect the running state without changing it.
    pub fn run_state(&self) -> &RunState<S> {
        &self.run_state
    }

    /// Materialized locals and operands, or results after the outermost return.
    pub fn stack(&self) -> &[Word] {
        &self.stack
    }

    /// Active call frames, from main to the current function.
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// True before entry, after Halt, or after the outermost return.
    pub fn is_halted(&self) -> bool {
        matches!(self.run_state, RunState::Halted)
    }

    /// Enter main using its module metadata and host-supplied arguments.
    ///
    /// Entry creates the outermost frame, zeroes non-argument locals, selects
    /// the function's first instruction, and sets the run state to runnable.
    /// A halted invocation can be replaced while reusing allocated storage.
    ///
    /// # Errors
    /// Rejects an active invocation, invalid metadata/target, a wrong argument
    /// count, or allocation failure, preserving the preceding logical state.
    pub fn start<I, V, Ty, Info>(
        &mut self,
        module: &LocalModule<I, V, Ty, Info>,
        arguments: &[Word],
    ) -> eyre::Result<()> {
        eyre::ensure!(
            matches!(self.run_state, RunState::Halted) && !self.executing,
            "a function is already active"
        );
        let function = module
            .main_function
            .ok_or_else(|| eyre::eyre!("module has no main function"))?;
        let (arity, locals, target) = function_entry(module, function as usize)?;
        eyre::ensure!(arguments.len() == arity, "entry argument count mismatch");
        self.stack
            .try_reserve(locals.saturating_sub(self.stack.len()))?;
        if self.frames.capacity() == 0 {
            self.frames.try_reserve(1)?;
        }
        self.stack.clear();
        self.stack.extend_from_slice(arguments);
        self.stack.resize(locals, 0);
        self.frames.clear();
        self.frames.push(Frame {
            base: 0,
            local_count: locals,
            // Frame's shared record retains this legacy field; this path does
            // not track source spans or implement a Span instruction.
            span: (0, 0, 0),
            function: Some(function as usize),
            ret_pc: 0,
        });
        self.pc = target;
        self.run_state = RunState::Runnable;
        Ok(())
    }

    /// Current continuation, or the fault/terminal position after execution stops.
    pub fn pc(&self) -> u32 {
        self.pc
    }

    pub(crate) fn resume_at(&mut self, pc: u32) {
        self.pc = pc;
        self.run_state = RunState::Runnable;
    }

    pub(crate) fn suspend(&mut self, state: S) {
        self.run_state = RunState::Other(state);
    }

    #[inline(always)]
    pub(super) fn ensure_running(&self) -> eyre::Result<()> {
        eyre::ensure!(
            matches!(self.run_state, RunState::Runnable) && !self.frames.is_empty(),
            "program is not running"
        );
        Ok(())
    }

    #[inline]
    pub(super) fn require_operands(&self, count: usize) -> eyre::Result<()> {
        self.ensure_running()?;
        let frame = self
            .frames
            .last()
            .ok_or_else(|| eyre::eyre!("no current frame"))?;
        eyre::ensure!(
            self.stack.len().saturating_sub(frame.operands_index()) >= count,
            "stack underflow"
        );
        Ok(())
    }

    #[inline]
    pub(super) fn peek_operand(&self) -> eyre::Result<Word> {
        self.require_operands(1)?;
        self.stack
            .last()
            .copied()
            .ok_or_else(|| eyre::eyre!("stack underflow"))
    }

    #[inline]
    pub(super) fn local_address(&self, index: u32) -> eyre::Result<usize> {
        self.ensure_running()?;
        let frame = self
            .frames
            .last()
            .ok_or_else(|| eyre::eyre!("no current frame"))?;
        eyre::ensure!(
            (index as usize) < frame.local_count,
            "local index out of bounds"
        );
        let address = frame
            .base
            .checked_add(index as usize)
            .ok_or_else(|| eyre::eyre!("local address overflow"))?;
        eyre::ensure!(address < self.stack.len(), "local index out of bounds");
        Ok(address)
    }
}

impl<S> Reset for ExecutionState<S> {
    fn reset(&mut self) {
        self.stack.clear();
        self.frames.clear();
        self.pc = 0;
        self.run_state = RunState::Halted;
    }
}

impl<S> PopStack<Word> for ExecutionState<S> {
    #[inline]
    fn pop(&mut self) -> eyre::Result<Word> {
        self.require_operands(1)?;
        self.stack
            .pop()
            .ok_or_else(|| eyre::eyre!("stack underflow"))
    }
}

// Insertion is storage plumbing, not execution: it never starts an invocation
// or changes PC. Step wiring supplies results only after successful preparation.
impl<S> PushStack<Word> for ExecutionState<S> {
    #[inline]
    fn push(&mut self, value: Word) {
        self.stack.push(value);
    }
}

macro_rules! float_access {
    ($ty:ty, $decode:ident, $encode:ident) => {
        impl<S> PopStack<$ty> for ExecutionState<S> {
            #[inline]
            fn pop(&mut self) -> eyre::Result<$ty> {
                <Self as PopStack<Word>>::pop(self).map($decode)
            }
        }
        impl<S> PushStack<$ty> for ExecutionState<S> {
            #[inline]
            fn push(&mut self, value: $ty) {
                <Self as PushStack<Word>>::push(self, $encode(value));
            }
        }
    };
}

float_access!(f32, decode_f32, encode_f32);

float_access!(f64, decode_f64, encode_f64);
