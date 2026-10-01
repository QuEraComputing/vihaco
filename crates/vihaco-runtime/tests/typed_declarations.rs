// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]

#[path = "support/declarations.rs"]
mod declarations;
use declarations::*;
use vihaco_runtime::__private::{
    Capabilities, Description, Device, DialectSpec, Field, InstructionCapability, Leaf,
    capabilities, require_available, require_coverage, validate,
};
use vihaco_runtime::{Composite, Effects, Execute};

// These functions are checked with the original lifetime/const bounds.
fn coverage<'a, const N: usize>(_: &'a [u8; N]) {
    require_coverage::<Cpu<'a, N>, RenamedDialect>();
    require_available::<Cpu<'a, N>, A>();
    require_available::<Cpu<'a, N>, B>();
}

struct Inner<C>(C);
impl<C> Composite for Inner<C> {}
#[allow(non_upper_case_globals)]
impl<C> Inner<C> {
    #[doc(hidden)]
    pub const __vihaco_field_cpu: usize = 0;
}
impl<C> Field<0> for Inner<C> {
    type Child = C;
    const SOURCE: &'static [&'static str] = &["cpu", "p", "p_long", "λ", "!"];
}
impl<C: Capabilities> Capabilities for Inner<C> {
    const DESCRIPTION: &'static Description = &Description::Composite(&[Device {
        name: <Self as Field<0>>::SOURCE[0],
        child: C::DESCRIPTION,
    }]);
}
trait Configuration {
    type Device: Capabilities;
}
struct Config<'a>(std::marker::PhantomData<&'a ()>);
impl<'a> Configuration for Config<'a> {
    type Device = Cpu<'a>;
}
struct Outer<T: Configuration> {
    r#type: Inner<T::Device>,
}
impl<T: Configuration> Composite for Outer<T> {}
#[allow(non_upper_case_globals)]
impl<T: Configuration> Outer<T> {
    #[doc(hidden)]
    pub const __vihaco_field_type: usize = 0;
}
impl<T: Configuration> Field<0> for Outer<T> {
    type Child = Inner<T::Device>;
    const SOURCE: &'static [&'static str] = &["type", "outer"];
}
impl<T: Configuration> Capabilities for Outer<T> {
    const DESCRIPTION: &'static Description = &Description::Composite(&[Device {
        name: <Self as Field<0>>::SOURCE[0],
        child: Inner::<T::Device>::DESCRIPTION,
    }]);
}

#[test]
fn aliases_coverage_and_borrowed_associated_fields_keep_real_types() {
    let state = [1, 2];
    coverage(&state);
    let mut machine = Outer::<Config> {
        r#type: Inner(Cpu(&state)),
    };
    // Literal indices keep associations available under original generic bounds.
    type Child<'a> = <<Outer<Config<'a>> as Field<0>>::Child as Field<0>>::Child;
    let cpu: &mut Child<'_> = &mut machine.r#type.0;
    assert_eq!(cpu.execute(&A, 3).unwrap(), 5);
    assert_eq!(cpu.execute(&B, ()).unwrap(), Effects::one(2));
    cpu.execute(&Extra, ()).unwrap(); // Execution is not dialect membership.
    let _: Option<<A as vihaco_runtime::__private::InstructionSpec>::Surface> = Some(7u32);
}

#[test]
fn named_indices_resolve_nested_aliases_and_are_local_to_each_parent() {
    type MachineAlias = Outer<Config<'static>>;
    type InnerAlias = <MachineAlias as Field<{ MachineAlias::__vihaco_field_type }>>::Child;
    type CpuChild = <InnerAlias as Field<{ InnerAlias::__vihaco_field_cpu }>>::Child;
    let _: Option<Cpu<'static>> = None::<CpuChild>;

    // This parent's cpu has index 1, while Inner's cpu has index 0.
    struct Reordered {
        inner: Inner<Cpu<'static>>,
        cpu: u32,
    }
    #[allow(non_upper_case_globals)]
    impl Reordered {
        const __vihaco_field_inner: usize = 0;
        const __vihaco_field_cpu: usize = 1;
    }
    impl Field<0> for Reordered {
        type Child = Inner<Cpu<'static>>;
        const SOURCE: &'static [&'static str] = &["inner"];
    }
    impl Field<1> for Reordered {
        type Child = u32;
        const SOURCE: &'static [&'static str] = &["cpu"];
    }
    let mut machine = Reordered {
        inner: Inner(Cpu(&[3, 4])),
        cpu: 7,
    };
    let cpu: &mut <Reordered as Field<{ Reordered::__vihaco_field_cpu }>>::Child = &mut machine.cpu;
    assert_eq!(*cpu, 7);
    type Nested = <Reordered as Field<{ Reordered::__vihaco_field_inner }>>::Child;
    type NestedCpu = <Nested as Field<{ Nested::__vihaco_field_cpu }>>::Child;
    let cpu: &mut NestedCpu = &mut machine.inner.0;
    assert_eq!(cpu.execute(&A, 2).unwrap(), 4);
}

#[test]
fn reporting_preserves_full_order_and_canonical_destinations() {
    let rows: Vec<InstructionCapability> = capabilities::<Outer<Config>>().collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].target, ["type", "cpu"]);
    assert_eq!((rows[0].dialect, rows[0].instruction), ("cpu", "a"));
    assert_eq!(rows[1].instruction, "b");
    assert_eq!(
        <Inner<Cpu> as Field<{ Inner::<Cpu>::__vihaco_field_cpu }>>::SOURCE,
        ["cpu", "p", "p_long", "λ", "!"]
    );
    assert!(capabilities::<Cpu>().all(|row| row.target.is_empty()));
}

#[test]
fn same_head_disjoint_dialects_and_repeated_types_at_distinct_targets_are_valid() {
    struct Machine;
    const DISJOINT: &[Leaf] = &[
        Leaf {
            dialect: "cpu",
            mnemonic: "c",
        },
        Leaf {
            dialect: "cpu",
            mnemonic: "A",
        },
        Leaf {
            dialect: "cpu",
            mnemonic: "λ",
        },
        Leaf {
            dialect: "cpu",
            mnemonic: "µ",
        },
    ];
    const DIFFERENT_HEAD: &[Leaf] = &[Leaf {
        dialect: "other",
        mnemonic: "a",
    }];
    const CHILD: Description = Description::Component(&[Dialect::LEAVES, DISJOINT, DIFFERENT_HEAD]);
    impl Capabilities for Machine {
        const DESCRIPTION: &'static Description = &Description::Composite(&[
            Device {
                name: "left",
                child: &CHILD,
            },
            Device {
                name: "right",
                child: &CHILD,
            },
        ]);
    }
    let rows: Vec<_> = capabilities::<Machine>().collect();
    assert_eq!(
        rows.iter().map(|row| row.instruction).collect::<Vec<_>>(),
        [
            "a", "b", "c", "A", "λ", "µ", "a", "a", "b", "c", "A", "λ", "µ", "a"
        ]
    );
    assert_eq!((rows[6].dialect, rows[6].instruction), ("other", "a"));
    assert!(rows[..7].iter().all(|row| row.target == ["left"]));
    assert!(rows[7..].iter().all(|row| row.target == ["right"]));
}

#[test]
fn empty_nested_composites_do_not_corrupt_iterator_paths() {
    struct Machine;
    const EMPTY: Description = Description::Composite(&[]);
    impl Capabilities for Machine {
        const DESCRIPTION: &'static Description = &Description::Composite(&[
            Device {
                name: "empty",
                child: &EMPTY,
            },
            Device {
                name: "nested",
                child: &Description::Composite(&[
                    Device {
                        name: "also_empty",
                        child: &EMPTY,
                    },
                    Device {
                        name: "live",
                        child: <Cpu as Capabilities>::DESCRIPTION,
                    },
                ]),
            },
        ]);
    }
    let mut rows = capabilities::<Machine>();
    assert_eq!(rows.next().unwrap().target, ["nested", "live"]);
    assert_eq!(rows.next().unwrap().instruction, "b");
    assert!(rows.next().is_none());
    assert!(rows.next().is_none());
}

#[test]
fn complete_validation_rejects_overlap_in_unselected_children() {
    const PARTIAL: &[Leaf] = &[
        Leaf {
            dialect: "cpu",
            mnemonic: "c",
        },
        Leaf {
            dialect: "cpu",
            mnemonic: "b",
        },
    ];
    const OVERLAP: Description = Description::Component(&[Dialect::LEAVES, PARTIAL]);
    const DEEP: Description = Description::Composite(&[Device {
        name: "unselected",
        child: &OVERLAP,
    }]);
    assert!(std::panic::catch_unwind(|| validate(&DEEP)).is_err());
}

#[test]
fn typed_relationships_reject_invalid_declarations() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/declarations/*.rs");
}
