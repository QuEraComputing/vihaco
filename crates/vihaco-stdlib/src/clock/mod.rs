// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Reusable global-clock and timing vocabulary for vihaco runtimes.
//!
//! [`GlobalClock`] is an ordered event queue. It owns modeled timeline state,
//! but does not execute instructions, dispatch events, or know about the
//! machine that owns it. A runtime can use the types in this module to build
//! its own scheduling policy and root event loop.

mod error;
mod queue;
mod traits;
mod types;

#[cfg(test)]
mod types_tests;

pub use error::ClockFault;
pub use queue::GlobalClock;
pub use traits::{ClockedComponent, TimedInstruction};
pub use types::{GlobalDuration, GlobalTick, GlobalTicksPerLocalCycle, LocalCycles, Schedule};
