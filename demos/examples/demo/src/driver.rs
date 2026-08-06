// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[cfg(test)]
mod tests {
    use crate::{
        arithmetic::Add,
        channel::Recv,
        clock::{ClockFault, GlobalTicksPerLocalCycle, LocalCycles, TimedInstruction},
        cpu::RuntimeInstruction,
        surface::CHANNEL_A_TO_B,
    };

    #[test]
    fn instruction_timing_belongs_to_the_instruction() {
        assert_eq!(
            RuntimeInstruction::IntegerAdd(Add).local_cycles(),
            LocalCycles::ONE
        );
        assert_eq!(
            RuntimeInstruction::Recv(Recv {
                channel: CHANNEL_A_TO_B,
            })
            .local_cycles(),
            LocalCycles::ONE
        );
    }

    #[test]
    fn clocked_component_rejects_zero_ratio() {
        assert!(matches!(
            GlobalTicksPerLocalCycle::new(0),
            Err(ClockFault::ZeroTickRatio)
        ));
    }
}
