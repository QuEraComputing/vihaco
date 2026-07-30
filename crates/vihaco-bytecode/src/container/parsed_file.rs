// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::ops::Range;

use super::section::SectionNode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFile {
    pub context: Range<usize>,
    pub root: SectionNode,
}
