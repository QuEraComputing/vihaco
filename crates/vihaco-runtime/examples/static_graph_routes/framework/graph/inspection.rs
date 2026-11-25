// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Project node occurrences and dependencies without executing a route.
//!
//! A path accepts an optional incoming edge and returns its last node ID. A
//! sequence connects its paths using those endpoints. A fork describes both
//! arms and adds a synthetic join. Indices and field names identify occurrences
//! for diagnostics within a projection.

use std::any::TypeId;

use eyre::{Result, ensure, eyre};

use super::{
    annotations::{Annotations, Metadata},
    builder::{DeliverFork, ForkJoin, Node, Route, Sequence},
    short_name,
};

impl<I, P, Mode> Route<I, P, Mode> {
    /// Projects the route without requiring a machine, driver, or executable path.
    pub(crate) fn topology(&self) -> Topology
    where
        P: Describe,
    {
        Topology::describe::<P>(&mut std::iter::empty())
    }
}

/// A route-local index into the backend policy table for named handoffs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EdgeId(pub(crate) usize);

/// The named topology supplied to a backend at configuration time.
pub(crate) struct RegisteredTopology {
    pub(crate) topology: Topology,
    pub(crate) route_key: TypeId,
}

/// Supplies the diagnostic name and optional machine field for an operation.
pub(crate) trait Label {
    /// The diagnostic name for an operation occurrence.
    const LABEL: &'static str;

    /// Returns the emitting field for a composite effect handler.
    fn source_field() -> Option<&'static str> {
        None
    }

    /// Returns the selected field marker’s type name when the operation targets a field.
    fn field() -> Option<&'static str> {
        None
    }
}

/// A diagnostic graph projected from a route's static path.
///
/// Node indices belong to this projection. Each occurrence has its own index,
/// even when multiple nodes use the same operation or field.
#[derive(Default)]
pub(crate) struct Topology {
    /// Nodes in traversal order, indexed by their occurrence IDs.
    pub(crate) nodes: Vec<Vertex>,

    /// Connections between occurrences in this projection.
    pub(crate) edges: Vec<Link>,
}

/// One operation occurrence or synthetic join in the projected graph.
pub(crate) struct Vertex {
    /// The optional name supplied by the route author.
    pub(crate) site_name: Option<&'static str>,

    /// The operation’s diagnostic label.
    pub(crate) name: &'static str,

    /// The field marker’s type name for a field-directed operation.
    pub(crate) field: Option<&'static str>,
    /// Emitting field for a composite effect handler.
    pub(crate) source_field: Option<&'static str>,

    /// Metadata attached to this operation occurrence.
    pub(crate) metadata: Metadata,
}

/// A directed payload dependency between two projected node occurrences.
pub(crate) struct Link {
    pub(crate) site_name: Option<&'static str>,

    /// The node occurrence producing the payload.
    pub(crate) from: usize,

    /// The node occurrence receiving the payload.
    pub(crate) to: usize,

    /// Metadata attached to this connection.
    pub(crate) metadata: Metadata,
}

impl Topology {
    pub(super) fn describe<P: Describe>(names: &mut impl Iterator<Item = &'static str>) -> Self {
        let mut topology = Self::default();
        P::describe(None, &mut topology, names);
        topology
    }

    /// Resolves one named edge within this topology before execution.
    pub(crate) fn named_edge(&self, name: &str) -> Result<EdgeId> {
        let mut matches = self
            .edges
            .iter()
            .filter_map(|link| link.site_name)
            .enumerate()
            .filter(|(_, site)| *site == name);
        let (index, _) = matches
            .next()
            .ok_or_else(|| eyre!("unknown route edge {name:?}"))?;
        ensure!(matches.next().is_none(), "ambiguous route edge {name:?}");
        Ok(EdgeId(index))
    }

    /// Adds a node and connects its incoming edge when it has a predecessor.
    fn vertex(
        &mut self,
        name: &'static str,
        field: Option<&'static str>,
        source_field: Option<&'static str>,
        metadata: Metadata,
        from: Incoming,
    ) -> usize {
        let id = self.nodes.len();
        self.nodes.push(Vertex {
            name,
            site_name: None,
            field,
            source_field,
            metadata,
        });
        if let Some((from, metadata, site_name)) = from {
            self.edges.push(Link {
                site_name,
                from,
                to: id,
                metadata,
            });
        }
        id
    }

    /// Displays node indices, edge endpoints, and annotation tag names.
    pub(crate) fn print(&self, title: &str) {
        println!("{title}");
        for (id, node) in self.nodes.iter().enumerate() {
            let site = node.field.map(short_name).unwrap_or("");
            let source = node
                .source_field
                .map(|field| format!("{} → ", short_name(field)))
                .unwrap_or_default();
            let name = node
                .site_name
                .map(|name| format!(" ({name})"))
                .unwrap_or_default();
            println!(
                "  {id}  {} {source}{site}{name} {:?}",
                node.name,
                node.metadata.names()
            );
        }
        for link in &self.edges {
            let name = link
                .site_name
                .map(|name| format!(" ({name})"))
                .unwrap_or_default();
            println!(
                "  {} → {}{name}  {:?}",
                link.from,
                link.to,
                link.metadata.names()
            );
        }
    }
}

/// Carries a predecessor and its edge properties into a path.
pub(crate) type Incoming = Option<(usize, Metadata, Option<&'static str>)>;

/// Appends a path to the topology and returns its exit node's index.
pub(crate) trait Describe {
    fn describe(
        from: Incoming,
        topology: &mut Topology,
        names: &mut impl Iterator<Item = &'static str>,
    ) -> usize;
}

impl<N: Label, A: Annotations, Execution> Describe for Node<N, A, Execution> {
    fn describe(
        from: Incoming,
        topology: &mut Topology,
        names: &mut impl Iterator<Item = &'static str>,
    ) -> usize {
        let id = topology.vertex(N::LABEL, N::field(), N::source_field(), A::metadata(), from);
        topology.nodes[id].site_name = A::NAMED.then(|| names.next()).flatten();
        id
    }
}

impl<L: Describe, E: Annotations, R: Describe> Describe for Sequence<L, E, R> {
    fn describe(
        from: Incoming,
        topology: &mut Topology,
        names: &mut impl Iterator<Item = &'static str>,
    ) -> usize {
        let predecessor = L::describe(from, topology, names);
        let name = E::NAMED.then(|| names.next()).flatten();
        R::describe(Some((predecessor, E::metadata(), name)), topology, names)
    }
}

trait DescribeBranches {
    fn describe(
        split: usize,
        topology: &mut Topology,
        names: &mut impl Iterator<Item = &'static str>,
    ) -> Vec<usize>;
}

macro_rules! describe_branches {
    ($(($branch:ident, $input:ident, $value:ident, $index:tt)),+) => {
        impl<$($branch: Describe,)+> DescribeBranches for ($($branch,)+) {
            fn describe(split: usize, topology: &mut Topology, names: &mut impl Iterator<Item = &'static str>) -> Vec<usize> {
                vec![$($branch::describe(Some((split, Metadata::default(), None)), topology, names),)+]
            }
        }
    };
}
for_each_branch_tuple!(describe_branches);

impl<S: Describe, B: DescribeBranches> Describe for ForkJoin<S, B> {
    fn describe(
        from: Incoming,
        topology: &mut Topology,
        names: &mut impl Iterator<Item = &'static str>,
    ) -> usize {
        let split = S::describe(from, topology, names);
        let exits = B::describe(split, topology, names);
        let join = topology.vertex("join results", None, None, Metadata::default(), None);
        topology.edges.extend(exits.into_iter().map(|from| Link {
            from,
            to: join,
            metadata: Metadata::default(),
            site_name: None,
        }));
        join
    }
}

impl<S: Describe, B: DescribeBranches> Describe for DeliverFork<S, B> {
    fn describe(
        from: Incoming,
        topology: &mut Topology,
        names: &mut impl Iterator<Item = &'static str>,
    ) -> usize {
        let split = S::describe(from, topology, names);
        B::describe(split, topology, names);
        split
    }
}
