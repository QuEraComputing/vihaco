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

/// Declares an application-defined metadata tag and its static value.
pub(crate) trait Annotation: 'static {
    /// The concrete type recovered by a metadata lookup.
    type Value: Any + Sync;

    /// The static value shared by every occurrence of this annotation.
    const VALUE: &'static Self::Value;
}

/// Erases the value type for inspection while retaining the annotation tag’s identity.
struct MetadataEntry {
    tag: TypeId,
    name: &'static str,
    value: &'static (dyn Any + Sync),
}

/// The annotations projected for one node or edge.
#[derive(Default)]
pub(crate) struct Metadata(Vec<MetadataEntry>);

// Looks up typed annotation values and prepares tag names for display.
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

/// Marks an empty annotation list.
pub(crate) struct Unannotated;

/// Appends annotation type A to an existing type-level annotation list.
pub(crate) struct Annotated<Previous, A>(PhantomData<fn() -> (Previous, A)>);

/// Projects a type-level annotation list into inspectable entries.
pub(crate) trait Annotations {
    /// Appends this list’s entries in declaration order.
    fn collect(metadata: &mut Metadata);

    /// Materializes the annotations for one inspected graph item.
    fn metadata() -> Metadata {
        let mut metadata = Metadata::default();
        Self::collect(&mut metadata);
        metadata
    }
}

// An empty list contributes no entries.
impl Annotations for Unannotated {
    fn collect(_: &mut Metadata) {}
}

// Collects the earlier annotations before adding the current one.
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
