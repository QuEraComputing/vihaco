// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Checked word-stack CPU component.
//!
//! Enable `vihaco-cpu/typed-declarations` to use the `cpu` runtime dialect and
//! per-instruction `Execute` implementations. This forwards the framework feature;
//! enabling the framework feature alone does not select CPU's typed declaration.
//! Feature-off use retains the legacy instruction enums and `CPUMessage`.
//!
//! Typed instructions parse directly into executable values. Branch/call addresses
//! and string/function/heap reference IDs must already be resolved by the SST
//! producer. Constants accept readable literals and store encoded `Word` payloads.
//! `label @name` is consumed by the SST function parser as readability metadata;
//! labels, `function_start`, and `function_end` have no runtime instruction leaves.
//! Composite source uses `device::dialect::mnemonic`, for example
//! `host::cpu::const_i64 -42`. Generated aggregate bytecode codecs are separate work.
//!
//! ```
//! # #[cfg(feature = "typed-declarations")]
//! # {
//! use vihaco::{Execute, Parse, chumsky::Parser as _, traits::StackMemory};
//! use vihaco_cpu::{CPU, cpu, encode_i64, StepOutcome};
//! let instruction = cpu::ConstI64::parser().parse("const_i64 -42")
//!     .into_result().unwrap();
//! assert_eq!(instruction, cpu::ConstI64(encode_i64(-42)));
//! let mut cpu = CPU::default();
//! assert_eq!(cpu.execute(&instruction, ()).unwrap(), StepOutcome::Continue);
//! assert_eq!(cpu.stack(), &[encode_i64(-42)]);
//! # }
//! ```

mod data;
#[cfg(feature = "typed-declarations")]
#[path = "cpu.rs"]
mod dialect;
#[cfg(not(feature = "typed-declarations"))]
mod display;
#[cfg(not(feature = "typed-declarations"))]
mod instruction;
#[cfg(feature = "typed-declarations")]
pub use dialect::cpu;
#[cfg(feature = "typed-declarations")]
mod execute;
#[cfg(feature = "typed-declarations")]
mod literal;
#[cfg(feature = "typed-declarations")]
pub mod message;
#[cfg(feature = "typed-declarations")]
#[doc(hidden)]
pub use data::{__vihaco_fields_CPU, __vihaco_inventory_CPU};
mod outcome;
pub mod word;
pub use data::CPU;
#[cfg(not(feature = "typed-declarations"))]
pub use data::CPUMessage;
#[cfg(not(feature = "typed-declarations"))]
pub use data::{RuntimeInstruction, SurfaceInstruction};
#[cfg(not(feature = "typed-declarations"))]
pub use instruction::{SurfaceType, SurfaceValue};
pub use outcome::StepOutcome;
pub use word::{
    Word, decode_bool, decode_f32, decode_f64, decode_function_ref, decode_heap_ref, decode_i32,
    decode_i64, decode_string_id, decode_u32, decode_u64, encode_bool, encode_f32, encode_f64,
    encode_function_ref, encode_heap_ref, encode_i32, encode_i64, encode_string_id, encode_u32,
    encode_u64,
};

#[cfg(all(test, not(feature = "typed-declarations")))]
mod tests {
    use super::*;

    #[test]
    fn surface_instruction_implements_surface_marker() {
        fn require_surface_instruction<T: vihaco::SurfaceInstruction>() {}

        require_surface_instruction::<SurfaceInstruction>();
    }
}
