// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod component;
mod data;
mod display;
mod instruction;
mod outcome;
pub mod word;
pub use component::CPUMessage;
pub use data::CPU;
pub use data::{RuntimeInstruction, SurfaceInstruction};
pub use instruction::{SurfaceType, SurfaceValue};
pub use outcome::StepOutcome;
pub use word::{
    Word, decode_bool, decode_f32, decode_f64, decode_function_ref, decode_heap_ref, decode_i32,
    decode_i64, decode_string_id, decode_u32, decode_u64, encode_bool, encode_f32, encode_f64,
    encode_function_ref, encode_heap_ref, encode_i32, encode_i64, encode_string_id, encode_u32,
    encode_u64,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_instruction_implements_surface_marker() {
        fn require_surface_instruction<T: vihaco::SurfaceInstruction>() {}

        require_surface_instruction::<SurfaceInstruction>();
    }
}
