// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! One route declaration, interpreted for inspection and execution.

mod annotations;
mod builder;
mod execution;
mod inspection;

pub(crate) use annotations::{Annotation, Metadata};
pub(in crate::framework) use builder::node;
#[cfg(feature = "program-image-execution")]
pub(crate) use builder::{Node, Sequence};
pub(crate) use builder::{NodePlan, Route, edge, fork_join, step_graph};
pub(crate) use execution::{AnnotatedDriver, Run};
pub(crate) use inspection::{Describe, Label};

/// Uses the final path segment as a concise diagnostic label.
fn short_name(name: &'static str) -> &'static str {
    name.rsplit("::").next().unwrap_or(name)
}
