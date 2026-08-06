// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::component;

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
