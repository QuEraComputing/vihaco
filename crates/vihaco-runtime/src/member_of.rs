// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// Membership of a type in a group identified by the marker type `G`.
///
/// A type may belong to multiple groups. Membership does not imply execution
/// support or establish that blanket trait implementations are disjoint.
pub trait MemberOf<G> {}
