// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod machine;

// The encoding/instruction/event-sink traits + `Reset` moved to `vihaco-abi`;
// re-export them here so intra-crate `crate::traits::*` paths still resolve.
pub use machine::{FrameMemory, GetProgramInfo, ProgramCounter, StackFrame, StackMemory, Stdout};
pub use vihaco_abi::traits::{
    EffectSink, FromBytes, FromBytesWithOpcode, FromText, Instruction, OpCode, Reset, WriteBytes,
};
