// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Load programs, queue their entries, and let the backend order execution turns.

use super::{
    backends::Drive,
    digitizer::Machine,
    owner::{Loaded, Owner},
    tasks::TaskId,
};
use eyre::{Result, ensure};
use std::collections::{HashMap, HashSet};
use vihaco_abi::word::Word;
use vihaco_runtime::HasProgram;
use vihaco_stdlib::execution_state::RunState;

pub(crate) type MachineId = usize;

enum Turn {
    Entry { machine: MachineId, args: Vec<Word> },
    Issue(MachineId),
}

pub(crate) struct Runtime<D: Drive> {
    backend: D,
    owner: Owner<D::Submitter>,
    turns: HashMap<TaskId, Turn>,
    active: HashSet<MachineId>,
}

impl<D: Drive> Runtime<D> {
    pub(crate) fn new(backend: D) -> Self {
        let owner = Owner::new(Vec::new(), backend.submitter());
        Self {
            backend,
            owner,
            turns: HashMap::new(),
            active: HashSet::new(),
        }
    }

    pub(crate) fn load(&mut self, program: Loaded) -> MachineId {
        let machine = self.owner.machines.len();
        self.owner.machines.push(program);
        machine
    }

    /// Queue entry through the backend. Starting the invocation happens in run.
    pub(crate) fn schedule_entry(&mut self, machine: MachineId, args: Vec<Word>) -> Result<()> {
        ensure!(
            machine < self.owner.machines.len(),
            "unknown machine {machine}"
        );
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

    /// Drive every scheduled invocation to halt. Returns halt order for inspection.
    pub(crate) fn run(&mut self) -> Result<Vec<MachineId>> {
        let mut halted = Vec::new();
        while !self.active.is_empty() {
            let id = self.backend.next_ready()?;
            // Ready IDs name either a scheduled program turn or a route result.
            let machine = match self.turns.remove(&id) {
                Some(Turn::Entry { machine, args }) => {
                    self.owner.enter(machine, &args)?;
                    machine
                }
                Some(Turn::Issue(machine)) => {
                    self.owner.issue_one(machine)?;
                    machine
                }
                None => self.owner.resume(id)?,
            };
            match self.machine(machine).program().run_state() {
                RunState::Runnable => self.enqueue(Turn::Issue(machine))?,
                RunState::Halted => {
                    ensure!(self.active.remove(&machine), "unscheduled machine halted");
                    halted.push(machine);
                }
                RunState::Other(_) => {}
            }
        }
        Ok(halted)
    }

    pub(crate) fn machine(&self, machine: MachineId) -> &Machine {
        self.owner.machines[machine].machine()
    }

    pub(crate) fn backend(&self) -> &D {
        &self.backend
    }
}
