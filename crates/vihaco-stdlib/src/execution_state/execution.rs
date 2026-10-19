// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Checked execution of the program dialect, including its PC and halt state.

use super::program;
use super::{
    ExecutionState,
    messages::{BranchMessage, CallMessage},
};

use vihaco_abi::word::Word;
use vihaco_module::PopStack;
use vihaco_runtime::{Execute, NoMessage};

impl ExecutionState {
    // Direct and indirect calls share checked frame setup and reservation order.
    fn enter(
        &mut self,
        function: usize,
        arity: usize,
        locals: usize,
        target: u32,
        indirect: bool,
    ) -> eyre::Result<()> {
        let required = arity
            .checked_add(usize::from(indirect))
            .ok_or_else(|| eyre::eyre!("argument count overflow"))?;
        self.require_operands(required)?;
        let ret_pc = self.pc;
        let base = self.stack.len() - required;
        let end = base
            .checked_add(locals)
            .ok_or_else(|| eyre::eyre!("frame size overflow"))?;
        self.stack
            .try_reserve(end.saturating_sub(self.stack.len()))?;
        self.frames.try_reserve(1)?;
        if indirect {
            self.stack.pop();
        }
        self.stack.resize(end, 0);
        self.frames.push(vihaco_abi::frame::Frame {
            base,
            local_count: locals,
            span: (0, 0, 0),
            function: Some(function),
            ret_pc,
        });
        self.pc = target;
        Ok(())
    }
}

impl Execute<program::ConstI32> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::ConstI32, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        self.stack.push(instruction.0);
        Ok(())
    }
}

impl Execute<program::ConstI64> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::ConstI64, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        self.stack.push(instruction.0);
        Ok(())
    }
}

impl Execute<program::ConstU32> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::ConstU32, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        self.stack.push(instruction.0);
        Ok(())
    }
}

impl Execute<program::ConstU64> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::ConstU64, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        self.stack.push(instruction.0);
        Ok(())
    }
}

impl Execute<program::ConstF32> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::ConstF32, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        self.stack.push(instruction.0);
        Ok(())
    }
}

impl Execute<program::ConstF64> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::ConstF64, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        self.stack.push(instruction.0);
        Ok(())
    }
}

impl Execute<program::ConstBool> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::ConstBool, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        self.stack.push(instruction.0);
        Ok(())
    }
}

impl Execute<program::ConstString> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::ConstString, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        self.stack.push(instruction.0);
        Ok(())
    }
}

impl Execute<program::ConstFunctionRef> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(
        &mut self,
        instruction: &program::ConstFunctionRef,
        _: NoMessage,
    ) -> eyre::Result<()> {
        self.ensure_running()?;
        self.stack.push(instruction.0);
        Ok(())
    }
}

impl Execute<program::ConstHeapRef> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::ConstHeapRef, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        self.stack.push(instruction.0);
        Ok(())
    }
}

impl Execute<program::LoadI32> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::LoadI32, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        self.stack.push(self.stack[address]);
        Ok(())
    }
}

impl Execute<program::LoadI64> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::LoadI64, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        self.stack.push(self.stack[address]);
        Ok(())
    }
}

impl Execute<program::LoadU32> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::LoadU32, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        self.stack.push(self.stack[address]);
        Ok(())
    }
}

impl Execute<program::LoadU64> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::LoadU64, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        self.stack.push(self.stack[address]);
        Ok(())
    }
}

impl Execute<program::LoadF32> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::LoadF32, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        self.stack.push(self.stack[address]);
        Ok(())
    }
}

impl Execute<program::LoadF64> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::LoadF64, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        self.stack.push(self.stack[address]);
        Ok(())
    }
}

impl Execute<program::LoadBool> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::LoadBool, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        self.stack.push(self.stack[address]);
        Ok(())
    }
}

impl Execute<program::StoreI32> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::StoreI32, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        let value = <Self as PopStack<Word>>::pop(self)?;
        self.stack[address] = value;
        Ok(())
    }
}

impl Execute<program::StoreI64> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::StoreI64, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        let value = <Self as PopStack<Word>>::pop(self)?;
        self.stack[address] = value;
        Ok(())
    }
}

impl Execute<program::StoreU32> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::StoreU32, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        let value = <Self as PopStack<Word>>::pop(self)?;
        self.stack[address] = value;
        Ok(())
    }
}

impl Execute<program::StoreU64> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::StoreU64, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        let value = <Self as PopStack<Word>>::pop(self)?;
        self.stack[address] = value;
        Ok(())
    }
}

impl Execute<program::StoreF32> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::StoreF32, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        let value = <Self as PopStack<Word>>::pop(self)?;
        self.stack[address] = value;
        Ok(())
    }
}

impl Execute<program::StoreF64> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::StoreF64, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        let value = <Self as PopStack<Word>>::pop(self)?;
        self.stack[address] = value;
        Ok(())
    }
}

impl Execute<program::StoreBool> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::StoreBool, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let address = self.local_address(instruction.0)?;
        let value = <Self as PopStack<Word>>::pop(self)?;
        self.stack[address] = value;
        Ok(())
    }
}

impl Execute<program::Dup> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, _instruction: &program::Dup, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let value = self.peek_operand()?;
        self.stack.push(value);
        Ok(())
    }
}

impl Execute<program::Return> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, instruction: &program::Return, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        let keep = instruction.0 as usize;
        self.require_operands(keep)?;
        let frame = *self
            .frames
            .last()
            .ok_or_else(|| eyre::eyre!("no current frame"))?;
        let len = self.stack.len();
        self.stack.copy_within(len - keep..len, frame.base);
        self.stack.truncate(frame.base + keep);
        self.frames.pop();
        if self.frames.is_empty() {
            self.halted = true;
        } else {
            self.pc = frame.ret_pc;
        }
        Ok(())
    }
}

impl Execute<program::Halt> for ExecutionState {
    type Message = NoMessage;
    type Effect = ();
    type Error = eyre::Report;

    #[inline]
    fn execute(&mut self, _instruction: &program::Halt, _: NoMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        self.halted = true;
        Ok(())
    }
}

impl Execute<program::Branch> for ExecutionState {
    type Message = BranchMessage;
    type Effect = ();
    type Error = eyre::Report;
    #[inline]
    fn execute(&mut self, _: &program::Branch, message: BranchMessage) -> eyre::Result<()> {
        self.ensure_running()?;
        self.pc = message.target;
        Ok(())
    }
}

impl Execute<program::ConditionalBranch> for ExecutionState {
    type Message = BranchMessage;
    type Effect = ();
    type Error = eyre::Report;
    #[inline]
    fn execute(
        &mut self,
        _: &program::ConditionalBranch,
        message: BranchMessage,
    ) -> eyre::Result<()> {
        self.ensure_running()?;
        self.pc = message.target;
        Ok(())
    }
}

impl Execute<program::Call> for ExecutionState {
    type Message = CallMessage;
    type Effect = ();
    type Error = eyre::Report;
    #[inline]
    fn execute(&mut self, _: &program::Call, message: CallMessage) -> eyre::Result<()> {
        self.enter(
            message.function,
            message.arity,
            message.locals,
            message.target,
            message.indirect,
        )
    }
}

impl Execute<program::IndirectCall> for ExecutionState {
    type Message = CallMessage;
    type Effect = ();
    type Error = eyre::Report;
    #[inline]
    fn execute(&mut self, _: &program::IndirectCall, message: CallMessage) -> eyre::Result<()> {
        self.enter(
            message.function,
            message.arity,
            message.locals,
            message.target,
            message.indirect,
        )
    }
}
