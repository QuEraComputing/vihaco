// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::component::{
    Add, And, BitAnd, BitOr, BitXor, Branch, Breakpoint, Call, ConditionalBranch, Const, Div, Dup,
    Eq, FunctionEnd, FunctionStart, Ge, GetItem, Gt, Halt, HeapAlloc, HeapDealloc, IndirectCall,
    Label, Le, Load, Lt, Mul, Ne, Neg, Not, Or, Print, Rem, Return, Rol, Ror, Shl, Shr, Span,
    Store, Sub, Xor,
};
use vihaco::color::{Themed, show_instruction};

macro_rules! display_unit {
    ($type:ty, |$f:ident| $body:expr) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, $f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                $body;
                Ok(())
            }
        }
    };
}

macro_rules! display_one {
    ($type:ty, |$f:ident, $value:ident| $body:expr) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, $f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let $value = self.0;
                $body;
                Ok(())
            }
        }
    };
}

macro_rules! display_two {
    ($type:ty, |$f:ident, $first:ident, $second:ident| $body:expr) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, $f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let $first = self.0;
                let $second = self.1;
                $body;
                Ok(())
            }
        }
    };
}

macro_rules! display_three {
    ($type:ty, |$f:ident, $first:ident, $second:ident, $third:ident| $body:expr) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, $f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let $first = self.0;
                let $second = self.1;
                let $third = self.2;
                $body;
                Ok(())
            }
        }
    };
}

display_three!(Span, |f, file, start, end| {
    show_instruction!(
        f,
        "span ",
        file,
        " ",
        format!("0x{:X}", start),
        " ",
        format!("0x{:X}", end)
    )
});
display_unit!(Label, |f| show_instruction!(f, "label"));
display_unit!(FunctionStart, |f| show_instruction!(f, "function_start"));
display_unit!(FunctionEnd, |f| show_instruction!(f, "function_end"));
display_unit!(Breakpoint, |f| show_instruction!(f, "breakpoint"));
display_one!(Branch, |f, target| show_instruction!(
    f,
    "br ",
    format!("0x{:X}", target)
));
display_two!(ConditionalBranch, |f, true_target, false_target| {
    show_instruction!(
        f,
        "br_if ",
        format!("0x{:X}", true_target),
        " ",
        format!("0x{:X}", false_target)
    )
});
display_one!(Return, |f, keep| show_instruction!(f, "ret ", keep));
display_two!(Call, |f, arity, target| {
    show_instruction!(f, "call ", arity, " ", format!("0x{:X}", target))
});
display_unit!(IndirectCall, |f| show_instruction!(f, "indirect_call"));
display_unit!(Halt, |f| show_instruction!(f, "halt"));
display_unit!(Print, |f| show_instruction!(f, "print"));
display_two!(Load, |f, ty, addr| {
    show_instruction!(f, "load ", ty, " ", format!("0x{:X}", addr))
});
display_two!(Store, |f, ty, addr| {
    show_instruction!(f, "store ", ty, " ", format!("0x{:X}", addr))
});
display_unit!(Dup, |f| show_instruction!(f, "dup"));
display_one!(HeapAlloc, |f, n| show_instruction!(f, "heap_alloc ", n));
display_unit!(GetItem, |f| show_instruction!(f, "get_item"));
display_unit!(HeapDealloc, |f| show_instruction!(f, "heap_dealloc"));
display_one!(Const, |f, value| show_instruction!(
    f,
    "const.",
    value.type_of(),
    " ",
    value
));
display_one!(Add, |f, ty| show_instruction!(f, "add.", ty));
display_one!(Sub, |f, ty| show_instruction!(f, "sub.", ty));
display_one!(Mul, |f, ty| show_instruction!(f, "mul.", ty));
display_one!(Div, |f, ty| show_instruction!(f, "div.", ty));
display_one!(Rem, |f, ty| show_instruction!(f, "rem.", ty));
display_one!(Neg, |f, ty| show_instruction!(f, "neg.", ty));
display_one!(Shl, |f, ty| show_instruction!(f, "shl.", ty));
display_one!(Shr, |f, ty| show_instruction!(f, "shr.", ty));
display_one!(Rol, |f, ty| show_instruction!(f, "rol.", ty));
display_one!(Ror, |f, ty| show_instruction!(f, "ror.", ty));
display_one!(BitAnd, |f, ty| show_instruction!(f, "and.", ty));
display_one!(BitOr, |f, ty| show_instruction!(f, "or.", ty));
display_one!(BitXor, |f, ty| show_instruction!(f, "xor.", ty));
display_unit!(Not, |f| show_instruction!(f, "not"));
display_unit!(And, |f| show_instruction!(f, "and"));
display_unit!(Or, |f| show_instruction!(f, "or"));
display_unit!(Xor, |f| show_instruction!(f, "xor"));
display_one!(Eq, |f, ty| show_instruction!(f, "eq.", ty));
display_one!(Ne, |f, ty| show_instruction!(f, "ne.", ty));
display_one!(Lt, |f, ty| show_instruction!(f, "lt.", ty));
display_one!(Gt, |f, ty| show_instruction!(f, "gt.", ty));
display_one!(Le, |f, ty| show_instruction!(f, "le.", ty));
display_one!(Ge, |f, ty| show_instruction!(f, "ge.", ty));
