// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! SST tooling: located diagnostics and a syntax checker that is generic over
//! the target machine.
//!
//! - [`source`] — [`SourceFile`], byte [`Span`]s, and 1-based [`Location`]s.
//! - [`diagnostic`] — structured [`Diagnostic`]s (data, not errors) and a
//!   rustc-style renderer that prints the offending lines with carets.
//! - [`check`] — checks that report *every* problem they can: framing
//!   ([`check_framing`]), pluggable section checks ([`check_file`]), and the
//!   typed [`ProgramChecker`] for a `#[composite]` machine
//!   ([`check_program`]). Checking never executes a program.
//!
//! Framing checks need no machine:
//!
//! ```
//! use vihaco_sst::{SourceFile, check_framing};
//!
//! let source = SourceFile::new("program.sst", "sst v2\n.section main\n.section main\n");
//! let report = check_framing(&source);
//! assert_eq!(report.diagnostics[0].to_string(), "3:10: error: duplicate section `main`");
//! assert!(report.render(&source).contains("first defined here"));
//! ```
//!
//! For a machine `M`, `check_program::<M>(&source)` additionally checks every
//! section's header, `.text`, symbols, metadata, and lowering; see the
//! "Checking SST files" section of the SST reference guide.

pub mod check;
pub mod diagnostic;
pub mod source;

pub use check::{
    CheckHeader, Declarations, ProgramChecker, SectionChecker, StandardHeader, check_file,
    check_framing, check_machine, check_program,
};
pub use diagnostic::{Diagnostic, Diagnostics, Label, Report, Severity};
pub use source::{Location, SourceFile, Span};
