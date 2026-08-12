// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A global-clock coordinator for waveform-like channel components.
//!
//! `CounterGroup` owns queued and active channels. `CounterMachine` owns only the timeline:
//! runtime instructions queue channels or begin playback, and each playback tick asks the group
//! for one `PlayReport`. The report is then sent to observers, which is the same boundary a future
//! FPGA waveform component can use. Sampling is deliberately one global tick for now; a later
//! component timing trait can replace `schedule_next_advance` without moving clock ownership into
//! the channel component.

use std::convert::Infallible;

use crate::{
    clock::{ClockFault, GlobalClock, GlobalDuration, GlobalTick},
    counter::counter_group::{self, CounterGroup},
    debug_trace::DebugTrace,
};
#[derive(Debug, Clone, Copy)]
pub enum MachineEvent {
    Step,
    AdvanceCounters,
}

vihaco::composite! {
    pub composite CounterMachine {
        error = CounterMachineFault;

        pub clock: GlobalClock<MachineEvent>,
        pub counter_group: CounterGroup,
        pub debug: DebugTrace,

        program: Vec<CounterMachineInstruction>,
        pc: usize,
        advance_scheduled: bool,
    }

    runtime {
        Queue(counter_group::runtime::instruction::Queue) => counter_group {
            message none;
        }
        Play(counter_group::runtime::instruction::Play) => counter_group {
            message none;
            effects {
                absorb with debug;
            }
        }
    }
}

impl CounterMachine {
    pub fn new(program: Vec<CounterMachineInstruction>) -> Self {
        Self {
            clock: GlobalClock::new(),
            counter_group: CounterGroup::new(),
            debug: DebugTrace::new(),
            program,
            pc: 0,
            advance_scheduled: false,
        }
    }

    pub fn run(&mut self) -> Result<RunOutcome, CounterMachineFault> {
        self.clock
            .schedule_at(GlobalTick::ZERO, MachineEvent::Step)?;

        while let Some((tick, event)) = self.clock.pop_earliest() {
            match event {
                MachineEvent::Step => self.step(tick)?,
                MachineEvent::AdvanceCounters => self.advance_counters(tick)?,
            }
        }

        Ok(RunOutcome::Completed)
    }

    fn step(&mut self, tick: GlobalTick) -> Result<(), CounterMachineFault> {
        let Some(instruction) = self.program.get(self.pc).cloned() else {
            return Ok(());
        };

        self.execute_generated(&instruction)?;
        self.pc += 1;

        if matches!(instruction, CounterMachineInstruction::Play(_))
            && self.counter_group.is_playing()
            && !self.advance_scheduled
        {
            self.schedule_next_advance(tick)?;
        }

        if self.pc < self.program.len() {
            let next_step = if matches!(instruction, CounterMachineInstruction::Play(_)) {
                tick.checked_add(GlobalDuration(1))?
            } else {
                tick
            };
            self.clock.schedule_at(next_step, MachineEvent::Step)?;
        }
        Ok(())
    }

    fn advance_counters(&mut self, tick: GlobalTick) -> Result<(), CounterMachineFault> {
        self.advance_scheduled = false;
        let report = self.counter_group.advance();
        self.debug.record_at(tick.0, &report);

        if self.counter_group.is_playing() {
            self.schedule_next_advance(tick)?;
        }
        Ok(())
    }

    fn schedule_next_advance(&mut self, tick: GlobalTick) -> Result<(), CounterMachineFault> {
        self.clock.schedule_at(
            tick.checked_add(GlobalDuration(1))?,
            MachineEvent::AdvanceCounters,
        )?;
        self.advance_scheduled = true;
        Ok(())
    }

    /// The initial clock position is exposed here only as a convenient scaffold anchor.
    pub fn now(&self) -> GlobalTick {
        self.clock.now()
    }
}

impl Default for CounterMachine {
    fn default() -> Self {
        Self::new(vec![])
    }
}

#[derive(Debug)]
pub enum CounterMachineFault {
    Clock(ClockFault),
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunOutcome {
    Completed,
}

#[cfg(test)]
mod tests {
    use super::{CounterMachine, CounterMachineInstruction, RunOutcome};
    use crate::counter::counter_group::runtime::instruction;

    #[test]
    fn queued_counters_are_advanced_on_shared_global_ticks() {
        let program = vec![
            CounterMachineInstruction::Queue(instruction::Queue {
                start: 10,
                duration: 2,
            }),
            CounterMachineInstruction::Queue(instruction::Queue {
                start: 20,
                duration: 3,
            }),
            CounterMachineInstruction::Play(instruction::Play),
        ];
        let mut machine = CounterMachine::new(program);

        assert_eq!(machine.run().unwrap(), RunOutcome::Completed);
        assert_eq!(machine.now().0, 3);
        assert_eq!(machine.debug.records.len(), 4);
    }

    #[test]
    fn a_later_play_starts_after_the_first_play_tick() {
        let program = vec![
            CounterMachineInstruction::Queue(instruction::Queue {
                start: 10,
                duration: 2,
            }),
            CounterMachineInstruction::Play(instruction::Play),
            CounterMachineInstruction::Queue(instruction::Queue {
                start: 100,
                duration: 2,
            }),
            CounterMachineInstruction::Play(instruction::Play),
        ];
        let mut machine = CounterMachine::new(program);

        machine.run().unwrap();

        let reports = machine
            .debug
            .records
            .iter()
            .filter(|record| record.route == "clock")
            .map(|record| record.effect.clone())
            .collect::<Vec<_>>();
        assert!(reports[0].contains("tick 1"));
        assert!(reports[0].contains("CounterId(0)"));
        assert!(!reports[0].contains("CounterId(1)"));
        assert!(reports[1].contains("CounterId(1)"));
    }
}

impl From<ClockFault> for CounterMachineFault {
    fn from(fault: ClockFault) -> Self {
        CounterMachineFault::Clock(fault)
    }
}

impl From<Infallible> for CounterMachineFault {
    fn from(never: Infallible) -> Self {
        match never {}
    }
}
