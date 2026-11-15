// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Program-owned storage and control instructions.

#[vihaco_runtime::dialect]
pub mod program {
    use super::super::{
        literal::{F32Literal, F64Literal, I32Literal, I64Literal},
        operand::{StringOperand, Target},
    };
    use vihaco_abi::{FunctionType, word::Word};
    use vihaco_cpu::field::FunctionTypeOperand;
    #[instruction]
    #[pattern = "'br $0:Target"]
    pub struct Branch(pub u32);

    #[instruction]
    #[pattern = "'cond_br $0:Target `,` $1:Target"]
    pub struct ConditionalBranch(pub u32, pub u32);

    #[instruction]
    #[pattern = "'ret $0"]
    pub struct Return(pub u32);

    /// Call the function referenced by the top operand (`fn`), passing the
    /// arguments below it. The operand is the expected signature, written
    /// `call_indirect (u64, f64) -> u64`; the call fails at runtime unless the
    /// referenced function declares exactly this signature. Structs and named
    /// types in it may be named (`call_indirect (struct @Point) -> type
    /// @location`) and are resolved when the program is lowered.
    #[instruction]
    #[pattern = "'call_indirect $0:FunctionTypeOperand"]
    pub struct IndirectCall(pub FunctionType);

    #[instruction]
    #[pattern = "'call $0 `,` $1:Target"]
    pub struct Call(pub u32, pub u32);

    #[instruction]
    #[pattern = "'halt"]
    pub struct Halt;

    #[instruction]
    #[pattern = "'load_i32 $0"]
    pub struct LoadI32(pub u32);

    #[instruction]
    #[pattern = "'load_i64 $0"]
    pub struct LoadI64(pub u32);

    #[instruction]
    #[pattern = "'load_u32 $0"]
    pub struct LoadU32(pub u32);

    #[instruction]
    #[pattern = "'load_u64 $0"]
    pub struct LoadU64(pub u32);

    #[instruction]
    #[pattern = "'load_f32 $0"]
    pub struct LoadF32(pub u32);

    #[instruction]
    #[pattern = "'load_f64 $0"]
    pub struct LoadF64(pub u32);

    #[instruction]
    #[pattern = "'load_bool $0"]
    pub struct LoadBool(pub u32);

    #[instruction]
    #[pattern = "'store_i32 $0"]
    pub struct StoreI32(pub u32);

    #[instruction]
    #[pattern = "'store_i64 $0"]
    pub struct StoreI64(pub u32);

    #[instruction]
    #[pattern = "'store_u32 $0"]
    pub struct StoreU32(pub u32);

    #[instruction]
    #[pattern = "'store_u64 $0"]
    pub struct StoreU64(pub u32);

    #[instruction]
    #[pattern = "'store_f32 $0"]
    pub struct StoreF32(pub u32);

    #[instruction]
    #[pattern = "'store_f64 $0"]
    pub struct StoreF64(pub u32);

    #[instruction]
    #[pattern = "'store_bool $0"]
    pub struct StoreBool(pub u32);

    /// Push local `$0`, whatever its declared type. Unlike the typed loads,
    /// this can load struct, array, named-type, `fn`, and `str` locals.
    #[instruction]
    #[pattern = "'load $0"]
    pub struct Load(pub u32);

    /// Pop the top operand into local `$0`, which must have the operand's
    /// declared type. Unlike the typed stores, this can store struct, array,
    /// named-type, `fn`, and `str` values.
    #[instruction]
    #[pattern = "'store $0"]
    pub struct Store(pub u32);

    #[instruction]
    #[pattern = "'dup"]
    pub struct Dup;

    #[instruction]
    #[pattern = "'const_i32 $0:I32Literal"]
    pub struct ConstI32(pub Word);

    #[instruction]
    #[pattern = "'const_i64 $0:I64Literal"]
    pub struct ConstI64(pub Word);

    #[instruction]
    #[pattern = "'const_u32 $0:u32"]
    pub struct ConstU32(pub Word);

    #[instruction]
    #[pattern = "'const_u64 $0"]
    pub struct ConstU64(pub Word);

    #[instruction]
    #[pattern = "'const_f32 $0:F32Literal"]
    pub struct ConstF32(pub Word);

    #[instruction]
    #[pattern = "'const_f64 $0:F64Literal"]
    pub struct ConstF64(pub Word);

    #[instruction]
    #[pattern = "'const_bool $0:bool"]
    pub struct ConstBool(pub Word);

    #[instruction]
    #[pattern = "'const_string $0:StringOperand"]
    pub struct ConstString(pub Word);

    #[instruction]
    #[pattern = "'const_fn_ref $0:Target"]
    pub struct ConstFunctionRef(pub Word);
}
