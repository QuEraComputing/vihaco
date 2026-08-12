// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::component;
use vihaco_parser::{BareToken, QuotedString};

pub struct ParentContext;

component! {
    component UsesParentContext {
        context: ParentContext,
    }

    runtime {
        instruction {
            ParentProduct { context: ParentContext },
        }
    }
}

component! {
    component RuntimeBlockComponent {}

    runtime {
        type Type = vihaco::Type;
        value Value = vihaco::Value;

        instruction {
            Add(Type, Value),
            Halt,
        }
    }

    syntax {
        type SurfaceType {
            U32 = "`u32`";
        }

        value SurfaceValue {
            Bare(BareToken),
        }

        instruction {
            Second(SurfaceValue) = "'second $0";
        }
    }
}

component! {
    component ComponentWithoutInstructions {}
}

component! {
    component GenericComponent<T, const N: usize>
    where
        T: Clone,
    {
        _value: T,
    }

    runtime {
        instruction {
            Unit,
            Tuple(T),
            Named { value: T },
            Array([T; N]),
        }
    }
}

component! {
    component SyntaxComponent {
        state: u8,
    }

    runtime {
        instruction {
            Runtime(u32),
        }
    }

    syntax {
        type SyntaxType {
            U32 = "`u32`";
        }

        value SyntaxValue {
            Quoted(QuotedString),
            Bare(BareToken),
        }

        instruction {
            Runtime(SyntaxValue) = "'runtime $0";
        }
    }
}

#[test]
fn components_without_instructions_still_generate_the_component_module() {
    let _: component_without_instructions::ComponentWithoutInstructions =
        component_without_instructions::ComponentWithoutInstructions {};
}

#[test]
fn runtime_instruction_products_use_runtime_aliases_and_coexist_with_syntax() {
    let _: runtime_block_component::runtime::instruction::Add =
        runtime_block_component::runtime::instruction::Add(
            vihaco::Type::I64,
            vihaco::Value::I64(1),
        );
    let _: runtime_block_component::runtime::instruction::Halt =
        runtime_block_component::runtime::instruction::Halt;
    let _: runtime_block_component::syntax::Instruction =
        runtime_block_component::syntax::Instruction::Second(
            runtime_block_component::syntax::SurfaceValue::Bare(vihaco_parser::BareToken(
                "second".to_owned(),
            )),
        );
}

#[test]
fn generated_modules_can_use_names_from_the_parent_module() {
    let _: uses_parent_context::UsesParentContext = uses_parent_context::UsesParentContext {
        context: ParentContext,
    };
    let _: uses_parent_context::runtime::instruction::ParentProduct =
        uses_parent_context::runtime::instruction::ParentProduct {
            context: ParentContext,
        };
}

#[test]
fn generated_products_support_all_field_forms() {
    let _: generic_component::runtime::instruction::Unit =
        generic_component::runtime::instruction::Unit;
    let _: generic_component::runtime::instruction::Tuple<u8> =
        generic_component::runtime::instruction::Tuple(1);
    let _: generic_component::runtime::instruction::Named<u8> =
        generic_component::runtime::instruction::Named { value: 1 };
    let _: generic_component::runtime::instruction::Array<u8, 2> =
        generic_component::runtime::instruction::Array([1, 2]);
    let _: core::marker::PhantomData<generic_component::GenericComponent<u8, 2>> =
        core::marker::PhantomData;
}

#[test]
fn syntax_declarations_support_payloads_and_derived_value_patterns() {
    use chumsky::Parser as _;
    use vihaco::{InstructionSet, Parse};

    let quoted = syntax_component::syntax::SyntaxValue::parser()
        .parse("\"hello\"")
        .into_result()
        .unwrap();
    assert_eq!(
        quoted,
        syntax_component::syntax::SyntaxValue::Quoted(QuotedString("hello".to_owned(),))
    );

    let bare = syntax_component::syntax::SyntaxValue::parser()
        .parse("token")
        .into_result()
        .unwrap();
    assert_eq!(
        bare,
        syntax_component::syntax::SyntaxValue::Bare(BareToken("token".to_owned(),))
    );

    let instruction = syntax_component::syntax::Instruction::parser()
        .parse("runtime token")
        .into_result()
        .unwrap();
    assert_eq!(
        instruction,
        syntax_component::syntax::Instruction::Runtime(
            syntax_component::syntax::SyntaxValue::Bare(BareToken("token".to_owned())),
        )
    );

    let _: <syntax_component::SyntaxComponent as InstructionSet>::Type =
        syntax_component::syntax::SyntaxType::U32;
}

#[test]
fn runtime_blocks_generate_aliases_alongside_syntax() {
    let _: runtime_block_component::runtime::Type = vihaco::Type::I64;
    let _: runtime_block_component::runtime::Value = vihaco::Value::I64(7);
    let _: runtime_block_component::syntax::Instruction =
        runtime_block_component::syntax::Instruction::Second(
            runtime_block_component::syntax::SurfaceValue::Bare(BareToken("value".to_owned())),
        );
}
