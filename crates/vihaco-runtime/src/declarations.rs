// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Cross-crate contracts emitted by the typed declaration macros.
//!
//! These traits are visible for generated code, not a manual producer API.
//! Their contracts may change with the owning macros; user implementations
//! are unsupported.
//! Actual types carry identity; static descriptions carry ordered source names.
//! Coverage is checked under the declaration's original generic bounds.
//!
//! Owning macros must force `validate` in a declaration-level const when the
//! description is fixed, and at concrete instruction-set association. An unused
//! generic associated constant is insufficient. Internal capability reporting
//! forces validation for its concrete monomorphization at code generation.

use crate::{Component, Execute, Parse};

pub use vihaco_parser_derive::__dialect_parsers;

pub use crate::capability::{InstructionCapability, capabilities};

/// One actual runtime instruction and its paired source representation.
pub trait InstructionSpec {
    type Surface: for<'src> Parse<'src>;
    type Dialect: DialectSpec;
    const MNEMONIC: &'static str;
}

/// Nominal dialect identity and its complete ordered instruction description.
pub trait DialectSpec {
    /// Generated leaves share one canonical dialect name and have distinct
    /// mnemonics, checked by the dialect macro before emission.
    const LEAVES: &'static [Leaf];
}

/// Declared dialect membership, independent of arbitrary Execute impls.
pub trait Supports<D: DialectSpec>: Component {}

/// Emitted by a dialect with one Execute bound per declared instruction.
pub trait Coverage<C: Component>: DialectSpec {}

/// Called under the component declaration's original generic bounds.
pub fn require_coverage<C: Component, D: Coverage<C>>() {}

pub fn require_available<C, I>()
where
    I: InstructionSpec,
    C: Supports<I::Dialect> + Execute<I>,
{
}

/// One eligible field's actual type and source spellings.
///
/// Indices are local to the parent type. The composite macro emits an impl with
/// a literal index and a hidden `__vihaco_field_<name>` associated constant.
/// The child is the exact authored field type in the parent declaration scope.
/// Imported aliases and associated/generic types retain ordinary Rust identity.
/// Ordinary state and loadable section fields have no relation: a section stops
/// parent target traversal and capability lifting, while its loader still runs.
/// Target lookup reads that constant from the actual parent type, including
/// through aliases; users never need to spell or coordinate numeric indices.
pub trait Field<const INDEX: usize> {
    type Child;
    /// Canonical field name first, then aliases in declaration order.
    const SOURCE: &'static [&'static str];
}

/// Declaration-local device metadata, including loadable section boundaries.
/// Codes remain optional; no implicit numeric code is assigned. Consumers of
/// numbered metadata must require explicit codes rather than infer them from
/// target indices. Ordinary state fields are absent.
pub struct CompositeDevice {
    pub name: &'static str,
    pub code: Option<u8>,
    pub source: &'static [&'static str],
    pub section: Option<&'static str>,
}

pub trait CompositeSpec {
    const DEVICES: &'static [CompositeDevice];
}

/// Canonical source selector for one declared instruction.
pub struct Leaf {
    pub dialect: &'static str,
    pub mnemonic: &'static str,
}

/// Borrowed immutable descriptions; no registry or dispatch table is needed.
pub enum Description {
    Component(&'static [&'static [Leaf]]),
    Composite(&'static [Device]),
}

pub struct Device {
    /// Canonical field name; parser aliases live in Field::SOURCE.
    pub name: &'static str,
    pub child: &'static Description,
}

/// Complete availability, independent of `HasInstructionSet` selection.
/// Composites reference every eligible child description in field order; a
/// child's executable set never filters these descriptions. Repeated child
/// types at distinct fields are distinct destinations. Parsing aliases live in
/// source metadata, so reports use one canonical path per instruction.
pub trait Capabilities {
    /// Raw generated data. Internal reporting uses the validated `capabilities`.
    const DESCRIPTION: &'static Description;
}

// str/slice PartialEq is not const-stable (including on Rust 1.96).
// Required only for cross-crate selector checks during const evaluation.
const fn equal(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

// Each generated dialect list has one canonical dialect name. Different names
// cannot overlap; only matching names require instruction-pair comparisons.
const fn assert_disjoint_dialects(mut left: &[Leaf], right: &[Leaf]) {
    let (Some(left_head), Some(right_head)) = (left.first(), right.first()) else {
        return;
    };
    if !equal(left_head.dialect, right_head.dialect) {
        return;
    }
    while let Some((leaf, tail)) = left.split_first() {
        let mut candidates = right;
        while let Some((other, other_tail)) = candidates.split_first() {
            assert!(
                !equal(leaf.mnemonic, other.mnemonic),
                "overlapping canonical instruction selector"
            );
            candidates = other_tail;
        }
        left = tail;
    }
}

/// Check complete canonical selectors across dialects in a const context.
/// Source-name syntax and local alias collisions are checked by owning macros
/// with standard string/character operations before descriptions are emitted.
///
/// Repeated instruction selectors reject within one component. The same leaf
/// at distinct device destinations is valid. Every child is checked, including
/// children omitted from a downstream executable selection.
/// Dialect pairs are compared once. Different heads skip leaf comparisons;
/// matching heads use allocation-free pairwise mnemonic checks.
pub const fn validate(description: &Description) {
    match description {
        Description::Component(dialects) => {
            let mut remaining = *dialects;
            while let Some((dialect, tail)) = remaining.split_first() {
                let mut others = tail;
                while let Some((other, other_tail)) = others.split_first() {
                    assert_disjoint_dialects(dialect, other);
                    others = other_tail;
                }
                remaining = tail;
            }
        }
        Description::Composite(devices) => {
            let mut remaining = *devices;
            while let Some((device, tail)) = remaining.split_first() {
                validate(device.child);
                remaining = tail;
            }
        }
    }
}
