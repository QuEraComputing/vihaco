// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

// ===========================================================================================
// === AUTHOR: reusable, machine-agnostic components =========================================
// ===========================================================================================
//
// None of these know about routes, `CpuA`/`CpuB`, `MachineEvent`, or `HeterogeneousMachine`. They
// are the `stack` / `arithmetic` / `clock` / `channel` library pieces the demo composes. In this
// first demo their shared boundary type is `i64`, so no cross-component cast is ever performed.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// A generic, deterministic event queue over a machine-defined event sum `E`.
///
/// Note on layering: this is a reusable library component, not vihaco core (see `demo.md`:
/// "local and global clocks" live under reusable component libraries, and "the reusable library
/// item is `GlobalClock<E>`, not a general runtime driver"). What core actually owns is the
/// contract around it: the owned child-step outcome (`Execution::Complete`/`Parked`), route
/// dispatch, and the runtime root owning an event loop. The queue policy itself is swappable. A
/// machine wanting fixed or state-dependent latency drops in a different component with the same
/// shape, the way `ChannelFabric` is swappable, without changing core.
///
/// It owns timeline state (`now`, a monotonic `seq`) but never calls back into its containing
/// composite, fetches an instruction, or knows the machine's private fields. Events are ordered by
/// `(tick, seq)`; the sequence number gives stable ordering to events scheduled for the same global
/// tick. Host execution time never contributes to modeled duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct GlobalTick(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct GlobalDuration(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct LocalCycles(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct GlobalTicksPerLocalCycle(u64);

/// Runtime instructions provide the local duration of their own operation.
trait TimedInstruction {
    fn local_cycles(&self) -> LocalCycles;
}

/// Owned scheduling work returned by a clocked component. The parent adds any child identity
/// before submitting the request to its root `GlobalClock`.
struct Schedule<E> {
    at: GlobalTick,
    event: E,
}

/// Generic boundary for a component that participates in a global event loop.
///
/// The trait shares only clock vocabulary with `GlobalClock`: ticks, instruction timing, and
/// owned scheduling requests. It does not depend on a particular clock implementation or root
/// event enum. Components supply their own instruction, event, completion, and fault types.
trait ClockedComponent {
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

impl GlobalTick {
    const ZERO: Self = Self(0);

    fn checked_add(self, duration: GlobalDuration) -> Result<Self, ClockFault> {
        self.0
            .checked_add(duration.0)
            .map(Self)
            .ok_or(ClockFault::GlobalTickOverflow)
    }
}

impl LocalCycles {
    const ONE: Self = Self(1);

    fn checked_add(self, other: Self) -> Result<Self, ClockFault> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(ClockFault::LocalCycleOverflow)
    }

    fn checked_mul(self, ratio: GlobalTicksPerLocalCycle) -> Result<GlobalDuration, ClockFault> {
        self.0
            .checked_mul(ratio.0)
            .map(GlobalDuration)
            .ok_or(ClockFault::DurationOverflow)
    }
}

impl GlobalTicksPerLocalCycle {
    fn new(value: u64) -> Result<Self, ClockFault> {
        (value != 0)
            .then_some(Self(value))
            .ok_or(ClockFault::ZeroTickRatio)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ClockFault {
    ZeroTickRatio,
    LocalCycleOverflow,
    DurationOverflow,
    GlobalTickOverflow,
    SequenceOverflow,
    SchedulingInPast,
}

struct GlobalClock<E> {
    now: GlobalTick,
    seq: u64,
    pending: BinaryHeap<Scheduled<E>>,
}

struct Scheduled<E> {
    tick: GlobalTick,
    seq: u64,
    event: E,
}

// `BinaryHeap` is a max-heap, so reverse the natural `(tick, seq)` ordering. This makes the
// earliest event the heap's greatest element while keeping the event payload unconstrained.
impl<E> Ord for Scheduled<E> {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .tick
            .cmp(&self.tick)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl<E> PartialOrd for Scheduled<E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<E> PartialEq for Scheduled<E> {
    fn eq(&self, other: &Self) -> bool {
        (self.tick, self.seq) == (other.tick, other.seq)
    }
}

impl<E> Eq for Scheduled<E> {}

impl<E> GlobalClock<E> {
    fn new() -> Self {
        Self {
            now: GlobalTick::ZERO,
            seq: 0,
            pending: BinaryHeap::new(),
        }
    }

    /// Insert owned scheduling work at an absolute global tick.
    fn schedule_at(&mut self, tick: GlobalTick, event: E) -> Result<(), ClockFault> {
        if tick < self.now {
            return Err(ClockFault::SchedulingInPast);
        }
        let seq = self
            .seq
            .checked_add(1)
            .ok_or(ClockFault::SequenceOverflow)?;
        self.seq = seq;
        self.pending.push(Scheduled { tick, seq, event });
        Ok(())
    }

    /// Convert child-local relative work into an absolute global tick.
    fn schedule_after(&mut self, after: GlobalDuration, event: E) -> Result<(), ClockFault> {
        self.schedule_at(self.now.checked_add(after)?, event)
    }

    /// Remove the earliest owned event by `(tick, seq)`, advancing `now` to it. Returns the event
    /// and its tick, or `None` when the timeline is exhausted.
    fn pop_earliest(&mut self) -> Option<(GlobalTick, E)> {
        let Scheduled { tick, event, .. } = self.pending.pop()?;
        // Global time is monotonic: `now` only ever advances to the dispatched event's tick.
        self.now = tick;
        Some((tick, event))
    }

    fn now(&self) -> GlobalTick {
        self.now
    }

    fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    #[test]
    fn heap_returns_events_in_timeline_order() {
        let mut clock = GlobalClock::new();
        clock.schedule_at(GlobalTick(10), 10).unwrap();
        clock.schedule_at(GlobalTick(2), 2).unwrap();
        clock.schedule_at(GlobalTick(2), 3).unwrap();
        clock.schedule_at(GlobalTick(1), 1).unwrap();

        assert_eq!(clock.pop_earliest(), Some((GlobalTick(1), 1)));
        assert_eq!(clock.pop_earliest(), Some((GlobalTick(2), 2)));
        assert_eq!(clock.pop_earliest(), Some((GlobalTick(2), 3)));
        assert_eq!(clock.pop_earliest(), Some((GlobalTick(10), 10)));
        assert_eq!(clock.pop_earliest(), None);
    }
}
