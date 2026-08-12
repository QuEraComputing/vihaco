// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod component;
mod data;
mod display;
mod instruction;
mod outcome;
pub use component::message::FunctionInfo;
pub use component::{
    Add, And, BitAnd, BitOr, BitXor, Branch, Breakpoint, Call, ConditionalBranch, Const, Div, Dup,
    Eq, FunctionEnd, FunctionStart, Ge, GetItem, Gt, Halt, HeapAlloc, HeapDealloc, IndirectCall,
    Label, Le, Load, Lt, Mul, Ne, Neg, Not, Or, Print, PrintEffect, Rem, Return, Rol, Ror, Shl,
    Shr, Span, Store, Sub, Xor, message,
};
pub use data::CPU;
pub use instruction::{SurfaceInstruction, SurfaceType, SurfaceValue};
pub use outcome::StepOutcome;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn surface_instruction_implements_surface_marker() {
        fn require_surface_instruction<T: vihaco::SurfaceInstruction>() {}

        require_surface_instruction::<SurfaceInstruction>();
    }
}
