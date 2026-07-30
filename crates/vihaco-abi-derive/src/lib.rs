// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod common;
mod derive_instruction;

use proc_macro::TokenStream;

/// Derive the bytecode-instruction traits (`OpCode`, `FromBytesWithOpcode`,
/// `WriteBytes`) for an instruction enum.
///
/// Generated code is rooted at whichever crate the consumer actually depends
/// on (`vihaco` facade or `vihaco-abi` directly), resolved at expansion time.
/// Override with `#[vihaco(crate = ::some::path)]` on the deriving type.
#[proc_macro_derive(Instruction, attributes(instruction, opcode, vihaco))]
pub fn derive_instruction(input: TokenStream) -> TokenStream {
    derive_instruction::expand(input)
}
