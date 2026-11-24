// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Experimental host interface for programs that can park within an instruction.
//!
//! A driver resolves machine IDs to invocations. Checked drivers use the
//! machine's `HasProgram` implementation to reach its execution state and PC.

use vihaco_abi::word::Word;

/// Authorization to fetch instructions for one invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunCommand {
    /// Run until a breakpoint, suspension, or halt.
    Continue,
    /// Retire one instruction, even if it takes several deliveries.
    SingleStep,
}

/// Why the driver returned control to the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramStop<W> {
    /// The originating instruction still owns in-flight work.
    Pending { pc: u32, work: W },
    /// A single-step command retired this instruction.
    Retired { pc: u32 },
    /// A breakpoint stopped execution at this instruction.
    Breakpoint { pc: u32 },
    /// The invocation has halted.
    Halted { pc: u32 },
}

/// Owns program issue and resumption for one or more machines.
///
/// A delivery advances an existing instruction under its saved command.
/// Ordinary execution faults travel through `eyre::Result`.
pub trait ProgramDriver {
    /// The machine selected by a host command.
    type MachineId: Copy;

    /// Identifies pending work for the selected machine.
    type WorkId: Copy;

    /// A backend-specific readiness event or completion payload.
    type Delivery;

    /// Enter the selected machine's loaded program with the supplied arguments.
    fn start(&mut self, machine: Self::MachineId, arguments: &[Word]) -> eyre::Result<()>;

    /// Issue instructions under the supplied command until the program stops.
    fn run(
        &mut self,
        machine: Self::MachineId,
        command: RunCommand,
    ) -> eyre::Result<ProgramStop<Self::WorkId>>;

    /// Deliver work to a parked instruction and honor its saved command.
    fn deliver(
        &mut self,
        machine: Self::MachineId,
        work: Self::WorkId,
        delivery: Self::Delivery,
    ) -> eyre::Result<ProgramStop<Self::WorkId>>;
}
