// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![allow(dead_code, non_snake_case)]
use framework::__private::{Capabilities, CompositeSpec, Field, capabilities};
use framework::{Execute, HasInstructionSet, component, dialect};
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
fn exact_types<'a, const N: usize>() {
    fn same<T>(_: std::marker::PhantomData<T>, _: std::marker::PhantomData<T>) {}
    same(
        std::marker::PhantomData::<<Inner<'a, N> as Field<0>>::Child>,
        std::marker::PhantomData::<Cpu<'a, N>>,
    );
    same(
        std::marker::PhantomData::<<<Outer<'a, Choice, N> as Field<0>>::Child as Field<1>>::Child>,
        std::marker::PhantomData::<Cpu<'a, N>>,
    );
    same(
        std::marker::PhantomData::<<Cpu<'a, N> as HasInstructionSet>::Runtime>,
        std::marker::PhantomData::<control::Halt>,
    );
}
fn main() {
    exact_types::<2>();
    assert_eq!(Inner::<'static, 2>::__vihaco_field_type, 0);
    assert_eq!(Inner::<'static, 2>::__vihaco_field_MiXeD, 1);
    assert_eq!(Inner::<'static, 2>::__vihaco_field_Δ, 2);
    assert_eq!(
        <Inner<'static> as Field<0>>::SOURCE,
        ["type", "core", "core-v2"]
    );
    assert_eq!(<Inner<'static> as Field<1>>::SOURCE, ["MiXeD", "other"]);
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
    for (pair, target) in rows.chunks_exact(2).zip(expected) {
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
    const __vihaco_check_capabilities: usize = 99;
}
