// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Check SST files and report every problem as a located [`Diagnostic`].
//!
//! Checking is layered:
//!
//! - [`check_framing`] needs no instruction set: it reports every framing
//!   problem in the file (version line, directives, duplicate sections and
//!   blocks, stray content).
//! - [`check_file`] adds section contents through a [`SectionChecker`], the
//!   plug-in point for header and instruction parsers. The helpers in this
//!   module ([`parse_instructions`], [`parse_declarations`],
//!   [`parse_opaque_header`], [`check_global_context`]) turn the standard
//!   parsers' errors into diagnostics for custom checkers.
//! - [`ProgramChecker`] is the typed checker for a `#[composite]` machine `M`:
//!   header and `.text` syntax, symbols, header directives, function bindings,
//!   and (by default) lowering with `MetadataLoweringContext`. The
//!   conveniences [`check_program`] and [`check_machine`] run it over every
//!   section. Nothing is executed.
//! - [`FileChecker`] checks a file whose sections target different machines:
//!   each section with the checker registered for its name, or a default.
//!
//! [`Diagnostic`]: crate::Diagnostic

mod file;
mod framing;
mod metadata;
mod parse;
mod program;

pub use file::FileChecker;
pub(crate) use file::{AsSectionChecker, Routed, Routes};
pub use framing::{Block, Framing, Section, check_framing, scan};
pub use parse::{
    check_global_context, parse_declarations, parse_instructions, parse_opaque_header,
};
pub use program::{
    CheckHeader, CheckedMetadata, ContextLowering, Declarations, Lowering, MetadataLowering,
    NoLowering, ProgramChecker, StandardHeader, check_machine, check_program,
};

use crate::diagnostic::{Diagnostics, Report};
use crate::source::SourceFile;

/// Checks the contents of an SST file once its framing is known.
///
/// Implement this to plug custom header or instruction parsers into
/// [`check_file`]; closures `FnMut(&Section, &mut Diagnostics)` implement it
/// too. Sections are visited in file order, including duplicates and sections
/// whose directive was malformed.
pub trait SectionChecker {
    /// Check the `.global` block. The block is empty (and has no directive)
    /// when the file omits it. The default accepts anything.
    fn check_global(&mut self, global: &Block<'_>, out: &mut Diagnostics<'_>) {
        let _ = (global, out);
    }

    /// Check one section.
    fn check_section(&mut self, section: &Section<'_>, out: &mut Diagnostics<'_>);

    /// Called once after every section, for whole-file rules such as required
    /// sections. The default does nothing.
    fn finish(&mut self, framing: &Framing<'_>, out: &mut Diagnostics<'_>) {
        let _ = (framing, out);
    }
}

impl<F> SectionChecker for F
where
    F: FnMut(&Section<'_>, &mut Diagnostics<'_>),
{
    fn check_section(&mut self, section: &Section<'_>, out: &mut Diagnostics<'_>) {
        self(section, out)
    }
}

/// Check framing, then every section with `checker`, and report everything.
pub fn check_file(source: &SourceFile, checker: &mut (impl SectionChecker + ?Sized)) -> Report {
    let mut out = Diagnostics::new(source);
    let framing = scan(source, &mut out);
    checker.check_global(&framing.global, &mut out);
    for section in &framing.sections {
        checker.check_section(section, &mut out);
    }
    checker.finish(&framing, &mut out);
    out.finish()
}
