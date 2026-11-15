// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Runtime inputs supplied by the composite for particular CPU instructions.

use vihaco_abi::FunctionType;

/// Function metadata selected for a direct or indirect invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, vihaco_runtime::Message)]
pub struct FunctionInfo {
    pub arity: u32,
    pub start_address: u32,
    pub local_count: u32,
}

/// The function an indirect call selected: its frame metadata and its
/// declared signature.
///
/// `call_indirect` compares `signature` with the signature it carries and
/// fails before entering the function unless they are equal, so a function
/// reference can only be called at the type the verifier checked.
#[derive(Debug, Clone, PartialEq, Eq, vihaco_runtime::Message)]
pub struct IndirectCallee {
    pub info: FunctionInfo,
    pub signature: FunctionType,
}

/// Text prepared by the composite for a print instruction.
///
/// CPU execution consumes the stack operand. The composite or its observers
/// handle emitting the prepared text, as in the legacy path.
///
/// A print message cannot be passed to a call instruction:
///
/// ```compile_fail
/// use vihaco_runtime::Execute;
/// use vihaco_cpu::{CPU, cpu, message};
/// CPU::default().execute(&cpu::Call(0, 10), message::Print("text".into()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, vihaco_runtime::Message)]
pub struct Print(pub String);
