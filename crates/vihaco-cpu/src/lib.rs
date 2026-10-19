// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Checked word-stack CPU component.
//!
//! Enable `vihaco-cpu/typed-declarations` to use the `cpu` runtime dialect and
//! per-instruction `Execute` implementations. This forwards to the lower runtime
//! crates; facade applications can instead enable `vihaco/typed-declarations`.
//! Feature-off use retains the legacy instruction enums and `CPUMessage`.
//!
//! The nondefault `program-image-execution` feature implies `typed-declarations`
//! and switches CPU handlers to typed operands and stack result effects.
//! ProgramImage owns locals, operands, frames, and the program counter. Heap
//! storage remains on CPU and uses the same typed transfers.
//!
//! With this feature, program instructions and their lowering helpers live in
//! `vihaco_stdlib::program`; CPU exposes only computation and heap instructions.
//! The following syntax description and example describe the typed-only route.
//!
//! Typed instruction syntax retains readable literals until explicit lowering
//! encodes them into runtime `Word` payloads. Branch and call targets accept
//! numeric addresses or `@name` symbols; function references accept numeric
//! indices or function symbols. String constants accept indices or quoted text.
//! Loaders supply `lower::LabelLookup`, `lower::FunctionLookup`, and
//! `lower::StringIntern` to resolve them. `vihaco_stdlib::ProgramImage` implements
//! string interning using its section string table. Heap references remain numeric.
//! For explicit metadata, [`vihaco::loader::MetadataLoweringContext`] implements
//! all three lookup capabilities and rejects strings absent from the supplied
//! table. It works with `ParsedModule::lower_with_metadata` in `vihaco-syntax`.
//! `label @name` is retained in the parsed function's label metadata without an
//! executable slot. Labels, `function_start`, and `function_end` have no runtime
//! instruction leaves.
//! Composite source uses `device::dialect::mnemonic`, for example
//! `host::cpu::const_i64 -42`. Generated aggregate bytecode codecs are separate work.
//!
//! ```
//! # #[cfg(all(feature = "typed-declarations", not(feature = "program-image-execution")))]
//! # {
//! use vihaco_runtime::{Execute, NoMessage, Lower, Parse, Syntax, chumsky::Parser as _};
//! use vihaco_module::host::StackMemory;
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

#[cfg(not(feature = "program-image-execution"))]
mod data;

#[cfg(feature = "program-image-execution")]
#[path = "program_cpu.rs"]
mod data;

#[cfg(feature = "typed-declarations")]
#[cfg_attr(not(feature = "program-image-execution"), path = "cpu.rs")]
#[cfg_attr(feature = "program-image-execution", path = "computation.rs")]
mod dialect;

#[cfg(not(feature = "typed-declarations"))]
mod display;

#[cfg(not(feature = "typed-declarations"))]
mod instruction;

#[cfg(feature = "typed-declarations")]
pub use dialect::cpu;

#[cfg(feature = "typed-declarations")]
mod execute;

#[cfg(all(
    feature = "typed-declarations",
    not(feature = "program-image-execution")
))]
mod literal;

#[cfg(all(
    feature = "typed-declarations",
    not(feature = "program-image-execution")
))]
pub mod lower;

#[cfg(all(
    feature = "typed-declarations",
    not(feature = "program-image-execution")
))]
pub mod message;

#[cfg(all(
    feature = "typed-declarations",
    not(feature = "program-image-execution")
))]
pub mod operand;

#[cfg(feature = "typed-declarations")]
#[doc(hidden)]
pub use data::{__vihaco_fields_CPU, __vihaco_inventory_CPU};

#[cfg(not(feature = "program-image-execution"))]
mod outcome;
pub mod word;

pub use data::CPU;

#[cfg(not(feature = "typed-declarations"))]
pub use data::CPUMessage;

#[cfg(not(feature = "typed-declarations"))]
pub use data::{RuntimeInstruction, SurfaceInstruction};

#[cfg(not(feature = "typed-declarations"))]
pub use instruction::{SurfaceType, SurfaceValue};

#[cfg(not(feature = "program-image-execution"))]
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
        fn require_surface_instruction<T: vihaco_parser::SurfaceInstruction>() {}

        require_surface_instruction::<SurfaceInstruction>();
    }
}
