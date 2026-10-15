// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![allow(dead_code, unused_imports)] // Declaration-only expansion probes.

use chumsky::Parser as _;
use vihaco::{Execute, Parse as _, attributes::component, composite, dialect};

mod vendor_a {
    use super::*;
    dialect! { #[vihaco(inventory_id = probe_a)] cpu { Add(u32 => i64), } }
}

mod vendor_b {
    use super::*;
    dialect! { #[vihaco(inventory_id = probe_b)] cpu { Halt, } }
}

use vendor_a::cpu as a;
use vendor_b::cpu as b;

macro_rules! execute {
    ($device:ty, $leaf:ty) => {
        impl Execute<$leaf> for $device {
            type Message = ();
            type Effect = ();
            type Error = ();
            fn execute(&mut self, _: &$leaf, _: ()) -> Result<(), ()> {
                Ok(())
            }
        }
    };
}

// Wildcards emulate complete membership on the preserved partial-API draft.
#[component]
#[instructions { vendor_a::cpu::*, vendor_b::cpu::* }]
#[derive(Default)]
pub struct Direct;
execute!(Direct, a::Add);
execute!(Direct, b::Halt);

#[component]
#[instructions { a::* }]
struct OneAlias;
execute!(OneAlias, a::Add);

#[component]
#[instructions { a::*, b::* }]
struct TwoAliases;
execute!(TwoAliases, a::Add);
execute!(TwoAliases, b::Halt);

macro_rules! capture {
    (
        state: { $name:ident },
        inventory: { $(entry {
            selector: { $selector:path },
            type_route: { $route:path },
            source_steps: { $($steps:tt)* },
        },)* },
    ) => {
        const $name: &[(&str, &str, &str)] = &[
            $((stringify!($selector), stringify!($route), stringify!($($steps)*)),)*
        ];
    };
}

__vihaco_inventory_two_aliases::__entries! { callback: capture, state: { ALIASED }, }

#[component]
#[instructions { b::*, a::* }]
struct Reversed;
execute!(Reversed, a::Add);
execute!(Reversed, b::Halt);
__vihaco_inventory_reversed::__entries! { callback: capture, state: { REVERSED }, }

#[composite]
struct Machine {
    #[device(1, alias = "d")]
    field: Direct,
    other: (),
}

#[vihaco::instruction_set(composite = Machine)]
pub enum ChosenInstruction {
    Add(field::cpu::Add),
}

// Candidate: the inventory supplies canonical names; first-expansion provider
// mounts remain untouched. A finalizer would group these leaf re-exports into
// canonical root modules. This two-provider callback isolates that operation.
macro_rules! merge_selectors {
    (
        state: { $namespace:ident, $root:ident },
        inventory: {
            entry { selector: { cpu::Add }, type_route: { $($add:ident)::* }, source_steps: {}, },
            entry { selector: { cpu::Halt }, type_route: { $($halt:ident)::* }, source_steps: {}, },
        },
    ) => {
        pub mod $namespace {
            pub mod $root {
                pub use crate::__vihaco_inventory_two_aliases::__types::runtime::$( $add )::*;
                pub use crate::__vihaco_inventory_two_aliases::__types::runtime::$( $halt )::*;
            }
        }
    };
}
__vihaco_inventory_two_aliases::__entries! {
    callback: merge_selectors, state: { canonical, cpu },
}

// Mimic immediate composite mounts and selector-shaped enum payloads after the
// staged merge; the runtime declaration still stays at the authored scope.
#[composite]
struct AliasMachine {
    #[device(1)]
    field: TwoAliases,
}

mod staged {
    pub mod __selectors {
        pub use crate::canonical as field;
    }
    pub mod __types {
        pub mod runtime {
            pub use crate::__vihaco_inventory_two_aliases::__types::runtime as field;
        }
        pub mod surface {
            pub use crate::__vihaco_inventory_two_aliases::__types::surface as field;
        }
    }
    pub use crate::__vihaco_inventory_alias_machine::__entries;
}
#[vihaco::instruction_set(composite = AliasMachine, inventory = staged)]
enum StagedInstruction {
    Halt(field::cpu::Halt),
}

#[test]
fn complete_membership_preserves_list_order_and_canonical_names() {
    assert_eq!(ALIASED[0].0, "cpu :: Add");
    assert_eq!(ALIASED[0].1, "provider_0 :: cpu :: Add");
    assert_eq!(ALIASED[1].0, "cpu :: Halt");
    assert!(ALIASED.iter().all(|entry| entry.2.is_empty()));
    assert_eq!(REVERSED[0].0, "cpu :: Halt");
    assert_eq!(REVERSED[1].0, "cpu :: Add");
}

#[test]
fn provenance_mounts_and_one_import_alias_keep_concrete_types() {
    let _: Option<__vihaco_inventory_two_aliases::__types::runtime::provider_0::cpu::Add> =
        None::<a::Add>;
    let _: Option<__vihaco_inventory_two_aliases::__types::surface::provider_1::cpu::Halt> =
        None::<b::syntax::Halt>;
    let _: Option<__vihaco_inventory_one_alias::__selectors::cpu::Add> = None::<a::Add>;
    let _: Option<__vihaco_inventory_direct::__selectors::cpu::Halt> = None::<b::Halt>;
    let _ = Direct::default();
}

#[test]
fn inventory_driven_canonical_merge_resolves_both_aliases() {
    let _: Option<canonical::cpu::Add> = None::<a::Add>;
    let _: Option<canonical::cpu::Halt> = None::<b::Halt>;
    let instruction = StagedInstruction::Halt(b::Halt);
    assert!(matches!(instruction, StagedInstruction::Halt(_)));
    assert!(matches!(
        StagedSurfaceInstruction::parser()
            .parse("field::cpu.halt")
            .into_result()
            .unwrap(),
        StagedSurfaceInstruction::Halt(b::syntax::Halt)
    ));
}

#[test]
fn selected_parser_uses_surface_operands_and_omits_available_halt() {
    assert!(matches!(
        ChosenSurfaceInstruction::parser()
            .parse("d::cpu.add 4")
            .into_result()
            .unwrap(),
        ChosenSurfaceInstruction::Add(a::syntax::Add(4))
    ));
    assert!(
        ChosenSurfaceInstruction::parser()
            .parse("field::cpu.halt")
            .has_errors()
    );
    let instruction = ChosenInstruction::Add(a::Add(-4));
    assert!(matches!(instruction, ChosenInstruction::Add(a::Add(-4))));
}

// Characterize the draft's last-pattern-wins bug; Step 5 must reject this.
dialect! { duplicate_pattern {
    #[pattern = "'first"]
    #[pattern = "'second"]
    Halt,
} }

#[test]
fn reference_duplicate_patterns_use_the_last_attribute() {
    assert!(
        duplicate_pattern::syntax::Halt::parser()
            .parse("duplicate_pattern.second")
            .into_result()
            .is_ok()
    );
    assert!(
        duplicate_pattern::syntax::Halt::parser()
            .parse("duplicate_pattern.first")
            .has_errors()
    );
}

// Item conditions placed before a macro suppress the entire expansion.
#[cfg(any())]
#[component]
#[instructions { missing::* }]
struct DisabledBefore;

#[cfg_attr(all(), component)]
#[instructions { a::* }]
#[vihaco(inventory_id = cfg_attr_device)]
#[derive(Default)]
struct AttrDevice;
execute!(AttrDevice, a::Add);

#[test]
fn ordinary_attributes_survive_enabled_cfg_attr() {
    let _ = AttrDevice::default();
}

#[component]
#[cfg(any())]
#[instructions { missing::* }]
struct DisabledAfter;

dialect! { r#type { Halt, } }

#[test]
fn raw_identifiers_resolve_and_characterize_source_spelling() {
    let _: Option<r#type::__private::__types::runtime::r#type::Halt> = None::<r#type::Halt>;
    assert!(
        r#type::syntax::Halt::parser()
            .parse("r#type.halt")
            .into_result()
            .is_ok()
    );
}

// Second candidate: a dialect mount helper emits concrete re-exports under its
// declared root, independently of inventory-entry expansion. This deliberately
// models only the output shape; generalized root grouping belongs to Step 6.
macro_rules! mount_cpu {
    ($namespace:ident) => {
        pub mod $namespace {
            pub mod cpu {
                pub use crate::a::Add;
                pub use crate::b::Halt;
            }
        }
    };
}
mount_cpu!(helper);
#[composite]
struct HelperMachine {
    #[device(1)]
    field: TwoAliases,
}
mod helper_inventory {
    pub mod __selectors {
        pub use crate::helper as field;
    }
    pub use crate::__vihaco_inventory_helper_machine::__entries;
    pub use crate::staged::__types;
}
#[vihaco::instruction_set(composite = HelperMachine, inventory = helper_inventory)]
enum HelperInstruction {
    Stop(field::cpu::Halt),
}
#[test]
fn mount_helper_shape_preserves_types_and_parsing() {
    assert!(matches!(
        HelperInstruction::Stop(b::Halt),
        HelperInstruction::Stop(_)
    ));
    assert!(matches!(
        HelperSurfaceInstruction::parser()
            .parse("field::cpu.halt")
            .into_result()
            .unwrap(),
        HelperSurfaceInstruction::Stop(b::syntax::Halt)
    ));
}
