#![allow(dead_code)] // Unused declaration checks must still be type checked.
use chumsky::{Parser, error::Simple, extra};
use vm::{
    Capabilities, Component, Description, Device, DialectSpec, Execute, Field, InstructionSpec,
    Parse, Supports,
};

pub type RenamedAdd = chain::again::renamed::Add;
pub type AddSurface = <RenamedAdd as InstructionSpec>::Surface;

pub struct Cpu<T: Clone = u8, const N: usize = 2>(pub T);
impl<T: Clone, const N: usize> Component for Cpu<T, N> {}
impl<T: Clone, const N: usize, I: InstructionSpec> Execute<I> for Cpu<T, N> {
    type Message = ();
    type Effect = vm::Effects<usize>;
    type Error = vm::eyre::Report;
    fn execute(&mut self, _: &I, _: ()) -> Result<Self::Effect, Self::Error> {
        Ok(vm::Effects::one(N))
    }
}
impl<T: Clone, const N: usize> Supports<external::arithmetic::__VihacoDialect> for Cpu<T, N> {}
impl<T: Clone, const N: usize> Supports<external::control::__VihacoDialect> for Cpu<T, N> {}
impl<T: Clone, const N: usize> Capabilities for Cpu<T, N> {
    const DESCRIPTION: &'static Description = &Description::Component(&[
        external::arithmetic::__VihacoDialect::LEAVES,
        external::control::__VihacoDialect::LEAVES,
    ]);
}
const _: () = {
    // The declaration checks the original bounds without adding coverage bounds.
    fn coverage<T: Clone, const N: usize>() {
        vm::require_coverage::<Cpu<T, N>, external::arithmetic::__VihacoDialect>();
        vm::require_coverage::<Cpu<T, N>, external::control::__VihacoDialect>();
    }
    vm::validate(<Cpu<u8> as Capabilities>::DESCRIPTION);
};

pub struct Controller<C>(pub C);
#[allow(non_upper_case_globals)]
impl<C> Controller<C> {
    #[doc(hidden)]
    pub const __vihaco_field_cpu: usize = 0;
}
impl<C> Field<0> for Controller<C> {
    type Child = C;
    const SOURCE: &'static [&'static str] = &["cpu", "p", "p_long", "λ", "!"];
}
impl<C: Capabilities> Capabilities for Controller<C> {
    const DESCRIPTION: &'static Description = &Description::Composite(&[Device {
        name: <Self as Field<0>>::SOURCE[0],
        child: C::DESCRIPTION,
    }]);
}
pub struct Machine<C>(pub Controller<C>);
#[allow(non_upper_case_globals)]
impl<C> Machine<C> {
    #[doc(hidden)]
    pub const __vihaco_field_controller: usize = 0;
}
impl<C> Field<0> for Machine<C> {
    type Child = Controller<C>;
    const SOURCE: &'static [&'static str] = &["controller", "ctl", "ctl_long"];
}
impl<C: Capabilities> Capabilities for Machine<C> {
    const DESCRIPTION: &'static Description = &Description::Composite(&[Device {
        name: <Self as Field<0>>::SOURCE[0],
        child: Controller::<C>::DESCRIPTION,
    }]);
}
pub type ConcreteMachine = Machine<Cpu<u8>>;

#[declarations::instruction_set(composite = self::ConcreteMachine)]
#[derive(Clone, Debug, PartialEq)]
pub enum MachineInstruction {
    #[target(controller::cpu)]
    Add(RenamedAdd),
    #[target(controller::cpu)]
    Halt(chain::again::control::Halt),
    #[target(controller::cpu)]
    Symbol(external::arithmetic::Symbol),
}

pub fn selected_parser<'src>()
-> impl Parser<'src, &'src str, MachineSurfaceInstruction, extra::Err<Simple<'src, char>>> {
    let prefixes = vm::prefix(
        <ConcreteMachine as Field<{ ConcreteMachine::__vihaco_field_controller }>>::SOURCE,
    )
    .then(vm::prefix(
        <Controller<Cpu<u8>> as Field<{ Controller::<Cpu<u8>>::__vihaco_field_cpu }>>::SOURCE,
    ));
    let add = prefixes
        .clone()
        .ignore_then(AddSurface::parser())
        .map(MachineSurfaceInstruction::Add)
        .boxed();
    let halt = prefixes
        .clone()
        .ignore_then(<chain::again::control::Halt as InstructionSpec>::Surface::parser())
        .map(MachineSurfaceInstruction::Halt)
        .boxed();
    let symbol = prefixes
        .ignore_then(<external::arithmetic::Symbol as InstructionSpec>::Surface::parser())
        .map(MachineSurfaceInstruction::Symbol)
        .boxed();
    add.or(halt).boxed().or(symbol).boxed()
}

pub trait DeviceType {
    type Cpu: Capabilities;
}
pub struct Config;
impl DeviceType for Config {
    type Cpu = Cpu<u8>;
}
pub struct Associated<'a, T: DeviceType, const N: usize = 2> {
    pub cpu: T::Cpu,
    pub state: &'a [u8; N],
}
#[allow(non_upper_case_globals)]
impl<'a, T: DeviceType, const N: usize> Associated<'a, T, N> {
    #[doc(hidden)]
    pub const __vihaco_field_cpu: usize = 0;
}
impl<'a, T: DeviceType, const N: usize> Field<0> for Associated<'a, T, N> {
    type Child = T::Cpu;
    const SOURCE: &'static [&'static str] = &["cpu"];
}

pub const OVERLAP: Description = Description::Component(&[
    external::arithmetic::__VihacoDialect::LEAVES,
    external::arithmetic::__VihacoDialect::LEAVES,
]);
pub struct Bad<T>(pub T);
impl<T> Capabilities for Bad<T> {
    const DESCRIPTION: &'static Description = &OVERLAP;
}
impl<T> Bad<T> {
    pub const UNUSED: () = vm::validate(&OVERLAP);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_real_fields_aliases_associated_types_and_execution() {
        let mut machine = Machine(Controller(Cpu::<u8>(7)));
        type Parent =
            <ConcreteMachine as Field<{ ConcreteMachine::__vihaco_field_controller }>>::Child;
        type Child = <Parent as Field<{ Parent::__vihaco_field_cpu }>>::Child;
        let cpu: &mut Child = &mut machine.0.0;
        assert_eq!(cpu.0, 7);
        assert_eq!(
            cpu.execute(&external::control::Halt, ()).unwrap(),
            vm::Effects::one(2)
        );
        let mut associated = Associated::<Config> {
            cpu: Cpu(1),
            state: &[0; 2],
        };
        let _: &mut <Associated<Config> as Field<{ Associated::<Config>::__vihaco_field_cpu }>>::Child = &mut associated.cpu;
        // An unrelated Execute exists but does not imply Supports for this dialect.
        machine.0.0.execute(&external::r#type::Stop, ()).unwrap();
        let value = 9u8;
        let mut borrowed = Machine(Controller(Cpu::<&u8>(&value)));
        type BorrowedChild<'a> = <<Machine<Cpu<&'a u8>> as Field<0>>::Child as Field<0>>::Child;
        let child: &mut BorrowedChild<'_> = &mut borrowed.0.0;
        assert_eq!(*child.0, 9);
    }

    #[test]
    fn descriptions_are_complete_ordered_and_independent_of_selected_enum() {
        let rows = vm::flatten(vm::description::<ConcreteMachine>());
        assert_eq!(
            rows.iter().map(|(_, leaf)| leaf.name).collect::<Vec<_>>(),
            ["add", "symbol", "scoped", "a", "jump", "halt", "omitted"]
        );
        assert!(rows.iter().all(|(path, _)| path == &["controller", "cpu"]));
        let Description::Composite(devices) = ConcreteMachine::DESCRIPTION else {
            panic!()
        };
        assert_eq!(devices[0].name, "controller");
        assert_eq!(
            <ConcreteMachine as Field<{ ConcreteMachine::__vihaco_field_controller }>>::SOURCE,
            ["controller", "ctl", "ctl_long"]
        );
        vm::validate(&Description::Component(&[
            external::left::cpu::__VihacoDialect::LEAVES,
            external::right::cpu::__VihacoDialect::LEAVES,
        ]));
        assert_eq!(external::r#type::__VihacoDialect::LEAVES[0].head, "type");
    }

    #[test]
    fn authored_scope_symbol_values_and_surface_association() {
        let add: AddSurface = external::arithmetic::AddSurface(3);
        assert_eq!(
            add,
            AddSurface::parser()
                .parse("arithmetic.add 3")
                .into_result()
                .unwrap()
        );
        let scoped = external::arithmetic::ScopedSurface::parser()
            .parse("arithmetic.scoped 1, 2, 3, [4, 5]")
            .into_result()
            .unwrap();
        assert_eq!(
            scoped,
            external::arithmetic::ScopedSurface(1, 2, 3, vec![4, 5])
        );
        let _: external::arithmetic::A = external::arithmetic::A(9);
        assert_eq!(
            external::arithmetic::ASurface::parser()
                .parse("arithmetic.a 9")
                .into_result()
                .unwrap(),
            external::arithmetic::ASurface(9)
        );
        assert_eq!(
            external::arithmetic::BranchSurface::parser()
                .parse("arithmetic.jump 7")
                .into_result()
                .unwrap(),
            external::arithmetic::BranchSurface(7)
        );
        let _: Option<<ConcreteMachine as vm::HasInstructionSet>::Runtime> = None;
        let _: Option<<ConcreteMachine as vm::HasInstructionSet>::Syntax> = None;
    }

    #[test]
    fn selected_only_nested_aliases_prefixes_and_errors() {
        for ctl in ["controller", "ctl", "ctl_long"] {
            for cpu in ["cpu", "p", "p_long", "λ", "!"] {
                let input = format!("{ctl}::{cpu}::arithmetic.add -3");
                assert_eq!(
                    selected_parser().parse(&input).into_result().unwrap(),
                    MachineSurfaceInstruction::Add(external::arithmetic::AddSurface(-3))
                );
            }
        }
        assert_eq!(
            selected_parser()
                .parse("ctl::p::arithmetic.symbol target")
                .into_result()
                .unwrap(),
            MachineSurfaceInstruction::Symbol(external::arithmetic::SymbolSurface(
                vm::vihaco_parser_ident::Ident("target".into())
            ))
        );
        for input in [
            "ctl::p::control.omitted",
            "ctl::p::arithmetic.scoped 1, 2, 3, []",
            "ctl::p::arithmetic.add nope",
            "ctl::p::control.halt extra",
            "ctl::px::control.halt",
        ] {
            assert!(selected_parser().parse(input).has_errors(), "{input}");
        }
    }

    #[test]
    fn thirty_associated_leaf_parsers_exceed_tuple_choice_limit() {
        macro_rules! choices {
            ($first:ident, $($rest:ident),+) => {{
                let mut branches = vec![(<external::many::$first as InstructionSpec>::MNEMONIC.len(),
                    <external::many::$first as InstructionSpec>::Surface::parser().map(|_| stringify!($first)).boxed()),
                    $((<external::many::$rest as InstructionSpec>::MNEMONIC.len(),
                    <external::many::$rest as InstructionSpec>::Surface::parser().map(|_| stringify!($rest)).boxed())),+];
                branches.sort_by_key(|(length, _)| std::cmp::Reverse(*length));
                let mut branches = branches.into_iter();
                let (_, first) = branches.next().unwrap();
                branches.fold(first, |parser, (_, branch)| parser.or(branch).boxed())
            }};
        }
        // Source alternatives need longest-mnemonic-first ordering independently
        // of authored enum/description order, as in the existing parser derive.
        let inputs: Vec<_> = (0..30).map(|i| format!("many.v{i}")).collect();
        let parser = choices!(
            V0, V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13, V14, V15, V16, V17, V18,
            V19, V20, V21, V22, V23, V24, V25, V26, V27, V28, V29
        );
        for (i, text) in inputs.iter().enumerate() {
            assert_eq!(
                parser.parse(text.as_str()).into_result().unwrap(),
                format!("V{i}")
            );
        }
    }

    #[test]
    fn function_local_checks_and_raw_field_indices() {
        struct Local(Cpu<u8>);
        #[allow(non_upper_case_globals)]
        impl Local {
            const __vihaco_field_type: usize = 0;
        }
        impl Field<{ Local::__vihaco_field_type }> for Local {
            type Child = Cpu<u8>;
            const SOURCE: &'static [&'static str] = &["type"];
        }
        let mut local = Local(Cpu(1));
        let _: &mut <Local as Field<{ Local::__vihaco_field_type }>>::Child = &mut local.0;
        fn check_local() {
            vm::require_available::<Cpu<u8>, chain::again::control::Halt>();
        }
        check_local();
    }
}
