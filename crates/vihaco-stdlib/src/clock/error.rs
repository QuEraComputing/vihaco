// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// Errors produced while validating or advancing a modeled clock.
#[derive(Debug, PartialEq, Eq)]
pub enum ClockFault {
    ZeroTickRatio,
    LocalCycleOverflow,
    DurationOverflow,
    GlobalTickOverflow,
    SequenceOverflow,
    SchedulingInPast,
}

impl std::fmt::Display for ClockFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::ZeroTickRatio => "global ticks per local cycle must be nonzero",
            Self::LocalCycleOverflow => "local cycle arithmetic overflowed",
            Self::DurationOverflow => "global duration arithmetic overflowed",
            Self::GlobalTickOverflow => "global tick arithmetic overflowed",
            Self::SequenceOverflow => "global clock sequence overflowed",
            Self::SchedulingInPast => "cannot schedule an event in the past",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ClockFault {}
