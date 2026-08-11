// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::component;
use vihaco_parser::{BareToken, QuotedString};

pub struct ParentContext;

component! {
    component UsesParentContext {
        context: ParentContext,
    }

    instruction {
        ParentProduct { context: ParentContext },
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

    instruction {
        Unit,
        Tuple(T),
        Named { value: T },
        Array([T; N]),
    }
}

component! {
    component SyntaxComponent {
        state: u8,
    }

    instruction {
        Runtime(u32),
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
fn generated_modules_can_use_names_from_the_parent_module() {
    let _: uses_parent_context::UsesParentContext = uses_parent_context::UsesParentContext {
        context: ParentContext,
    };
    let _: uses_parent_context::instruction::ParentProduct =
        uses_parent_context::instruction::ParentProduct {
            context: ParentContext,
        };
}

#[test]
fn generated_products_support_all_field_forms() {
    let _: generic_component::instruction::Unit = generic_component::instruction::Unit;
    let _: generic_component::instruction::Tuple<u8> = generic_component::instruction::Tuple(1);
    let _: generic_component::instruction::Named<u8> =
        generic_component::instruction::Named { value: 1 };
    let _: generic_component::instruction::Array<u8, 2> =
        generic_component::instruction::Array([1, 2]);
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
