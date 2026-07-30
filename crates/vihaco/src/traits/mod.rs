// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

// The encoding/instruction/event-sink traits + `Reset` moved to `vihaco-abi`;
// re-export them here so intra-crate `crate::traits::*` paths still resolve.
pub use vihaco_abi::traits::{
    EffectSink, FromBytes, FromBytesWithOpcode, FromText, Instruction, OpCode, Reset, WriteBytes,
};
// The host-VM traits moved to `vihaco-module`; re-export so `crate::traits::*`
// (and the `crate::machine` shim) still resolve.
pub use vihaco_module::host::{
    FrameMemory, GetProgramInfo, ProgramCounter, StackFrame, StackMemory, Stdout,
};
