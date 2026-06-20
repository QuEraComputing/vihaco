// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::instruction::Instruction;
use vihaco::color::{Themed, show_instruction};

impl std::fmt::Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Instruction::*;
        match self {
            Span(file, start, end) => {
                show_instruction!(
                    f,
                    "span ",
                    file,
                    " ",
                    format!("0x{:X}", start),
                    " ",
                    format!("0x{:X}", end)
                )
            }
            Label => show_instruction!(f, "label"),
            FunctionStart => show_instruction!(f, "function_start"),
            FunctionEnd => show_instruction!(f, "function_end"),
            Breakpoint => show_instruction!(f, "breakpoint"),
            Branch(target) => {
                show_instruction!(f, "br ", format!("0x{:X}", target))
            }
            ConditionalBranch(true_target, false_target) => {
                show_instruction!(
                    f,
                    "br_if ",
                    format!("0x{:X}", true_target),
                    " ",
                    format!("0x{:X}", false_target)
                )
            }
            Return(keep) => show_instruction!(f, "ret ", keep),
            Call(arity, target) => {
                show_instruction!(f, "call ", arity, " ", format!("0x{:X}", target))
            }
            IndirectCall => show_instruction!(f, "indirect_call"),
            Halt => show_instruction!(f, "halt"),
            Print => show_instruction!(f, "print"),
            Load(ty, addr) => {
                show_instruction!(f, "load ", ty, " ", format!("0x{:X}", addr))
            }
            Store(ty, addr) => {
                show_instruction!(f, "store ", ty, " ", format!("0x{:X}", addr))
            }
            Dup => show_instruction!(f, "dup"),
            HeapAlloc(n_elements) => show_instruction!(f, "heap_alloc ", n_elements),
            GetItem => show_instruction!(f, "get_item"),
            HeapDealloc => show_instruction!(f, "heap_dealloc"),
            Const(v) => show_instruction!(f, "const.", v.type_of(), " ", v),
            Add(ty) => show_instruction!(f, "add.", ty),
            Sub(ty) => show_instruction!(f, "sub.", ty),
            Mul(ty) => show_instruction!(f, "mul.", ty),
            Div(ty) => show_instruction!(f, "div.", ty),
            Rem(ty) => show_instruction!(f, "rem.", ty),
            Neg(ty) => show_instruction!(f, "neg.", ty),
            Shl(ty) => show_instruction!(f, "shl.", ty),
            Shr(ty) => show_instruction!(f, "shr.", ty),
            Rol(ty) => show_instruction!(f, "rol.", ty),
            Ror(ty) => show_instruction!(f, "ror.", ty),
            BitAnd(ty) => show_instruction!(f, "and.", ty),
            BitOr(ty) => show_instruction!(f, "or.", ty),
            BitXor(ty) => show_instruction!(f, "xor.", ty),

            Not => show_instruction!(f, "not"),
            And => show_instruction!(f, "and"),
            Or => show_instruction!(f, "or"),
            Xor => show_instruction!(f, "xor"),

            Eq(ty) => show_instruction!(f, "eq.", ty),
            Ne(ty) => show_instruction!(f, "ne.", ty),
            Lt(ty) => show_instruction!(f, "lt.", ty),
            Gt(ty) => show_instruction!(f, "gt.", ty),
            Le(ty) => show_instruction!(f, "le.", ty),
            Ge(ty) => show_instruction!(f, "ge.", ty),
        }
        Ok(())
    }
}
