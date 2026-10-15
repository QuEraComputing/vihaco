// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Checked word-stack CPU component.
//!
//! Enable `vihaco-cpu/typed-declarations` to use the `cpu` runtime dialect and
//! per-instruction `Execute` implementations. This forwards the framework feature;
//! enabling the framework feature alone does not select CPU's typed declaration.
//! Feature-off use retains the legacy instruction enums and `CPUMessage`.
//!
//! Typed instruction syntax retains readable literals until explicit lowering
//! encodes them into runtime `Word` payloads. Branch and call targets accept
//! numeric addresses or `@name` symbols; function references accept numeric
//! indices or function symbols. String constants accept indices or quoted text.
//! Loaders supply `lower::LabelLookup`, `lower::FunctionLookup`, and
//! `lower::StringIntern` to resolve them. [`vihaco::ProgramImage`] implements
//! string interning using its section string table. Heap references remain numeric.
//! `label @name` is retained in the parsed function's label metadata without an
//! executable slot. Labels, `function_start`, and `function_end` have no runtime
//! instruction leaves.
//! Composite source uses `device::dialect::mnemonic`, for example
//! `host::cpu::const_i64 -42`. Generated aggregate bytecode codecs are separate work.
//!
//! ```
//! # #[cfg(feature = "typed-declarations")]
//! # {
//! use vihaco::{Execute, NoMessage, Lower, Parse, Syntax, chumsky::Parser as _, traits::StackMemory};
//! use vihaco_cpu::{CPU, cpu, encode_i64, StepOutcome};
//! let instruction = cpu::ConstI64::<Syntax>::parser().parse("const_i64 -42")
//!     .into_result().unwrap();
//! let instruction = instruction.lower(&mut ()).unwrap();
//! assert_eq!(instruction, cpu::ConstI64(encode_i64(-42)));
//! let mut cpu = CPU::default();
//! assert_eq!(cpu.execute(&instruction, NoMessage::new()).unwrap(), StepOutcome::Continue);
//! assert_eq!(cpu.stack(), &[encode_i64(-42)]);
//! # }
//! ```
//!
//! Equal string contents reuse the same section-local index:
//!
//! ```
//! # #[cfg(feature = "typed-declarations")]
//! # {
//! use vihaco::{Lower, NoContext, Parse, ProgramImage, Syntax, chumsky::Parser as _};
//! use vihaco_cpu::cpu;
//! let mut image = ProgramImage::<(), NoContext>::default();
//! for _ in 0..2 {
//!     let source = cpu::ConstString::<Syntax>::parser().parse("const_string \"hello\"")
//!         .into_result().unwrap();
//!     assert_eq!(source.lower(&mut image).unwrap(), cpu::ConstString(0));
//! }
//! assert_eq!(image.module.strings, ["hello"]);
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
pub mod lower;
#[cfg(feature = "typed-declarations")]
pub mod message;
#[cfg(feature = "typed-declarations")]
pub mod operand;
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
