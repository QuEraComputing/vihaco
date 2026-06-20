// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandKind {
    NonNegativeU32,
    NonNegativeU64,
    I64,
    F64,
    Bool,
    String,
    Symbol,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalInstructionVariantSyntax {
    pub mnemonic: &'static str,
    pub operands: &'static [OperandKind],
}

impl CanonicalInstructionVariantSyntax {
    pub const fn new(mnemonic: &'static str, operands: &'static [OperandKind]) -> Self {
        Self { mnemonic, operands }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SugarOperandKind {
    PassThrough,
    PushConst(OperandKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstructionSugarVariantSyntax {
    pub mnemonic: &'static str,
    pub operands: &'static [SugarOperandKind],
}

impl InstructionSugarVariantSyntax {
    pub const fn new(mnemonic: &'static str, operands: &'static [SugarOperandKind]) -> Self {
        Self { mnemonic, operands }
    }
}

pub trait CanonicalInstructionSyntax {
    fn variants() -> &'static [CanonicalInstructionVariantSyntax] {
        &[]
    }
}

pub trait InstructionSugarSyntax {
    fn variants() -> &'static [InstructionSugarVariantSyntax] {
        &[]
    }
}
