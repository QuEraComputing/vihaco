// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! CPU component for typed arithmetic, bitwise, logic, comparison, and heap instructions.
//!
//! Composite Steps deliver typed operands to its `Execute` implementations and
//! handle the resulting effects. The standard `vihaco_stdlib::ExecutionState`
//! component owns local slots, operands, frames, and the program counter.
//! Program instructions and their lowering helpers live in `vihaco_stdlib::program`.

#[path = "computation.rs"]
mod dialect;

mod execute;

#[path = "program_cpu.rs"]
mod data;

pub mod word;

pub use data::CPU;
#[doc(hidden)]
pub use data::{__vihaco_fields_CPU, __vihaco_inventory_CPU};
pub use dialect::cpu;
pub use word::{
    Word, decode_bool, decode_f32, decode_f64, decode_function_ref, decode_heap_ref, decode_i32,
    decode_i64, decode_string_id, decode_u32, decode_u64, encode_bool, encode_f32, encode_f64,
    encode_function_ref, encode_heap_ref, encode_i32, encode_i64, encode_string_id, encode_u32,
    encode_u64,
};
