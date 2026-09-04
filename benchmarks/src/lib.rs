// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Repository-only workload discovery, validation, and CPU execution fixtures.

pub mod machine;
mod resolve;
mod workloads;

pub use workloads::{Case, Contract, Native, Workload, discover, validate};
