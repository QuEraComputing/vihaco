// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Computational and heap operations; operands belong to ProgramImage.

#[vihaco_runtime::dialect]
pub mod cpu {
    #[instruction]
    #[pattern = "'heap_alloc $0"]
    pub struct HeapAlloc(pub u32);

    #[instruction]
    #[pattern = "'get_item"]
    pub struct GetItem;

    #[instruction]
    #[pattern = "'heap_dealloc"]
    pub struct HeapDealloc;

    #[instruction]
    #[pattern = "'heap_reserve"]
    pub struct HeapReserve;

    #[instruction]
    #[pattern = "'heap_push"]
    pub struct HeapPush;

    #[instruction]
    #[pattern = "'heap_len"]
    pub struct HeapLen;

    #[instruction]
    #[pattern = "'add_i32"]
    pub struct AddI32;

    #[instruction]
    #[pattern = "'add_f32"]
    pub struct AddF32;

    #[instruction]
    #[pattern = "'add_i64"]
    pub struct AddI64;

    #[instruction]
    #[pattern = "'add_u32"]
    pub struct AddU32;

    #[instruction]
    #[pattern = "'add_u64"]
    pub struct AddU64;

    #[instruction]
    #[pattern = "'add_f64"]
    pub struct AddF64;

    #[instruction]
    #[pattern = "'sub_i32"]
    pub struct SubI32;

    #[instruction]
    #[pattern = "'sub_i64"]
    pub struct SubI64;

    #[instruction]
    #[pattern = "'sub_u32"]
    pub struct SubU32;

    #[instruction]
    #[pattern = "'sub_u64"]
    pub struct SubU64;

    #[instruction]
    #[pattern = "'sub_f32"]
    pub struct SubF32;

    #[instruction]
    #[pattern = "'sub_f64"]
    pub struct SubF64;

    #[instruction]
    #[pattern = "'mul_i32"]
    pub struct MulI32;

    #[instruction]
    #[pattern = "'mul_i64"]
    pub struct MulI64;

    #[instruction]
    #[pattern = "'mul_u32"]
    pub struct MulU32;

    #[instruction]
    #[pattern = "'mul_u64"]
    pub struct MulU64;

    #[instruction]
    #[pattern = "'mul_f32"]
    pub struct MulF32;

    #[instruction]
    #[pattern = "'mul_f64"]
    pub struct MulF64;

    #[instruction]
    #[pattern = "'div_i32"]
    pub struct DivI32;

    #[instruction]
    #[pattern = "'div_i64"]
    pub struct DivI64;

    #[instruction]
    #[pattern = "'div_u32"]
    pub struct DivU32;

    #[instruction]
    #[pattern = "'div_u64"]
    pub struct DivU64;

    #[instruction]
    #[pattern = "'div_f32"]
    pub struct DivF32;

    #[instruction]
    #[pattern = "'div_f64"]
    pub struct DivF64;

    #[instruction]
    #[pattern = "'rem_i32"]
    pub struct RemI32;

    #[instruction]
    #[pattern = "'rem_i64"]
    pub struct RemI64;

    #[instruction]
    #[pattern = "'rem_u32"]
    pub struct RemU32;

    #[instruction]
    #[pattern = "'rem_u64"]
    pub struct RemU64;

    #[instruction]
    #[pattern = "'rem_f32"]
    pub struct RemF32;

    #[instruction]
    #[pattern = "'rem_f64"]
    pub struct RemF64;

    #[instruction]
    #[pattern = "'neg_i32"]
    pub struct NegI32;

    #[instruction]
    #[pattern = "'neg_i64"]
    pub struct NegI64;

    #[instruction]
    #[pattern = "'neg_f32"]
    pub struct NegF32;

    #[instruction]
    #[pattern = "'neg_f64"]
    pub struct NegF64;

    #[instruction]
    #[pattern = "'shl_i32"]
    pub struct ShlI32;

    #[instruction]
    #[pattern = "'shl_i64"]
    pub struct ShlI64;

    #[instruction]
    #[pattern = "'shl_u32"]
    pub struct ShlU32;

    #[instruction]
    #[pattern = "'shl_u64"]
    pub struct ShlU64;

    #[instruction]
    #[pattern = "'shr_i32"]
    pub struct ShrI32;

    #[instruction]
    #[pattern = "'shr_i64"]
    pub struct ShrI64;

    #[instruction]
    #[pattern = "'shr_u32"]
    pub struct ShrU32;

    #[instruction]
    #[pattern = "'shr_u64"]
    pub struct ShrU64;

    #[instruction]
    #[pattern = "'rol_i32"]
    pub struct RolI32;

    #[instruction]
    #[pattern = "'rol_i64"]
    pub struct RolI64;

    #[instruction]
    #[pattern = "'rol_u32"]
    pub struct RolU32;

    #[instruction]
    #[pattern = "'rol_u64"]
    pub struct RolU64;

    #[instruction]
    #[pattern = "'ror_i32"]
    pub struct RorI32;

    #[instruction]
    #[pattern = "'ror_i64"]
    pub struct RorI64;

    #[instruction]
    #[pattern = "'ror_u32"]
    pub struct RorU32;

    #[instruction]
    #[pattern = "'ror_u64"]
    pub struct RorU64;

    #[instruction]
    #[pattern = "'bitand_i32"]
    pub struct BitAndI32;

    #[instruction]
    #[pattern = "'bitand_i64"]
    pub struct BitAndI64;

    #[instruction]
    #[pattern = "'bitand_u32"]
    pub struct BitAndU32;

    #[instruction]
    #[pattern = "'bitand_u64"]
    pub struct BitAndU64;

    #[instruction]
    #[pattern = "'bitor_i32"]
    pub struct BitOrI32;

    #[instruction]
    #[pattern = "'bitor_i64"]
    pub struct BitOrI64;

    #[instruction]
    #[pattern = "'bitor_u32"]
    pub struct BitOrU32;

    #[instruction]
    #[pattern = "'bitor_u64"]
    pub struct BitOrU64;

    #[instruction]
    #[pattern = "'bitxor_i32"]
    pub struct BitXorI32;

    #[instruction]
    #[pattern = "'bitxor_i64"]
    pub struct BitXorI64;

    #[instruction]
    #[pattern = "'bitxor_u32"]
    pub struct BitXorU32;

    #[instruction]
    #[pattern = "'bitxor_u64"]
    pub struct BitXorU64;

    #[instruction]
    #[pattern = "'not"]
    pub struct Not;

    #[instruction]
    #[pattern = "'and"]
    pub struct And;

    #[instruction]
    #[pattern = "'or"]
    pub struct Or;

    #[instruction]
    #[pattern = "'xor"]
    pub struct Xor;

    #[instruction]
    #[pattern = "'eq_i32"]
    pub struct EqI32;

    #[instruction]
    #[pattern = "'eq_i64"]
    pub struct EqI64;

    #[instruction]
    #[pattern = "'eq_u32"]
    pub struct EqU32;

    #[instruction]
    #[pattern = "'eq_u64"]
    pub struct EqU64;

    #[instruction]
    #[pattern = "'eq_f32"]
    pub struct EqF32;

    #[instruction]
    #[pattern = "'eq_f64"]
    pub struct EqF64;

    #[instruction]
    #[pattern = "'ne_i32"]
    pub struct NeI32;

    #[instruction]
    #[pattern = "'ne_i64"]
    pub struct NeI64;

    #[instruction]
    #[pattern = "'ne_u32"]
    pub struct NeU32;

    #[instruction]
    #[pattern = "'ne_u64"]
    pub struct NeU64;

    #[instruction]
    #[pattern = "'ne_f32"]
    pub struct NeF32;

    #[instruction]
    #[pattern = "'ne_f64"]
    pub struct NeF64;

    #[instruction]
    #[pattern = "'lt_i32"]
    pub struct LtI32;

    #[instruction]
    #[pattern = "'lt_i64"]
    pub struct LtI64;

    #[instruction]
    #[pattern = "'lt_u32"]
    pub struct LtU32;

    #[instruction]
    #[pattern = "'lt_u64"]
    pub struct LtU64;

    #[instruction]
    #[pattern = "'lt_f32"]
    pub struct LtF32;

    #[instruction]
    #[pattern = "'lt_f64"]
    pub struct LtF64;

    #[instruction]
    #[pattern = "'gt_i32"]
    pub struct GtI32;

    #[instruction]
    #[pattern = "'gt_i64"]
    pub struct GtI64;

    #[instruction]
    #[pattern = "'gt_u32"]
    pub struct GtU32;

    #[instruction]
    #[pattern = "'gt_u64"]
    pub struct GtU64;

    #[instruction]
    #[pattern = "'gt_f32"]
    pub struct GtF32;

    #[instruction]
    #[pattern = "'gt_f64"]
    pub struct GtF64;

    #[instruction]
    #[pattern = "'le_i32"]
    pub struct LeI32;

    #[instruction]
    #[pattern = "'le_i64"]
    pub struct LeI64;

    #[instruction]
    #[pattern = "'le_u32"]
    pub struct LeU32;

    #[instruction]
    #[pattern = "'le_u64"]
    pub struct LeU64;

    #[instruction]
    #[pattern = "'le_f32"]
    pub struct LeF32;

    #[instruction]
    #[pattern = "'le_f64"]
    pub struct LeF64;

    #[instruction]
    #[pattern = "'ge_i32"]
    pub struct GeI32;

    #[instruction]
    #[pattern = "'ge_i64"]
    pub struct GeI64;

    #[instruction]
    #[pattern = "'ge_u32"]
    pub struct GeU32;

    #[instruction]
    #[pattern = "'ge_u64"]
    pub struct GeU64;

    #[instruction]
    #[pattern = "'ge_f32"]
    pub struct GeF32;

    #[instruction]
    #[pattern = "'ge_f64"]
    pub struct GeF64;
}
