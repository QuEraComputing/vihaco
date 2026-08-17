// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn checked_time_operations_report_overflow() {
        assert_eq!(
            GlobalTick(u64::MAX).checked_add(GlobalDuration(1)),
            Err(ClockFault::GlobalTickOverflow)
        );
        assert_eq!(
            LocalCycles(u64::MAX).checked_add(LocalCycles(1)),
            Err(ClockFault::LocalCycleOverflow)
        );
        assert_eq!(
            LocalCycles(u64::MAX).checked_mul(GlobalTicksPerLocalCycle(2)),
            Err(ClockFault::DurationOverflow)
        );
        assert_eq!(
            GlobalTicksPerLocalCycle::new(0),
            Err(ClockFault::ZeroTickRatio)
        );
    }
}
