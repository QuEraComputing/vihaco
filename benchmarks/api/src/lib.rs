// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Stable measurement boundary. No vihaco types belong in this crate.

pub use eyre::Result;

/// Call the CPU operation directly, without enum dispatch.
pub const CPU_OPERATION: u8 = 0;
/// Dispatch through the CPU instruction enum.
pub const CPU_ENUM: u8 = 1;
/// Dispatch through the composite machine and then the CPU.
pub const COMPOSITE: u8 = 2;

/// A checkout's implementation of the shared workload contracts.
/// Keep loading, allocation of initial state, and destruction outside timing.
/// Programs must implement the algorithm and numeric semantics of the shared
/// workload ID, even when syntax, instruction encoding, or runtime APIs change.
/// Adapters own the entire execution loop; the harness never dispatches steps.
pub trait BenchmarkMachine {
    /// Immutable loaded program, shared by repeated invocations.
    type Program;
    /// Fresh mutable execution state for one invocation.
    type Invocation;

    /// Load the checkout's implementation of a stable workload ID.
    fn load(workload: &str) -> Result<Self::Program>;
    /// Initialize the program with the shared workload inputs, outside timing.
    fn prepare(program: &Self::Program, iterations: u64, seed: u64) -> Result<Self::Invocation>;
    /// Execute a complete invocation and read its result. The const parameter
    /// selects CPU-only (`false`) or composite (`true`) dispatch outside the
    /// timed step loop. Both routes include fetch, execution, and PC advancement.
    fn execute<const COMPOSITE: bool>(
        invocation: &mut Self::Invocation,
        program: &Self::Program,
    ) -> Result<u64>;
}

/// One constant instruction, without a program fetch/PC loop.
/// ROUTE is CPU_OPERATION, CPU_ENUM, or COMPOSITE. Instruction types and state
/// may differ between checkouts; the harness owns their black-box barriers.
pub trait ConstantBenchmark<const ROUTE: u8> {
    type State;
    type Instruction;

    fn prepare() -> Result<Self::State>;
    fn instruction(value: u64) -> Self::Instruction;
    /// Execute once, check successful continuation, and read the pushed value.
    fn execute(state: &mut Self::State, instruction: &Self::Instruction) -> Result<u64>;
}
