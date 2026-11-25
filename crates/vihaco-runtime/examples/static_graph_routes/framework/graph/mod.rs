// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! One route declaration, interpreted for inspection and execution.

// Keep tuple arities consistent across construction, inspection, and execution.
macro_rules! for_each_branch_tuple {
    ($implement:ident) => {
        $implement!((A, IA, a, 0), (B, IB, b, 1));
        $implement!((A, IA, a, 0), (B, IB, b, 1), (C, IC, c, 2));
        $implement!((A, IA, a, 0), (B, IB, b, 1), (C, IC, c, 2), (D, ID, d, 3));
        $implement!(
            (A, IA, a, 0),
            (B, IB, b, 1),
            (C, IC, c, 2),
            (D, ID, d, 3),
            (E, IE, e, 4)
        );
        $implement!(
            (A, IA, a, 0),
            (B, IB, b, 1),
            (C, IC, c, 2),
            (D, ID, d, 3),
            (E, IE, e, 4),
            (F, IF, f, 5)
        );
        $implement!(
            (A, IA, a, 0),
            (B, IB, b, 1),
            (C, IC, c, 2),
            (D, ID, d, 3),
            (E, IE, e, 4),
            (F, IF, f, 5),
            (G, IG, g, 6)
        );
        $implement!(
            (A, IA, a, 0),
            (B, IB, b, 1),
            (C, IC, c, 2),
            (D, ID, d, 3),
            (E, IE, e, 4),
            (F, IF, f, 5),
            (G, IG, g, 6),
            (H, IH, h, 7)
        );
    };
}

mod annotations;
mod builder;
mod execution;
mod inspection;

pub(in crate::framework) use annotations::Unannotated;
pub(crate) use annotations::{Annotation, Metadata};
#[cfg(feature = "program-image-execution")]
pub(crate) use builder::Sequence;
pub(in crate::framework) use builder::{
    BindOrigin, EmitsFrom, FieldOrigin, NoInstruction, NoOrigin, OneInstruction, ResolveOperation,
    node,
};
pub(crate) use builder::{Fork, Node, NodePlan, Route, edge, fork_join, split_with, step_graph};
pub(crate) use execution::{AnnotatedDriver, Run};
pub(crate) use inspection::{Describe, Label};

/// Uses the final path segment as a concise diagnostic label.
fn short_name(name: &'static str) -> &'static str {
    name.rsplit("::").next().unwrap_or(name)
}
