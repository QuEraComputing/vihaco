#[allow(dead_code)]
trait Instruction {}

macro_rules! dialect {
    ($($t:tt)*) => {};
}

dialect! {
    arith [
        Add,
        Sub,
        Mul,
    ]
}

mod arith {
    pub struct Add;
    impl super::Instruction for Add {}
    pub struct Sub;
    impl super::Instruction for Sub {}
    pub struct Mul;
    impl super::Instruction for Mul {}

    #[doc(hidden)]
    #[macro_export]
    macro_rules! __vihaco_arith_instructions {
        // The field-aware form forwards the instruction names and types to
        // the composite callback. The callback is responsible for qualifying
        // the names with the component field.
        ($callback:ident, $field:ident, [$($variants:tt)*] ; $($dialect:ident),*) => {
            $callback! {
                [
                    $($variants)*
                ]
                ; field: $field
                ; instructions: [
                    (Add, $crate::test::arith::Add),
                    (Sub, $crate::test::arith::Sub),
                    (Mul, $crate::test::arith::Mul),
                ]
                ; $($dialect),*
            }
        };

        // Existing instruction-variant form used by the conceptual example
        // below.
        ($callback:ident, [$($variants:tt)*] ; $($dialect:ident),*) => {
            $callback! {
                [
                    $($variants)*
                    ArithAdd($crate::test::arith::Add),
                    ArithSub($crate::test::arith::Sub),
                    ArithMul($crate::test::arith::Mul),
                ]
                ; $($dialect),*
            }
        };
    }
}

dialect! {
    logic {
        And,
        Not,
    }
}

mod logic {
    pub struct And;
    impl super::Instruction for And {}
    pub struct Not;
    impl super::Instruction for Not {}

    #[doc(hidden)]
    #[macro_export]
    macro_rules! __vihaco_logic_instructions {
        // The field-aware form forwards the instruction names and types to
        // the composite callback.
        ($callback:ident, $field:ident, [$($variants:tt)*] ; $($dialect:ident),*) => {
            $callback! {
                [
                    $($variants)*
                ]
                ; field: $field
                ; instructions: [
                    (And, $crate::test::logic::And),
                    (Not, $crate::test::logic::Not),
                ]
                ; $($dialect),*
            }
        };

        ($callback:ident, [$($variants:tt)*] ; $($dialect:ident),*) => {
            $callback! {
                [
                    $($variants)*
                    LogicAnd($crate::test::logic::And),
                    LogicNot($crate::test::logic::Not),
                ]
                ; $($dialect),*
            }
        };
    }
}

mod conceptual_composite {
    use crate::{__vihaco_arith_instructions, __vihaco_logic_instructions};

    macro_rules! collect_a_arith_variants {
        ([$($variants:tt)*]
            ; field: $field:ident
            ; instructions: [
                (Add, $add_type:path),
                (Sub, $sub_type:path),
                (Mul, $mul_type:path),
                $(,)?
            ]
            ; $($dialect:ident),*
        ) => {
            __vihaco_arith_instructions!(
                collect_b_arith_variants,
                b,
                [
                    $($variants)*
                    AArithAdd($add_type),
                    AArithSub($sub_type),
                    AArithMul($mul_type),
                ]
                ;
            );
        };
    }

    macro_rules! collect_b_arith_variants {
        ([$($variants:tt)*]
            ; field: $field:ident
            ; instructions: [
                (Add, $add_type:path),
                (Sub, $sub_type:path),
                (Mul, $mul_type:path),
                $(,)?
            ]
            ; $($dialect:ident),*
        ) => {
            #[allow(dead_code)]
            enum FlattenedInstruction {
                $($variants)*
                BArithAdd($add_type),
                BArithSub($sub_type),
                BArithMul($mul_type),
            }
        };
    }

    // The same dialect is used by both fields. Each invocation supplies a
    // different field name, so the collector can qualify the variants before
    // emitting one flattened enum.
    __vihaco_arith_instructions!(collect_a_arith_variants, a, [];);

    macro_rules! __collect_instruction_variants {
        ([$($variants:tt)*] ; ) => {
            #[allow(dead_code)]
            pub enum Instruction {
                $($variants)*
            }
        };

        ([$($variants:tt)*] ; $dialect:ident $(, $rest:ident)*) => {
            $dialect!(
                __collect_instruction_variants,
                [$($variants)*] ;
                $($rest),*
            );
        };
    }

    macro_rules! make_instruction_enum {
        ($($dialect:ident),+ $(,)?) => {
            __collect_instruction_variants! {
                [] ; $($dialect),+
            }
        };
    }

    // This is the conceptual composite expansion: both dialects contribute
    // variants to one instruction enum.
    make_instruction_enum!(__vihaco_arith_instructions, __vihaco_logic_instructions,);
}

#[derive(Default, Debug)]
#[allow(clippy::upper_case_acronyms, dead_code)]
// #[component(dialect = arith)]
pub struct CPU {
    x: u32,
}

impl vihaco::HasInstructionSet for CPU {
    type Syntax = u32;
    type Runtime = (arith::Add, arith::Sub, arith::Mul);
}

impl vihaco::Component for CPU {}

#[allow(dead_code)]
trait Execute<T>
where
    Self: vihaco::Component,
    T: Instruction,
{
    type Message;
    type Effect;

    fn execute(&mut self, instruction: &T, message: Self::Message) -> Self::Effect;
}

struct A(CPU);
struct B(CPU);

impl Execute<arith::Add> for CPU {
    type Message = (u32, u32);
    type Effect = ();

    fn execute(&mut self, _instruction: &arith::Add, _message: Self::Message) -> Self::Effect {}
}

impl Execute<arith::Sub> for CPU {
    type Message = (u32, u32);
    type Effect = ();

    fn execute(&mut self, _instruction: &arith::Sub, _message: Self::Message) -> Self::Effect {}
}

impl Execute<arith::Mul> for CPU {
    type Message = (u32, u32);
    type Effect = ();

    fn execute(&mut self, _instruction: &arith::Mul, _message: Self::Message) -> Self::Effect {}
}

struct Composite {
    a: CPU,
    b: CPU,
}


mod __v_isa_check_cpu {
    fn __v_isa_assert_implements_add<T: super::Execute<super::arith::Add>>() {}
    fn __v_isa_assert_implements_sub<T: super::Execute<super::arith::Sub>>() {}
    fn __v_isa_assert_implements_mul<T: super::Execute<super::arith::Mul>>() {}

    #[allow(non_snake_case)]
    fn __v__isa_check_cpu() {
        __v_isa_assert_implements_add::<super::CPU>();
        __v_isa_assert_implements_sub::<super::CPU>();
        __v_isa_assert_implements_mul::<super::CPU>();
    }
}
