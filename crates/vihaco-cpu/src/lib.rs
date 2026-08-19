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
}
