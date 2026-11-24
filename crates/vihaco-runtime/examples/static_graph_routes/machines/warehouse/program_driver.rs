// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Run example bytecode programs through generated dispatch and a route driver.

use crate::framework::{Route, Run};
use eyre::{Result, ensure};
use std::{cell::RefCell, rc::Rc};
use vihaco_abi::instruction::{Instruction as BytecodeInstruction, WriteBytes};
use vihaco_bytecode::decode_instruction_stream;
use vihaco_runtime::Dispatch;

pub(super) fn encode(instructions: &[impl WriteBytes]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for instruction in instructions {
        instruction.write_bytes(&mut bytes)?;
    }
    Ok(bytes)
}

pub(super) struct Program<I> {
    code: Vec<I>,
}

impl<I> Program<I> {
    pub(super) fn load<B: BytecodeInstruction + Into<I>>(bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            code: decode_instruction_stream::<B>(bytes)?
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }
}

type Completion = Rc<RefCell<Option<Result<()>>>>;

/// Supplies the route driver and the current instruction's completion slot.
pub(super) struct ProgramContext<'a, D> {
    backend: &'a D,
    completion: &'a Completion,
}

impl<D> ProgramContext<'_, D> {
    pub(super) fn start<M, I, P>(&self, machine: &mut M, route: &Route<I, P>, instruction: I)
    where
        P: Run<M, I, D, Output = ()>,
    {
        let completion = Rc::clone(self.completion);
        route.start_with(machine, self.backend, instruction, move |_, _, result| {
            *completion.borrow_mut() = Some(result);
        });
    }
}

#[derive(PartialEq, Eq)]
enum State {
    Ready,
    InFlight,
    Faulted,
}

pub(super) struct ProgramDriver<'a, I, D> {
    program: &'a Program<I>,
    backend: &'a D,
    completion: Completion,
    pc: usize,
    state: State,
}

impl<'a, I, D> ProgramDriver<'a, I, D> {
    pub(super) fn new(program: &'a Program<I>, backend: &'a D) -> Self {
        Self {
            program,
            backend,
            completion: Rc::default(),
            pc: 0,
            state: State::Ready,
        }
    }

    #[cfg(test)]
    pub(super) fn pc(&self) -> usize {
        self.pc
    }

    pub(super) fn run<M>(
        &mut self,
        machine: &mut M,
        mut drive_pending: impl FnMut(&mut M, &D) -> Result<()>,
    ) -> Result<()>
    where
        M: for<'context> Dispatch<ProgramContext<'context, D>, Instruction = I>,
    {
        ensure!(
            self.state != State::Faulted,
            "program faulted at PC {}",
            self.pc
        );
        let context = ProgramContext {
            backend: self.backend,
            completion: &self.completion,
        };

        while let Some(instruction) = self.program.code.get(self.pc) {
            if self.state == State::Ready {
                if let Err(error) = machine.dispatch(instruction, &context) {
                    self.state = State::Faulted;
                    return Err(error);
                }
                self.state = State::InFlight;
            }

            let Some(result) = self.completion.borrow_mut().take() else {
                drive_pending(machine, self.backend)?;
                continue;
            };
            if let Err(error) = result {
                self.state = State::Faulted;
                return Err(error);
            }

            self.pc += 1;
            self.state = State::Ready;
        }
        Ok(())
    }
}
