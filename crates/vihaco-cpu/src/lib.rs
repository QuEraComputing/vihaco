// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod component;
mod data;
mod display;
mod instruction;
mod outcome;
pub use component::CPUMessage;
pub use data::CPU;
pub use data::{RuntimeInstruction, SurfaceInstruction};
pub use instruction::{SurfaceType, SurfaceValue};
pub use outcome::StepOutcome;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_instruction_implements_surface_marker() {
        fn require_surface_instruction<T: vihaco::SurfaceInstruction>() {}

        require_surface_instruction::<SurfaceInstruction>();
    }

    #[test]
    fn cpu_instruction_derive_exposes_explicit_canonical_syntax_entries() {
        let variants = <RuntimeInstruction as vihaco::CanonicalInstructionSyntax>::variants();

        let expect = [
            ("cpu::const_i64", &[vihaco::OperandKind::I64][..]),
            ("cpu::const_f64", &[vihaco::OperandKind::F64][..]),
            ("cpu::const_bool", &[vihaco::OperandKind::Bool][..]),
            ("cpu::const_u64", &[vihaco::OperandKind::NonNegativeU64][..]),
            ("cpu::fn_ref", &[vihaco::OperandKind::Symbol][..]),
            ("cpu::call_direct", &[vihaco::OperandKind::Symbol][..]),
        ];

        for (mnemonic, operands) in expect {
            let syntax = variants
                .iter()
                .find(|syntax| syntax.mnemonic == mnemonic)
                .unwrap_or_else(|| panic!("missing canonical syntax entry for {mnemonic}"));

            assert_eq!(
                syntax.operands, operands,
                "unexpected operands for canonical syntax entry {mnemonic}"
            );
        }
    }
}
