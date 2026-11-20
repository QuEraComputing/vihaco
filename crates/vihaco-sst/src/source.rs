// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Source text, byte spans, and line/column locations.
//!
//! These types are shared by every SST tool in this crate: diagnostics point
//! into a [`SourceFile`] with [`Span`]s, and [`Location`]s are derived from it.

use std::fmt;
use std::ops::Range;

/// A half-open byte range `start..end` into a [`SourceFile`]'s text.
///
/// An empty span marks a position, such as where a missing token belongs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// An empty span at `offset`.
    pub const fn point(offset: usize) -> Self {
        Self::new(offset, offset)
    }

    pub const fn len(self) -> usize {
        self.end - self.start
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Move a span relative to a block so it is relative to the whole file.
    pub const fn shift(self, base: usize) -> Self {
        Self::new(base + self.start, base + self.end)
    }

    pub const fn range(self) -> Range<usize> {
        self.start..self.end
    }
}

impl From<Range<usize>> for Span {
    fn from(range: Range<usize>) -> Self {
        Self::new(range.start, range.end)
    }
}

impl From<Span> for Range<usize> {
    fn from(span: Span) -> Self {
        span.range()
    }
}

/// A 1-based line and column. Columns count characters, not bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// A named SST source text with a line index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    name: String,
    text: String,
    /// Byte offset of the start of every line.
    line_starts: Vec<usize>,
}

impl SourceFile {
    /// Index `text` under a display `name`, usually its path.
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(index, _)| index + 1))
            .collect();
        Self {
            name: name.into(),
            text,
            line_starts,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text covered by `span`, clamped to the file.
    pub fn slice(&self, span: Span) -> &str {
        let end = span.end.min(self.text.len());
        &self.text[span.start.min(end)..end]
    }

    /// The line and column of a byte offset (clamped to the end of the file).
    pub fn location(&self, offset: usize) -> Location {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let start = self.line_starts[line];
        Location {
            line: line + 1,
            column: self.text[start..offset].chars().count() + 1,
        }
    }
}
