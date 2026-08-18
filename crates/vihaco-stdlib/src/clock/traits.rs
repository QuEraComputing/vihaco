// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{GlobalTick, GlobalTicksPerLocalCycle, Schedule};

/// Runtime instructions provide the local duration of their operation.
pub trait TimedInstruction {
    fn local_cycles(&self) -> super::LocalCycles;
}

/// Generic boundary for a component that participates in a global event loop.
pub trait ClockedComponent: Sized {
    type Event;
    type Completion;
    type Fault;

    fn step_at(
        &mut self,
        global_tick: GlobalTick,
        ticks_per_local_cycle: GlobalTicksPerLocalCycle,
    ) -> Result<Option<Schedule<Self::Event>>, Self::Fault>;

    fn resume(
        &mut self,
        completion: Self::Completion,
        global_tick: GlobalTick,
        ticks_per_local_cycle: GlobalTicksPerLocalCycle,
    ) -> Result<Option<Schedule<Self::Event>>, Self::Fault>;

    fn next_boundary_at(
        &self,
        global_tick: GlobalTick,
        ticks_per_local_cycle: GlobalTicksPerLocalCycle,
    ) -> Result<GlobalTick, Self::Fault>;
}
