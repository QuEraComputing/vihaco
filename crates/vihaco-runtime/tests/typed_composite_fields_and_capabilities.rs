// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]
#![allow(dead_code, non_snake_case)]
use framework::__private::{Capabilities, CompositeSpec, capabilities};
use framework::{Execute, Field, HasInstructionSet, component, dialect};
use framework::{composite, field};
use vihaco_runtime as framework;

#[dialect]
mod control {
    #[instruction]
    pub struct Halt;
    #[instruction]
    pub struct Extra;
}
#[component]
#[instructions { control }]
struct Cpu<'a, const N: usize = 2>(&'a [u8; N]);
impl<const N: usize> Execute<control::Halt> for Cpu<'_, N> {
    type Message = ();
    type Effect = ();
    type Error = std::convert::Infallible;
    fn execute(&mut self, _: &control::Halt, _: ()) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl<const N: usize> Execute<control::Extra> for Cpu<'_, N> {
    type Message = ();
    type Effect = ();
    type Error = std::convert::Infallible;
    fn execute(&mut self, _: &control::Extra, _: ()) -> Result<(), Self::Error> {
        Ok(())
    }
}
// Executable selection is deliberately narrower than complete capabilities.
impl<const N: usize> HasInstructionSet for Cpu<'_, N> {
    type Runtime = control::Halt;
    type Syntax = control::syntax::Halt;
}
use Cpu as Imported;
type Alias<'a, const N: usize = 2> = Imported<'a, N>;
// Reexport companion namespaces alongside the type's alternate spellings.
pub use __vihaco_fields_Cpu as __vihaco_fields_Alias;
pub use __vihaco_fields_Cpu as __vihaco_fields_Imported;
trait ChildType {
    type Child: Capabilities;
}
struct Choice;
impl ChildType for Choice {
    type Child = Cpu<'static>;
}

#[composite]
#[vihaco(crate = framework)]
#[derive(Default)]
struct Empty {}
#[composite]
#[vihaco(crate = framework)]
struct State {
    count: u8,
}
#[composite]
#[vihaco(crate = framework)]
struct Inner<'a, const N: usize = 2> {
    state: u8,
    #[cfg_attr(all(), allow(dead_code))]
    #[device(alias = "core", alias = "core-v2")]
    r#type: Alias<'a, N>,
    #[device(7, alias = "other")]
    MiXeD: self::Cpu<'a, N>,
    #[device(alias = "unicode")]
    Δ: Imported<'a, N>,
}
#[composite]
#[vihaco(crate = framework)]
struct Outer<'a, T: ChildType = Choice, const N: usize = 2>
where
    T::Child: Capabilities,
{
    #[device(alias = "ctl")]
    left: Inner<'a, N>,
    #[device(alias = "core", alias = "core-v2")]
    right: <T as ChildType>::Child,
    // No capability or instruction-set contract on a section boundary.
    #[device(42, alias = "program")]
    #[loadable("code")]
    image: Image,
}
#[composite]
#[vihaco(crate = framework)]
struct LoadOnly {
    #[device]
    #[loadable]
    image: Image,
}
#[composite]
#[vihaco(crate = framework)]
struct Generic<T: Capabilities> {
    #[device]
    child: T,
}
#[composite]
#[vihaco(crate = framework)]
struct HelperNames<'__vihaco_bc, __VihacoContext> {
    state: &'__vihaco_bc __VihacoContext,
}
#[composite]
#[vihaco(crate = framework)]
#[cfg(any())]
struct Disabled {
    #[device]
    unavailable: Missing,
}
#[composite]
#[vihaco(crate = framework)]
#[cfg_attr(all(), cfg(any()), derive(Clone))]
struct AlsoDisabled {
    #[device]
    unavailable: Missing,
}

#[derive(Default)]
struct Image {
    calls: Vec<String>,
}
impl framework::loader::LoadBytecodeSection<()> for Image {
    fn load_bytecode_section<'bc>(
        &mut self,
        section: framework::BytecodeSectionView<'bc, ()>,
    ) -> eyre::Result<()> {
        self.calls.push(section.display_path().to_string());
        Ok(())
    }
}
impl framework::loader::LoadSstSection<()> for Image {
    fn load_sst_section<'bc>(
        &mut self,
        section: framework::SstSectionView<'bc, ()>,
    ) -> eyre::Result<()> {
        self.calls.push(section.display_path().to_string());
        Ok(())
    }
}
impl framework::loader::LoadOwnBytecodeSection<()> for LoadOnly {
    fn load_own_bytecode_section<'bc>(
        &mut self,
        _: framework::BytecodeSectionView<'bc, ()>,
    ) -> eyre::Result<()> {
        Ok(())
    }
}
impl framework::loader::LoadOwnSstSection<()> for LoadOnly {
    fn load_own_sst_section<'bc>(
        &mut self,
        _: framework::SstSectionView<'bc, ()>,
    ) -> eyre::Result<()> {
        Ok(())
    }
}
fn exact_types<'a, const N: usize>(_: &'a [u8; N]) {
    fn same<T>(_: std::marker::PhantomData<T>, _: std::marker::PhantomData<T>) {}
    same(
        std::marker::PhantomData::<<Inner<'a, N> as Field<field!(Inner<'a, N>::r#type)>>::Child>,
        std::marker::PhantomData::<Cpu<'a, N>>,
    );
    same(
        std::marker::PhantomData::<
            <<Outer<'a, Choice, N> as Field<field!(Outer<'a, Choice, N>::left)>>::Child as Field<
                field!(Inner<'a, N>::MiXeD),
            >>::Child,
        >,
        std::marker::PhantomData::<Cpu<'a, N>>,
    );
    same(
        std::marker::PhantomData::<<Cpu<'a, N> as HasInstructionSet>::Runtime>,
        std::marker::PhantomData::<control::Halt>,
    );
}
#[test]
fn typed_composite_fields_and_capabilities() {
    exact_types(&[0; 2]);
    assert_eq!(
        <Inner<'static> as Field<field!(Inner<'static>::r#type)>>::SOURCE,
        ["type", "core", "core-v2"]
    );
    assert_eq!(
        <Inner<'static> as Field<field!(Inner<'static>::MiXeD)>>::SOURCE,
        ["MiXeD", "other"]
    );
    assert!(capabilities::<Empty>().next().is_none());
    assert!(capabilities::<State>().next().is_none());
    assert!(capabilities::<LoadOnly>().next().is_none());
    let rows: Vec<_> = capabilities::<Outer<'static>>().collect();
    assert_eq!(rows.len(), 8);
    assert_eq!(capabilities::<Generic<Cpu<'static>>>().count(), 2);
    let devices = <Outer<'static> as CompositeSpec>::DEVICES;
    assert_eq!(devices.len(), 3);
    assert_eq!(devices[0].code, None);
    assert_eq!(devices[0].section, None);
    assert_eq!(devices[2].name, "image");
    assert_eq!(devices[2].code, Some(42));
    assert_eq!(devices[2].source, ["image", "program"]);
    assert_eq!(devices[2].section, Some("code"));
    assert_eq!(<Inner<'static> as CompositeSpec>::DEVICES[1].code, Some(7));
    assert!(<State as CompositeSpec>::DEVICES.is_empty());
    let expected = [
        vec!["left", "type"],
        vec!["left", "MiXeD"],
        vec!["left", "Δ"],
        vec!["right"],
    ];
    for (pair, target) in rows.as_chunks::<2>().0.iter().zip(expected) {
        assert_eq!(pair[0].target, target);
        assert_eq!(pair[1].target, target);
        assert_eq!(pair[0].dialect, "control");
        assert_eq!(pair[0].instruction, "halt");
        assert_eq!(pair[1].instruction, "extra");
    }
    fn section_contracts<
        T: framework::loader::LoadBytecodeSection<()> + framework::loader::LoadSstSection<()>,
    >() {
    }
    section_contracts::<LoadOnly>();
    #[composite]
    #[vihaco(crate = framework)]
    struct Local {
        #[device]
        cpu: Cpu<'static>,
    }
    assert_eq!(capabilities::<Local>().count(), 2);
}

impl<const N: usize> Inner<'_, N> {
    #[allow(non_upper_case_globals)]
    const __vihaco_check_capabilities: usize = 99;
}

#[test]
fn typed_composites_reject_invalid_targets_and_capabilities() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/composites/*.rs");
}

#[test]
fn markers_are_zero_sized_and_distinct_by_owner_and_field() {
    use std::any::TypeId;
    assert_eq!(std::mem::size_of::<field!(Inner<'static>::r#type)>(), 0);
    assert_ne!(
        TypeId::of::<field!(Inner<'static>::r#type)>(),
        TypeId::of::<field!(Inner<'static>::MiXeD)>()
    );
    assert_ne!(
        TypeId::of::<field!(Inner<'static>::MiXeD)>(),
        TypeId::of::<field!(Inner<'static>::Δ)>()
    );
    #[composite]
    struct Another<T: Capabilities> {
        #[device(255)]
        r#type: T,
    }
    assert_ne!(
        TypeId::of::<field!(Inner<'static>::r#type)>(),
        TypeId::of::<field!(Another<Cpu<'static>>::r#type)>()
    );
    fn same<T>(_: std::marker::PhantomData<T>, _: std::marker::PhantomData<T>) {}
    same(
        std::marker::PhantomData::<<Inner<'static, 3> as Field<field!(Inner<'static, 3>::Δ)>>::Child>,
        std::marker::PhantomData::<Cpu<'static, 3>>,
    );
    assert_eq!(
        <Another<Cpu<'static>> as Field<field!(Another<Cpu<'static>>::r#type)>>::SOURCE,
        ["type"]
    );
}

mod paths {
    use super::*;
    #[composite]
    #[allow(non_camel_case_types)]
    struct r#type {
        #[device(0)]
        cpu: super::Cpu<'static>,
    }
    #[test]
    fn raw_composite_names_resolve() {
        type A = field!(r#type::cpu);
        assert_eq!(std::mem::size_of::<A>(), 0);
        assert_eq!(<r#type as Field<A>>::SOURCE, ["cpu"]);
    }
}

mod nested {
    use super::*;
    use std::{any::TypeId, marker::PhantomData};
    #[composite]
    struct Last<'a, const N: usize = 2> {
        #[device(0, alias = "leaf")]
        etc: super::Cpu<'a, N>,
    }
    #[composite]
    struct C<'a, const N: usize = 2> {
        #[device(1)]
        c: Last<'a, N>,
    }
    #[composite]
    struct B<'a, const N: usize = 2> {
        #[device(2)]
        b: C<'a, N>,
    }
    #[composite]
    struct A<'a, const N: usize = 2> {
        #[device(3)]
        a: B<'a, N>,
    }
    use A as ImportedA;
    type AliasA<'a, const N: usize = 2> = ImportedA<'a, N>;
    pub use __vihaco_fields_A as __vihaco_fields_AliasA;
    #[composite]
    struct Controller<'a, const N: usize = 2> {
        #[device(4)]
        cpu: AliasA<'a, N>,
    }
    #[composite]
    struct Machine<'a, const N: usize = 2> {
        #[device(255)]
        controller: Controller<'a, N>,
        #[device(0)]
        spare: Controller<'a, N>,
    }
    type Main<'a, const N: usize = 2> = field!(Machine<'a, N>::controller::cpu::a::b::c::etc);
    type Spare<'a, const N: usize = 2> = field!(Machine<'a, N>::spare::cpu::a::b::c::etc);
    fn same<T>(_: PhantomData<T>, _: PhantomData<T>) {}
    fn check<'a, const N: usize>(_: &'a [u8; N]) {
        same(
            PhantomData::<<Machine<'a, N> as Field<Main<'a, N>>>::Child>,
            PhantomData::<Cpu<'a, N>>,
        );
        same(
            PhantomData::<<Machine<'a, N> as Field<Spare<'a, N>>>::Child>,
            PhantomData::<Cpu<'a, N>>,
        );
    }
    trait Step<Route> {
        const DESTINATION: &'static str;
    }
    impl<'a, const N: usize> Step<Main<'a, N>> for Machine<'a, N> {
        const DESTINATION: &'static str = "main";
    }
    impl<'a, const N: usize> Step<Spare<'a, N>> for Machine<'a, N> {
        const DESTINATION: &'static str = "spare";
    }
    #[test]
    fn routes_preserve_all_steps_and_generic_child_types() {
        check(&[0; 3]);
        type Raw = framework::__private::FieldPath<
            __vihaco_fields_Machine::__markers::controller,
            framework::__private::FieldPath<
                __vihaco_fields_Controller::__markers::cpu,
                framework::__private::FieldPath<
                    __vihaco_fields_A::__markers::a,
                    framework::__private::FieldPath<
                        __vihaco_fields_B::__markers::b,
                        framework::__private::FieldPath<
                            __vihaco_fields_C::__markers::c,
                            __vihaco_fields_Last::__markers::etc,
                        >,
                    >,
                >,
            >,
        >;
        assert_eq!(TypeId::of::<Main<'static>>(), TypeId::of::<Raw>());
        assert_eq!(TypeId::of::<Main<'static, 3>>(), TypeId::of::<Raw>());
        assert_eq!(
            <Machine<'static> as Step<Main<'static>>>::DESTINATION,
            "main"
        );
        assert_eq!(
            <Machine<'static> as Step<Spare<'static>>>::DESTINATION,
            "spare"
        );
        assert_eq!(std::mem::size_of::<Main<'static>>(), 0);
        assert_ne!(
            TypeId::of::<Main<'static>>(),
            TypeId::of::<Spare<'static>>()
        );
        assert_eq!(
            <Machine<'static> as Field<Main<'static>>>::SOURCE,
            ["etc", "leaf"]
        );
        type Short = field!(Machine<'static>::controller::cpu);
        same(
            PhantomData::<<Machine<'static> as Field<Short>>::Child>,
            PhantomData::<A<'static>>,
        );
        let rows: Vec<_> = capabilities::<Machine<'static>>().collect();
        assert_eq!(rows[0].target, ["controller", "cpu", "a", "b", "c", "etc"]);
    }
}
