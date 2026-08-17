// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use super::{ClockFault, GlobalDuration, GlobalTick};

/// A deterministic event queue ordered by global tick and insertion sequence.
pub struct GlobalClock<E> {
    now: GlobalTick,
    seq: u64,
    pending: BinaryHeap<Scheduled<E>>,
}

struct Scheduled<E> {
    tick: GlobalTick,
    seq: u64,
    event: E,
}

// `BinaryHeap` is a max-heap, so reverse the natural `(tick, seq)` ordering.
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
    pub fn new() -> Self {
        Self {
            now: GlobalTick::ZERO,
            seq: 0,
            pending: BinaryHeap::new(),
        }
    }

    /// Inserts owned scheduling work at an absolute global tick.
    pub fn schedule_at(&mut self, tick: GlobalTick, event: E) -> Result<(), ClockFault> {
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

    /// Converts relative work into an absolute global tick.
    pub fn schedule_after(&mut self, after: GlobalDuration, event: E) -> Result<(), ClockFault> {
        self.schedule_at(self.now.checked_add(after)?, event)
    }

    /// Removes the earliest event and advances the current global tick to it.
    pub fn pop_earliest(&mut self) -> Option<(GlobalTick, E)> {
        let Scheduled { tick, event, .. } = self.pending.pop()?;
        self.now = tick;
        Some((tick, event))
    }

    pub fn now(&self) -> GlobalTick {
        self.now
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

impl<E> Default for GlobalClock<E> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_ordered_by_tick_then_insertion_order() {
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
        assert_eq!(clock.now(), GlobalTick(10));
    }

    #[test]
    fn relative_scheduling_uses_current_time() {
        let mut clock = GlobalClock::new();
        clock.schedule_at(GlobalTick(4), "first").unwrap();
        assert_eq!(clock.pop_earliest(), Some((GlobalTick(4), "first")));

        clock.schedule_after(GlobalDuration(3), "second").unwrap();
        assert_eq!(clock.pop_earliest(), Some((GlobalTick(7), "second")));
    }

    #[test]
    fn rejects_scheduling_in_the_past() {
        let mut clock = GlobalClock::new();
        clock.schedule_at(GlobalTick(4), ()).unwrap();
        clock.pop_earliest();

        assert_eq!(
            clock.schedule_at(GlobalTick(3), ()),
            Err(ClockFault::SchedulingInPast)
        );
    }
}
