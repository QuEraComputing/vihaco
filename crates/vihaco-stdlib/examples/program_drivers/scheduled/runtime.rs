// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Load programs, queue their entries, and let the backend order execution turns.

use super::{
    backends::Drive,
    driver::{Driver, Loaded, Pause, StepContext},
    tasks::TaskId,
};
use eyre::{Result, ensure};
use std::collections::{HashMap, HashSet};
use vihaco_abi::word::Word;
use vihaco_runtime::{Composite, Dispatch, HasInstructionSet, HasProgram};
use vihaco_stdlib::{ExecutionState, execution_state::RunState};

pub(crate) type MachineId = usize;

enum Turn {
    Entry { machine: MachineId, args: Vec<Word> },
    Issue(MachineId),
}

pub(crate) struct Runtime<M: Composite + HasInstructionSet, D: Drive> {
    backend: D,
    driver: Driver<M, D::Submitter>,
    machines: Vec<Loaded<M>>,
    turns: HashMap<TaskId, Turn>,
    active: HashSet<MachineId>,
}

impl<M, D> Runtime<M, D>
where
    M: Composite + HasInstructionSet + HasProgram<Program = ExecutionState<Pause>> + 'static,
    M: for<'a> Dispatch<StepContext<'a, M, D::Submitter>, Instruction = M::Runtime>,
    D: Drive,
{
    pub(crate) fn new(backend: D) -> Self {
        let driver = Driver::new(backend.submitter());
        Self {
            backend,
            driver,
            machines: Vec::new(),
            turns: HashMap::new(),
            active: HashSet::new(),
        }
    }

    pub(crate) fn load(&mut self, program: Loaded<M>) -> MachineId {
        let machine = self.machines.len();
        self.machines.push(program);
        machine
    }

    /// Queue entry through the backend. Starting the invocation happens in run.
    pub(crate) fn schedule_entry(&mut self, machine: MachineId, args: Vec<Word>) -> Result<()> {
        ensure!(machine < self.machines.len(), "unknown machine {machine}");
        ensure!(!self.active.contains(&machine), "machine already scheduled");
        self.enqueue(Turn::Entry { machine, args })?;
        self.active.insert(machine);
        Ok(())
    }

    fn enqueue(&mut self, turn: Turn) -> Result<()> {
        let id = self.backend.enqueue_turn()?;
        self.turns.insert(id, turn);
        Ok(())
    }

    /// Load a program, schedule its entry, and drive the runtime to halt.
    pub(crate) fn execute(&mut self, program: Loaded<M>, args: Vec<Word>) -> Result<MachineId> {
        let machine = self.load(program);
        self.schedule_entry(machine, args)?;
        self.run()?;
        Ok(machine)
    }

    /// Drive every scheduled invocation to halt. Returns halt order for inspection.
    pub(crate) fn run(&mut self) -> Result<Vec<MachineId>> {
        let mut halted = Vec::new();
        while !self.active.is_empty() {
            halted.extend(self.advance_one()?);
        }
        Ok(halted)
    }

    /// Execute one ready entry, issue, or resume turn.
    ///
    /// Returns the machine ID if the turn halted its program.
    pub(crate) fn advance_one(&mut self) -> Result<Option<MachineId>> {
        if self.active.is_empty() {
            return Ok(None);
        }

        let id = self.backend.next_ready()?;
        let (machine, result) = match self.turns.remove(&id) {
            Some(Turn::Entry { machine, args }) => (
                machine,
                self.driver.enter(&mut self.machines[machine], &args),
            ),
            Some(Turn::Issue(machine)) => (
                machine,
                self.driver.issue_one(machine, &mut self.machines[machine]),
            ),
            None => {
                let machine = self.driver.waiting_machine(id)?;
                (machine, self.driver.resume(id, &mut self.machines[machine]))
            }
        };
        if let Err(error) = result {
            self.active.remove(&machine);
            return Err(error);
        }

        match self.machine(machine).program().run_state() {
            RunState::Runnable => self.enqueue(Turn::Issue(machine))?,
            RunState::Halted => {
                ensure!(self.active.remove(&machine), "unscheduled machine halted");
                return Ok(Some(machine));
            }
            RunState::Other(_) => {}
        }
        Ok(None)
    }

    pub(crate) fn machine(&self, machine: MachineId) -> &M {
        self.machines[machine].machine()
    }

    pub(crate) fn backend(&self) -> &D {
        &self.backend
    }
}
