// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Static tag definitions and the metadata projected from them.
//!
//! Attaching a tag carries its type in the plan. Projection materializes entries
//! in attachment order. Lookup returns the most recent match, either by exact
//! tag or by value type. The simulator uses value types to recognize
//! `MinExecution<N>` across different values of `N`.

use super::short_name;
use std::{
    any::{Any, TypeId, type_name},
    marker::PhantomData,
};

/// Declares a typed metadata tag that can be attached to a node or edge.
///
/// The tag type provides identity. Its value type lets a reader group tags
/// that represent the same kind of metadata.
pub(crate) trait Annotation: 'static {
    /// The concrete type recovered by a metadata lookup.
    type Value: Any + Sync;

    /// The static value shared by every occurrence of this annotation.
    const VALUE: &'static Self::Value;
}

/// Stores one projected annotation with its tag identity and printable name.
///
/// The value's concrete type can be recovered through a typed lookup.
struct MetadataEntry {
    tag: TypeId,
    name: &'static str,
    value: &'static (dyn Any + Sync),
}

/// Holds the annotations for one node or edge in attachment order.
///
/// Inspection can look up an exact tag. Drivers can look up a value type,
/// such as execution ticks, across several tags.
#[derive(Default)]
pub(crate) struct Metadata(Vec<MetadataEntry>);

impl Metadata {
    /// Returns the most recently attached value for this annotation tag.
    pub(crate) fn get<A: Annotation>(&self) -> Option<&'static A::Value> {
        let entry = self
            .0
            .iter()
            .rev()
            .find(|entry| entry.tag == TypeId::of::<A>())?;
        (entry.value as &dyn Any).downcast_ref()
    }

    /// Returns the last annotation value with this semantic type.
    #[allow(dead_code, reason = "shared with the simulator and program examples")]
    pub(crate) fn get_value<T: Any + Sync>(&self) -> Option<&'static T> {
        self.0
            .iter()
            .rev()
            .find_map(|entry| (entry.value as &dyn Any).downcast_ref())
    }

    /// Returns concise tag names for the diagnostic display.
    pub(super) fn names(&self) -> Vec<&'static str> {
        self.0.iter().map(|entry| short_name(entry.name)).collect()
    }
}

/// Represents the empty annotation list used by a bare node or edge.
pub(crate) struct Unannotated;

/// Represents the annotation list formed by appending tag `A` to `Previous`.
pub(crate) struct Annotated<Previous, A>(PhantomData<fn() -> (Previous, A)>);

/// Materializes annotation entries from their type-level list.
///
/// Both topology inspection and the annotated execution hook use this projection.
pub(crate) trait Annotations {
    /// Appends this list’s entries in declaration order.
    fn collect(metadata: &mut Metadata);

    /// Creates the projected annotations for one node or edge.
    fn metadata() -> Metadata {
        let mut metadata = Metadata::default();
        Self::collect(&mut metadata);
        metadata
    }
}

/// An empty list contributes no entries.
impl Annotations for Unannotated {
    fn collect(_: &mut Metadata) {}
}

/// Collects the earlier annotations before adding the current one.
impl<Previous: Annotations, A: Annotation> Annotations for Annotated<Previous, A> {
    fn collect(metadata: &mut Metadata) {
        Previous::collect(metadata);
        metadata.0.push(MetadataEntry {
            tag: TypeId::of::<A>(),
            name: type_name::<A>(),
            value: A::VALUE,
        });
    }
}
