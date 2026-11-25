// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Project node occurrences and dependencies without executing a route.
//!
//! A path accepts an optional incoming edge and returns its last node ID. A
//! sequence connects its paths using those endpoints. A fork describes both
//! arms and adds a synthetic join. Indices and field names identify occurrences
//! for diagnostics within a projection.

use super::{
    annotations::{Annotations, Metadata},
    builder::{DeliverFork, ForkJoin, Node, Route, Sequence},
    short_name,
};

impl<I, P> Route<I, P> {
    /// Projects the route without requiring a machine, driver, or executable path.
    pub(crate) fn topology(&self) -> Topology
    where
        P: Describe,
    {
        let mut topology = Topology::default();
        P::describe(None, &mut topology);
        topology
    }
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
    /// The node occurrence producing the payload.
    pub(crate) from: usize,

    /// The node occurrence receiving the payload.
    pub(crate) to: usize,

    /// Metadata attached to this connection.
    pub(crate) metadata: Metadata,
}

impl Topology {
    /// Adds a node and connects its incoming edge when it has a predecessor.
    fn vertex(
        &mut self,
        name: &'static str,
        field: Option<&'static str>,
        source_field: Option<&'static str>,
        metadata: Metadata,
        from: Option<(usize, Metadata)>,
    ) -> usize {
        let id = self.nodes.len();
        self.nodes.push(Vertex {
            name,
            field,
            source_field,
            metadata,
        });
        if let Some((from, metadata)) = from {
            self.edges.push(Link {
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
            println!(
                "  {id}  {} {source}{site} {:?}",
                node.name,
                node.metadata.names()
            );
        }
        for link in &self.edges {
            println!("  {} → {}  {:?}", link.from, link.to, link.metadata.names());
        }
    }
}

/// Appends a path to the topology and returns its exit node's index.
///
/// The optional incoming edge connects the previous path's exit to this
/// path's entry. Composite paths use the returned index to connect successors.
pub(crate) trait Describe {
    /// Projects the path and attaches the incoming edge if one was supplied.
    fn describe(from: Option<(usize, Metadata)>, topology: &mut Topology) -> usize;
}

/// Projects one operation and its incoming edge when a predecessor exists.
impl<N: Label, A: Annotations, Execution> Describe for Node<N, A, Execution> {
    fn describe(from: Option<(usize, Metadata)>, topology: &mut Topology) -> usize {
        topology.vertex(N::LABEL, N::field(), N::source_field(), A::metadata(), from)
    }
}

/// Connects the left path’s last node to the right path’s entry node.
impl<L: Describe, E: Annotations, R: Describe> Describe for Sequence<L, E, R> {
    fn describe(from: Option<(usize, Metadata)>, topology: &mut Topology) -> usize {
        let predecessor = L::describe(from, topology);
        R::describe(Some((predecessor, E::metadata())), topology)
    }
}

/// Projects each branch from the same split, keeping their exits in declaration order.
trait DescribeBranches {
    fn describe(split: usize, topology: &mut Topology) -> Vec<usize>;
}

macro_rules! describe_branches {
    ($(($branch:ident, $input:ident, $value:ident, $index:tt)),+) => {
        impl<$($branch: Describe,)+> DescribeBranches for ($($branch,)+) {
            fn describe(split: usize, topology: &mut Topology) -> Vec<usize> {
                vec![$($branch::describe(Some((split, Metadata::default())), topology),)+]
            }
        }
    };
}
for_each_branch_tuple!(describe_branches);

impl<S: Describe, B: DescribeBranches> Describe for ForkJoin<S, B> {
    fn describe(from: Option<(usize, Metadata)>, topology: &mut Topology) -> usize {
        let split = S::describe(from, topology);
        let exits = B::describe(split, topology);
        let join = topology.vertex("join results", None, None, Metadata::default(), None);
        topology.edges.extend(exits.into_iter().map(|from| Link {
            from,
            to: join,
            metadata: Metadata::default(),
        }));
        join
    }
}

impl<S: Describe, B: DescribeBranches> Describe for DeliverFork<S, B> {
    fn describe(from: Option<(usize, Metadata)>, topology: &mut Topology) -> usize {
        let split = S::describe(from, topology);
        B::describe(split, topology);
        // Terminal forks have no successor that can use this index.
        split
    }
}
