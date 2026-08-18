// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::ClockFault;

/// An absolute position on the modeled global timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlobalTick(pub u64);

/// A distance between two positions on the modeled global timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlobalDuration(pub u64);

/// Work measured in the local clock domain of a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalCycles(pub u64);

/// The conversion ratio from local cycles to global ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlobalTicksPerLocalCycle(pub u64);

/// Owned scheduling work returned by a clocked component.
#[derive(Debug, PartialEq, Eq)]
pub struct Schedule<E> {
    pub at: GlobalTick,
    pub event: E,
}

impl GlobalTick {
    pub const ZERO: Self = Self(0);

    pub fn checked_add(self, duration: GlobalDuration) -> Result<Self, ClockFault> {
        self.0
            .checked_add(duration.0)
            .map(Self)
            .ok_or(ClockFault::GlobalTickOverflow)
    }
}

impl LocalCycles {
    pub const ONE: Self = Self(1);

    pub fn checked_add(self, other: Self) -> Result<Self, ClockFault> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(ClockFault::LocalCycleOverflow)
    }

    pub fn checked_mul(
        self,
        ratio: GlobalTicksPerLocalCycle,
    ) -> Result<GlobalDuration, ClockFault> {
        self.0
            .checked_mul(ratio.0)
            .map(GlobalDuration)
            .ok_or(ClockFault::DurationOverflow)
    }
}

impl GlobalTicksPerLocalCycle {
    pub fn new(value: u64) -> Result<Self, ClockFault> {
        (value != 0)
            .then_some(Self(value))
            .ok_or(ClockFault::ZeroTickRatio)
    }
}
