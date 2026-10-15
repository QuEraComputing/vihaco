// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::data::component::canonical_bool;
use crate::word::decode_function_ref;
use crate::{CPU, StepOutcome, Word, cpu};
use vihaco::traits::{StackFrame, StackMemory};
use vihaco::{Execute, NoMessage};

impl Execute<cpu::Span> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::Span, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let (file, start, end) = (instruction.0, instruction.1, instruction.2);
        self.span = (file, start, end);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::Breakpoint> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &cpu::Breakpoint,
        _: NoMessage,
    ) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        Ok(StepOutcome::Breakpoint)
    }
}

impl Execute<cpu::Branch> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::Branch, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let target = instruction.0;
        self.set_pending_pc(target);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ConditionalBranch> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        instruction: &cpu::ConditionalBranch,
        _: NoMessage,
    ) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let (true_target, false_target) = (instruction.0, instruction.1);
        let cond = self.stack_pop()?;
        match canonical_bool(cond)? {
            true => {
                self.set_pending_pc(true_target);
                Ok(StepOutcome::Continue)
            }
            false => {
                self.set_pending_pc(false_target);
                Ok(StepOutcome::Continue)
            }
        }
    }
}

impl Execute<cpu::Return> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &cpu::Return, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let keep = instruction.0;
        let frame = *self.get_frame()?;
        let available = self
            .stack
            .len()
            .checked_sub(frame.operands_index())
            .ok_or_else(|| eyre::eyre!("frame locals out of bounds"))?;
        if available < keep as usize {
            return Err(eyre::eyre!("not enough values to return"));
        }

        self.pop_frame()?;
        // Collect return values before truncating
        let top = self.stack.len() - keep as usize;
        let return_values: Vec<Word> = self.stack[top..].to_vec();
        self.stack.drain(frame.base..top);

        if self.get_frame().is_err() {
            // No more frames - program is returning
            self.set_return_values(return_values);
            Ok(StepOutcome::Return)
        } else {
            self.set_pending_pc(frame.ret_pc);
            Ok(StepOutcome::Continue)
        }
    }
}

impl Execute<cpu::IndirectCall> for CPU {
    type Message = crate::message::FunctionInfo;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &cpu::IndirectCall,
        message: Self::Message,
    ) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let (arity, target) = (message.arity, message.start_address);
        let local_count = message.local_count;
        // Only the function reference is an operand; metadata comes from the message.
        let required = (arity as usize)
            .checked_add(1)
            .ok_or_else(|| eyre::eyre!("argument count overflow"))?;
        self.require_operands(required)?;
        let function = decode_function_ref(*self.stack_top()?);
        self.stack_pop()?;
        self.enter_function(arity, target, local_count, Some(function as usize))
    }
}

impl Execute<cpu::Call> for CPU {
    type Message = crate::message::FunctionInfo;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        instruction: &cpu::Call,
        message: Self::Message,
    ) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let (arity, target) = (instruction.0, instruction.1);
        let local_count = message.local_count;
        self.enter_function(arity, target, local_count, None)
    }
}

impl Execute<cpu::Halt> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::Halt, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        Ok(StepOutcome::Halt)
    }
}

impl Execute<cpu::Print> for CPU {
    type Message = crate::message::Print;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &cpu::Print,
        message: Self::Message,
    ) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        self.stack_pop()?;
        drop(message.0);
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::Dup> for CPU {
    type Message = NoMessage;
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::Dup, _: NoMessage) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();

        let v = *self.stack_top()?;
        self.stack.push(v);
        Ok(StepOutcome::Continue)
    }
}
