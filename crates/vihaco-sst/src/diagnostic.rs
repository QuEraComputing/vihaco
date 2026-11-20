// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Structured, located diagnostics and their human-readable rendering.
//!
//! A [`Diagnostic`] is plain data: a severity, a message, a primary span with
//! its line and column, optional secondary labels, and notes. Checks collect
//! them in a [`Diagnostics`] sink and return a [`Report`]; nothing here is an
//! error value. [`Diagnostic::render`] prints the offending source lines with
//! carets, in the style of rustc:
//!
//! ```text
//! error: duplicate symbol `@main`
//!    --> program.sst:15:1
//!    |
//! 13 | @main:
//!    | ------ first defined here
//! 14 |     t.push 1
//! 15 | @main: t.halt
//!    | ^^^^^^ redefined here
//!    |
//!    = symbol names must be unique within a section
//! ```

use std::fmt;

use codespan_reporting::diagnostic as codespan;
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::{self, Chars, Config};

use crate::source::{Location, SourceFile, Span};

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The file would be rejected by its loader.
    Error,
    /// The file loads, but probably not as intended.
    Warning,
    /// Additional information.
    Note,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        })
    }
}

/// A secondary location attached to a diagnostic, such as the first
/// definition of a duplicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

/// One located problem in a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// The headline, for example ``expected an instruction, found `foo` ``.
    pub message: String,
    /// The primary offending text, as file byte offsets.
    pub span: Span,
    /// The 1-based line and column of `span.start`.
    pub location: Location,
    /// A short annotation printed under the primary span; may be empty.
    pub label: String,
    /// Related locations, such as "first defined here".
    pub secondary: Vec<Label>,
    /// Free-form notes and suggestions printed after the snippet.
    pub notes: Vec<String>,
}

impl Diagnostic {
    /// A diagnostic at `span` in `source`.
    pub fn new(
        severity: Severity,
        source: &SourceFile,
        span: impl Into<Span>,
        message: impl Into<String>,
    ) -> Self {
        let span = span.into();
        Self {
            severity,
            message: message.into(),
            location: source.location(span.start),
            span,
            label: String::new(),
            secondary: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn error(source: &SourceFile, span: impl Into<Span>, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, source, span, message)
    }

    pub fn warning(source: &SourceFile, span: impl Into<Span>, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, source, span, message)
    }

    /// Annotate the primary span.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Add a related location.
    #[must_use]
    pub fn with_secondary(mut self, span: impl Into<Span>, message: impl Into<String>) -> Self {
        self.secondary.push(Label {
            span: span.into(),
            message: message.into(),
        });
        self
    }

    /// Add a note printed after the snippet.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Render this diagnostic with the offending source lines, carets under
    /// the primary span and dashes under secondary spans. The output is plain
    /// text (no color) and ends with a blank line.
    pub fn render(&self, source: &SourceFile) -> String {
        let file = SimpleFile::new(source.name(), source.text());
        let clamp = |span: Span| {
            let end = span.end.min(source.text().len());
            span.start.min(end)..end
        };
        let severity = match self.severity {
            Severity::Error => codespan::Severity::Error,
            Severity::Warning => codespan::Severity::Warning,
            Severity::Note => codespan::Severity::Note,
        };
        let labels = std::iter::once(
            codespan::Label::primary((), clamp(self.span)).with_message(&self.label),
        )
        .chain(self.secondary.iter().map(|label| {
            codespan::Label::secondary((), clamp(label.span)).with_message(&label.message)
        }))
        .collect();
        let diagnostic = codespan::Diagnostic::new(severity)
            .with_message(&self.message)
            .with_labels(labels)
            .with_notes(self.notes.clone());
        let config = Config {
            chars: Chars::ascii(),
            ..Config::default()
        };
        let mut output = String::new();
        // Spans are clamped to the file, so the single file is always found.
        if term::emit_to_string(&mut output, &config, &file, &diagnostic).is_err() {
            output = format!("{}: {self}\n", source.name());
        }
        output
    }
}

/// `line:column: severity: message`, for compact listings and logs.
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.location, self.severity, self.message)
    }
}

/// A diagnostic sink bound to the source file being checked.
///
/// Checkers push into it; [`finish`](Self::finish) produces a [`Report`].
#[derive(Debug)]
pub struct Diagnostics<'a> {
    source: &'a SourceFile,
    items: Vec<Diagnostic>,
}

impl<'a> Diagnostics<'a> {
    pub fn new(source: &'a SourceFile) -> Self {
        Self {
            source,
            items: Vec::new(),
        }
    }

    /// The file being checked.
    pub fn source(&self) -> &'a SourceFile {
        self.source
    }

    /// Build (without recording) an error in this file, to be decorated and
    /// then passed to [`push`](Self::push).
    pub fn build_error(&self, span: impl Into<Span>, message: impl Into<String>) -> Diagnostic {
        Diagnostic::error(self.source, span, message)
    }

    /// Record an error with a primary label.
    pub fn error(
        &mut self,
        span: impl Into<Span>,
        message: impl Into<String>,
        label: impl Into<String>,
    ) {
        self.push(self.build_error(span, message).with_label(label));
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.items.push(diagnostic);
    }

    /// The number of errors recorded so far.
    pub fn error_count(&self) -> usize {
        self.items.iter().filter(|item| item.is_error()).count()
    }

    /// The diagnostics recorded so far, in recording order.
    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.items
    }

    /// Sort the diagnostics by position and produce the final report.
    pub fn finish(self) -> Report {
        Report::new(self.items)
    }
}

/// The result of checking a file: every diagnostic, ordered by position.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    /// A report of `diagnostics`, stably sorted by position.
    pub fn new(mut diagnostics: Vec<Diagnostic>) -> Self {
        diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
        Self { diagnostics }
    }

    /// Whether the file produced no errors (warnings and notes are allowed).
    pub fn is_ok(&self) -> bool {
        !self.has_errors()
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|item| item.is_error())
            .count()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Diagnostic> {
        self.diagnostics.iter()
    }

    /// Render every diagnostic, followed by a summary line when there are
    /// errors.
    pub fn render(&self, source: &SourceFile) -> String {
        let mut output: String = self
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.render(source))
            .collect();
        match self.error_count() {
            0 => {}
            1 => output.push_str(&format!("error: `{}` has 1 error\n", source.name())),
            n => output.push_str(&format!("error: `{}` has {n} errors\n", source.name())),
        }
        output
    }

    /// Convert to an `eyre` error carrying the rendered report when there are
    /// errors, for callers that want to stop on a malformed file.
    pub fn into_result(self, source: &SourceFile) -> eyre::Result<()> {
        if self.has_errors() {
            Err(eyre::eyre!("{}", self.render(source)))
        } else {
            Ok(())
        }
    }
}

impl<'r> IntoIterator for &'r Report {
    type Item = &'r Diagnostic;
    type IntoIter = std::slice::Iter<'r, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl IntoIterator for Report {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.into_iter()
    }
}
