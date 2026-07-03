// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use eyre::Result;

use super::Instruction;

use crate::{binary::ConstantId, frame::Frame, module::FunctionInfo};

pub trait ProgramCounter {
    type Instruction: Instruction;

    /// Get the current program counter.
    fn pc(&self) -> u32;

    /// Get a mutable reference to the program counter.
    fn pc_mut(&mut self) -> &mut u32;

    /// Get the instruction at the given program counter without advancing it.
    fn get_instruction(&self, pc: u32) -> Result<&Self::Instruction>;

    /// Peek the current instruction without advancing the program counter.
    fn peek_instruction(&self) -> Result<&Self::Instruction> {
        let pc = self.pc();
        self.get_instruction(pc)
    }

    /// Get the current instruction and advance the program counter by 1.
    fn next_instruction(&mut self) -> Result<&Self::Instruction> {
        let pc = self.pc();
        if let Some(v) = self.pc_mut().checked_add(1) {
            *self.pc_mut() = v;
        }
        self.get_instruction(pc)
    }
}

pub trait StackMemory {
    type Value;
    fn stack(&self) -> &Vec<Self::Value>;
    fn stack_mut(&mut self) -> &mut Vec<Self::Value>;
    fn stack_push<T: Into<Self::Value>>(&mut self, v: T);
    fn stack_pop(&mut self) -> Result<Self::Value>;
    fn stack_get(&self, pos: usize) -> Result<&Self::Value>;
    fn stack_get_mut(&mut self, pos: usize) -> Result<&mut Self::Value>;
    fn stack_len(&self) -> usize;
    fn stack_is_empty(&self) -> bool {
        self.stack_len() == 0
    }
    fn stack_top(&self) -> Result<&Self::Value> {
        if self.stack_is_empty() {
            Err(eyre::eyre!("stack underflow"))
        } else {
            self.stack_get(self.stack_len() - 1)
        }
    }
    fn stack_top_mut(&mut self) -> Result<&mut Self::Value> {
        let len = self.stack_len();
        if len == 0 {
            Err(eyre::eyre!("stack underflow"))
        } else {
            self.stack_get_mut(len - 1)
        }
    }
}

pub trait StackFrame {
    fn get_frame(&self) -> Result<&Frame>;
    fn get_frame_mut(&mut self) -> Result<&mut Frame>;
    fn push_frame(&mut self, frame: Frame);
    fn pop_frame(&mut self) -> Result<Frame>;
}

pub trait FrameMemory: StackFrame + StackMemory {
    fn frame_base(&self) -> Result<usize> {
        self.get_frame().map(|f| f.base)
    }

    fn get_local(&self, index: usize) -> Result<&Self::Value> {
        let base = self.frame_base()?;
        self.stack_get(base + index)
    }

    fn get_local_mut(&mut self, index: usize) -> Result<&mut Self::Value> {
        let base = self.frame_base()?;
        self.stack_get_mut(base + index)
    }
}

pub trait GetProgramGlobal {
    type Type;
    type Value;

    fn get_function(&self, index: usize) -> Result<FunctionInfo<Self::Type>>;
    fn get_string(&self, index: usize) -> Result<&String>;
    fn get_constant(&self, id: ConstantId) -> Result<&Self::Value>;
}

pub trait Stdout {
    type Output: std::io::Read + std::io::Write;
    fn stdout(&self) -> &Self::Output;
    fn stdout_mut(&mut self) -> &mut Self::Output;
}
