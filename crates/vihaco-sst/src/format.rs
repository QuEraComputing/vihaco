// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Format SST files into a canonical layout, keeping user comments and
//! regenerating `;;` annotations.
//!
//! Formatting never changes what a file means. The file is checked first
//! (with a [`ProgramChecker`]) and a file with errors is returned as its
//! [`Report`]. Instructions and header declarations are re-spelled with
//! their [`Print`] implementations; a value that cannot be printed faithfully
//! is reported instead of being altered. Finally the output is parsed again
//! and compared with the input: the same sections, headers, code, symbols,
//! and user comments.
//!
//! Comments come in two kinds:
//!
//! - Own-line comments starting with `;;` (but not `;;;`) are generated. The formatter
//!   removes them and writes fresh ones, one line per note directly above the
//!   item it describes, so formatting is idempotent. Turn them off with
//!   [`FormatOptions::generated_comments`].
//! - Every other comment is the user's. Each one is kept in its block,
//!   attached to the item it annotates: a comment on its own line stays above
//!   the next item, and a trailing comment stays at the end of its item's line.
//!
//! The layout rules are described in the "Formatting SST files" section of the
//! SST reference guide.

mod comments;
mod layout;
mod notes;

use std::marker::PhantomData;

use vihaco_abi::Lower;
use vihaco_bytecode::{NoContext, SSTGlobalContext};
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_module::module::{
    ConstantValue, NoInfo, ProgramDirective, ProgramHeader, StandardDirective, Symbol,
};
use vihaco_parser::{Parse, Print};
use vihaco_runtime::HasInstructionSet;

use self::comments::{Comment, blank_line_between, starts_line, user_part};
use self::layout::{INDENT, Line};
use self::notes::SectionInfo;
use crate::check::{
    self, AsSectionChecker, Declarations, MetadataLowering, ProgramChecker, Routed, Routes,
    SectionChecker,
};
use crate::diagnostic::{Diagnostics, Report};
use crate::source::{SourceFile, Span};

/// Indentation of header directives: one level under `.header`, like
/// instructions under `.text`. Framing directives and labels stay at column 0.
const HEADER_INDENT: usize = INDENT;

/// Formatter settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOptions {
    /// Write generated `;;` comments. Existing own-line `;;` comments are
    /// removed either way. Default: `true`.
    pub generated_comments: bool,
    /// The line width that header records are broken to fit. Default: `80`.
    pub max_width: usize,
    /// The furthest column at which trailing comments are aligned; lines
    /// longer than this put their comment two spaces after the code.
    /// Default: `60`.
    pub comment_column: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            generated_comments: true,
            max_width: 80,
            comment_column: 60,
        }
    }
}

impl FormatOptions {
    /// The defaults without generated comments.
    pub fn without_generated_comments() -> Self {
        Self {
            generated_comments: false,
            ..Self::default()
        }
    }
}

/// Where an instruction sits, for machine-specific [`Annotate`] hooks.
#[derive(Debug, Clone, Copy)]
pub struct InstructionContext<'a> {
    /// The section being formatted.
    pub section: &'a str,
    /// The instruction's address within the section.
    pub address: u32,
    /// The section's symbol table, in definition order.
    pub symbols: &'a [Symbol],
    /// The section's string table as lowering builds it: the strings the
    /// header derives, then the string literals of `.text` in address order.
    pub strings: &'a [String],
}

/// A hook that adds machine-specific generated notes to instructions.
///
/// Each returned string becomes a generated `;;` line above the instruction,
/// after the formatter's own notes. Closures
/// `FnMut(&I, &InstructionContext) -> Vec<String>` implement it.
pub trait Annotate<I> {
    fn annotate(&mut self, instruction: &I, context: &InstructionContext<'_>) -> Vec<String>;
}

impl<I, F> Annotate<I> for F
where
    F: FnMut(&I, &InstructionContext<'_>) -> Vec<String>,
{
    fn annotate(&mut self, instruction: &I, context: &InstructionContext<'_>) -> Vec<String> {
        self(instruction, context)
    }
}

/// Formats SST files for machine `M` whose headers are declarations `D`, one
/// per line.
///
/// The checker that gates formatting is a [`ProgramChecker`] with header
/// `Declarations<D>`, lowering `L`, and global context `C`; replace it with
/// [`with_checker`](Self::with_checker).
pub struct Formatter<M, D = ProgramDirective, L = MetadataLowering, C = NoContext>
where
    M: HasInstructionSet,
{
    checker: ProgramChecker<M, Declarations<D>, L, C>,
    options: FormatOptions,
    annotators: Vec<Box<dyn Annotate<M::Syntax>>>,
    marker: PhantomData<fn() -> D>,
}

impl<M: HasInstructionSet> Formatter<M> {
    /// The standard formatter: standard directive headers, checked with
    /// `ProgramChecker::<M>::new()`, and default options.
    pub fn new() -> Self {
        Self::with_checker(ProgramChecker::new())
    }
}

impl<M: HasInstructionSet> Default for Formatter<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M, D, L, C> Formatter<M, D, L, C>
where
    M: HasInstructionSet,
{
    /// A formatter gated by `checker`, for example
    /// `ProgramChecker::<M>::new().with_header::<Declarations<MyDeclaration>>()`.
    pub fn with_checker(checker: ProgramChecker<M, Declarations<D>, L, C>) -> Self {
        Self {
            checker,
            options: FormatOptions::default(),
            annotators: Vec::new(),
            marker: PhantomData,
        }
    }

    /// Use `options`.
    #[must_use]
    pub fn options(mut self, options: FormatOptions) -> Self {
        self.options = options;
        self
    }

    /// Add a hook for machine-specific instruction notes.
    #[must_use]
    pub fn with_annotator(mut self, annotator: impl Annotate<M::Syntax> + 'static) -> Self {
        self.annotators.push(Box::new(annotator));
        self
    }

    /// Format `source`, or return the report of everything that prevents it
    /// from being formatted without changing its meaning.
    pub fn format(&mut self, source: &SourceFile) -> Result<String, Report>
    where
        Self: SectionFormat,
    {
        let options = self.options.clone();
        format_file(&mut Single(self), source, &options)
    }
}

/// Format every section of `source` as a program for machine `M` with the
/// standard directive header.
pub fn format_program<M>(source: &SourceFile, options: FormatOptions) -> Result<String, Report>
where
    M: HasInstructionSet,
    for<'src> M::Syntax: Parse<'src>,
    M::Syntax: Print + Lower<MetadataLoweringContext> + 'static,
{
    Formatter::<M>::new().options(options).format(source)
}

/// Format every section of `source` as a program for machine `M` with header
/// declarations `D`, checked with `MetadataLoweringContext` lowering.
pub fn format_machine<M, D>(source: &SourceFile, options: FormatOptions) -> Result<String, Report>
where
    M: HasInstructionSet,
    for<'src> M::Syntax: Parse<'src>,
    M::Syntax: Print + Lower<MetadataLoweringContext> + 'static,
    for<'src> D: Parse<'src>,
    D: Print + StandardDirective + 'static,
    D::Type: Print + Clone,
    D::Value: ConstantValue + Clone,
{
    Formatter::with_checker(ProgramChecker::<M>::new().with_header::<Declarations<D>>())
        .options(options)
        .format(source)
}

/// Formats a file whose sections may target different machines.
///
/// Like [`FileChecker`](crate::FileChecker), each section is formatted (and
/// checked first) by the [`Formatter`] registered for its name, or by the
/// default one; a section with neither is an error, as is a registered
/// section that the file does not contain. The `.global` block is checked as
/// `C` and kept verbatim. The [`FormatOptions`] are the file's; the options
/// of the registered formatters are not used.
///
/// ```
/// # use vihaco_sst::{FileFormatter, FormatOptions, Formatter, SourceFile, format::SectionFormat};
/// # fn demo<Controller, Fpga>(source: &SourceFile) -> Result<String, vihaco_sst::Report>
/// # where
/// #     Controller: vihaco_runtime::HasInstructionSet + 'static,
/// #     Fpga: vihaco_runtime::HasInstructionSet + 'static,
/// #     Formatter<Controller>: SectionFormat,
/// #     Formatter<Fpga>: SectionFormat,
/// # {
/// FileFormatter::new()
///     .section::<Controller>("controller")
///     .section::<Fpga>("fpga")
///     .options(FormatOptions::default())
///     .format(source)
/// # }
/// ```
pub struct FileFormatter<C = NoContext> {
    routes: Routes<dyn SectionFormat>,
    options: FormatOptions,
    global: PhantomData<fn() -> C>,
}

impl FileFormatter {
    /// A formatter with no sections registered, for a file without a
    /// `.global` context, with default options.
    pub fn new() -> Self {
        Self {
            routes: Routes::default(),
            options: FormatOptions::default(),
            global: PhantomData,
        }
    }
}

impl Default for FileFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> FileFormatter<C> {
    /// Format section `name` as a program for machine `M`, with the standard
    /// formatter `Formatter::<M>::new()`. The section is required.
    #[must_use]
    pub fn section<M>(self, name: impl Into<String>) -> Self
    where
        M: HasInstructionSet + 'static,
        Formatter<M>: SectionFormat,
    {
        self.section_with(name, Formatter::<M>::new())
    }

    /// Format section `name` with `formatter`, for example one with an
    /// application header or annotators. The section is required.
    #[must_use]
    pub fn section_with(
        mut self,
        name: impl Into<String>,
        formatter: impl SectionFormat + 'static,
    ) -> Self {
        self.routes.insert(name.into(), Box::new(formatter));
        self
    }

    /// Format every section without its own formatter as a program for `M`.
    #[must_use]
    pub fn default_section<M>(self) -> Self
    where
        M: HasInstructionSet + 'static,
        Formatter<M>: SectionFormat,
    {
        self.default_with(Formatter::<M>::new())
    }

    /// Format every section without its own formatter with `formatter`.
    #[must_use]
    pub fn default_with(mut self, formatter: impl SectionFormat + 'static) -> Self {
        self.routes.set_default(Box::new(formatter));
        self
    }

    /// Use `options`.
    #[must_use]
    pub fn options(mut self, options: FormatOptions) -> Self {
        self.options = options;
        self
    }

    /// Check and parse the `.global` block as `C2`.
    pub fn with_global_context<C2>(self) -> FileFormatter<C2> {
        FileFormatter {
            routes: self.routes,
            options: self.options,
            global: PhantomData,
        }
    }

    /// Format `source`, or return the report of everything that prevents it
    /// from being formatted without changing its meaning.
    pub fn format(&mut self, source: &SourceFile) -> Result<String, Report>
    where
        C: SSTGlobalContext,
    {
        let options = self.options.clone();
        format_file(self, source, &options)
    }
}

mod sealed {
    pub trait Sealed {}
    impl<M: vihaco_runtime::HasInstructionSet, D, L, C> Sealed for super::Formatter<M, D, L, C> {}
}

/// A [`Formatter`] for one machine, as one section of a
/// [`FileFormatter`] or a [`Disassembler`](crate::Disassembler).
///
/// Implemented by every `Formatter<M, D, L, C>` that can format; it cannot
/// be implemented outside this crate. Its methods are internal.
pub trait SectionFormat: sealed::Sealed {
    #[doc(hidden)]
    fn checker(&mut self) -> &mut dyn SectionChecker;

    /// Read `section`, write it in the canonical layout, and snapshot it.
    #[doc(hidden)]
    fn format_section(
        &mut self,
        source: &SourceFile,
        section: &check::Section<'_>,
        options: &FormatOptions,
        out: &mut Diagnostics<'_>,
    ) -> Option<FormattedSection>;

    /// Read `section` and snapshot it.
    #[doc(hidden)]
    fn snapshot_section(
        &self,
        source: &SourceFile,
        section: &check::Section<'_>,
        out: &mut Diagnostics<'_>,
    ) -> Option<SectionSnapshot>;
}

impl<M, D, L, C> SectionFormat for Formatter<M, D, L, C>
where
    M: HasInstructionSet,
    for<'src> M::Syntax: Parse<'src>,
    M::Syntax: Print + 'static,
    for<'src> D: Parse<'src>,
    D: Print + StandardDirective + 'static,
    D::Type: Print + Clone,
    D::Value: ConstantValue + Clone,
    ProgramChecker<M, Declarations<D>, L, C>: SectionChecker,
{
    fn checker(&mut self) -> &mut dyn SectionChecker {
        &mut self.checker
    }

    fn format_section(
        &mut self,
        source: &SourceFile,
        section: &check::Section<'_>,
        options: &FormatOptions,
        out: &mut Diagnostics<'_>,
    ) -> Option<FormattedSection> {
        let blocks = read_blocks::<M::Syntax, D>(source, section, out)?;
        Some(self.write_blocks(source, section.name(), &blocks, options, out))
    }

    fn snapshot_section(
        &self,
        source: &SourceFile,
        section: &check::Section<'_>,
        out: &mut Diagnostics<'_>,
    ) -> Option<SectionSnapshot> {
        let blocks = read_blocks::<M::Syntax, D>(source, section, out)?;
        Some(snapshot_blocks(&blocks))
    }
}

impl<M, D, L, C> Formatter<M, D, L, C>
where
    M: HasInstructionSet,
    for<'src> M::Syntax: Parse<'src>,
    M::Syntax: Print + 'static,
{
    /// Write `blocks`, the body of section `name`, with this formatter's
    /// annotators, and snapshot them. Problems are reported to `out`.
    pub(crate) fn write_blocks(
        &mut self,
        source: &SourceFile,
        name: &str,
        blocks: &[Block<M::Syntax, D>],
        options: &FormatOptions,
        out: &mut Diagnostics<'_>,
    ) -> FormattedSection
    where
        for<'src> D: Parse<'src>,
        D: Print + StandardDirective + 'static,
        D::Type: Print + Clone,
        D::Value: ConstantValue + Clone,
    {
        let mut writer = Writer {
            source,
            options,
            annotators: &mut self.annotators,
            out,
        };
        FormattedSection {
            text: writer.section(name, blocks),
            snapshot: snapshot_blocks(blocks),
        }
    }
}

/// A section written in the canonical layout: everything after its
/// `.section` line and loose comments.
#[doc(hidden)]
pub struct FormattedSection {
    pub(crate) text: String,
    pub(crate) snapshot: SectionSnapshot,
}

/// Whether a block is a header, and its printed items.
type BlockSnapshot = (bool, Vec<String>);

/// What formatting must preserve of one section: per block, whether it is a
/// header and its printed items; and the section's user comments.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionSnapshot {
    blocks: Vec<BlockSnapshot>,
    comments: Vec<String>,
}

/// How a file is checked and its sections formatted.
pub(crate) trait FileRoutes {
    /// Check the whole file.
    fn check(&mut self, source: &SourceFile) -> Report;
    /// The formatter for section `name`.
    fn route(&mut self, name: &str) -> Option<&mut dyn SectionFormat>;
}

/// One formatter for every section.
struct Single<'f, F>(&'f mut F);

impl<M, D, L, C> FileRoutes for Single<'_, Formatter<M, D, L, C>>
where
    M: HasInstructionSet,
    Formatter<M, D, L, C>: SectionFormat,
{
    fn check(&mut self, source: &SourceFile) -> Report {
        check::check_file(source, self.0.checker())
    }

    fn route(&mut self, _: &str) -> Option<&mut dyn SectionFormat> {
        Some(self.0)
    }
}

impl<C: SSTGlobalContext> FileRoutes for FileFormatter<C> {
    fn check(&mut self, source: &SourceFile) -> Report {
        check::check_file(
            source,
            &mut Routed::<_, C> {
                routes: &mut self.routes,
                global: PhantomData,
            },
        )
    }

    fn route(&mut self, name: &str) -> Option<&mut dyn SectionFormat> {
        let formatter: &mut dyn SectionFormat = self.routes.get_mut(name)?;
        Some(formatter)
    }
}

impl AsSectionChecker for dyn SectionFormat {
    fn as_checker(&mut self) -> &mut dyn SectionChecker {
        self.checker()
    }
}

/// Check `source`, format every section with its routed formatter, and
/// verify that the output means the same as the input.
fn format_file(
    routes: &mut dyn FileRoutes,
    source: &SourceFile,
    options: &FormatOptions,
) -> Result<String, Report> {
    let report = routes.check(source);
    if report.has_errors() {
        return Err(report);
    }
    let mut out = Diagnostics::new(source);
    let framing = check::scan(source, &mut out);
    if out.error_count() > 0 {
        return Err(out.finish());
    }
    let frame = read_frame(source, &framing);
    let mut bodies = Vec::new();
    let mut sections = Vec::new();
    for section in &framing.sections {
        let Some(formatter) = routes.route(section.name()) else {
            out.error(
                section.name_span(),
                format!("no machine is registered for section `{}`", section.name()),
                "unknown section",
            );
            continue;
        };
        if let Some(formatted) = formatter.format_section(source, section, options, &mut out) {
            bodies.push(formatted.text);
            sections.push(formatted.snapshot);
        }
    }
    if out.error_count() > 0 {
        return Err(out.finish());
    }
    let formatted = write_file(&frame, &bodies);

    // The formatter must never change meaning: read the output back and
    // compare it with the input.
    let expected = snapshot(&frame, sections);
    let output = SourceFile::new(source.name(), formatted.as_str());
    if !reads_back_as(routes, &output, &expected) {
        let mut out = Diagnostics::new(source);
        let diagnostic = out
            .build_error(
                Span::point(0),
                "formatting would change the meaning of this file",
            )
            .with_label("in this file")
            .with_note("this is a formatter bug; the file was left unchanged");
        out.push(diagnostic);
        return Err(out.finish());
    }
    Ok(formatted)
}

/// Whether `output` checks clean and reads back to `expected`.
pub(crate) fn reads_back_as(
    routes: &mut dyn FileRoutes,
    output: &SourceFile,
    expected: &Snapshot,
) -> bool {
    if !routes.check(output).is_ok() {
        return false;
    }
    let mut out = Diagnostics::new(output);
    let framing = check::scan(output, &mut out);
    let frame = read_frame(output, &framing);
    let mut sections = Vec::new();
    for section in &framing.sections {
        let Some(formatter) = routes.route(section.name()) else {
            return false;
        };
        match formatter.snapshot_section(output, section, &mut out) {
            Some(snapshot) => sections.push(snapshot),
            None => return false,
        }
    }
    out.error_count() == 0 && snapshot(&frame, sections) == *expected
}

// ---------------------------------------------------------------------------
// Reading: framing, parsed items, and the comments attached to them.

/// A parsed item of a block with the comments that belong to it.
pub(crate) struct Entry<T> {
    pub value: T,
    pub span: Span,
    pub leading: Vec<Comment>,
    pub trailing: Vec<String>,
    pub blank_before: bool,
}

impl<T> Entry<T> {
    /// An item with no source position and no comments, as a disassembler
    /// produces it.
    pub(crate) fn bare(value: T) -> Self {
        Self {
            value,
            span: Span::point(0),
            leading: Vec::new(),
            trailing: Vec::new(),
            blank_before: false,
        }
    }
}

/// A header declaration and the comments inside it.
pub(crate) struct Declaration<D> {
    pub value: D,
    /// Comments inside the declaration, at file offsets.
    pub inner: Vec<Comment>,
    /// Every comment inside the declaration (including generated ones), as
    /// ranges relative to the declaration.
    pub raw: Vec<std::ops::Range<usize>>,
}

impl<D> Declaration<D> {
    /// A declaration without comments.
    pub(crate) fn new(value: D) -> Self {
        Self {
            value,
            inner: Vec::new(),
            raw: Vec::new(),
        }
    }
}

pub(crate) enum TextItem<I> {
    Symbol(Symbol),
    Instruction(I),
}

pub(crate) struct BlockModel<T> {
    pub comment: Option<String>,
    pub entries: Vec<Entry<T>>,
    /// Own-line comments after the last entry.
    pub tail: Vec<Comment>,
}

impl<T> BlockModel<T> {
    /// A block of items with no comments.
    pub(crate) fn bare(items: impl IntoIterator<Item = T>) -> Self {
        Self {
            comment: None,
            entries: items.into_iter().map(Entry::bare).collect(),
            tail: Vec::new(),
        }
    }
}

/// One block of a section's formatting model. A model read from source
/// carries its comments; one built by a disassembler has none.
pub(crate) enum Block<I, D> {
    Header(BlockModel<Declaration<D>>),
    Text(BlockModel<TextItem<I>>),
}

/// The comment after a directive on its line, if any.
fn directive_comment(text: &str, directive: Span) -> Option<String> {
    let end = text[directive.end..]
        .find('\n')
        .map_or(text.len(), |index| directive.end + index);
    let rest = text[directive.end..end].trim();
    rest.starts_with(';')
        .then(|| user_part(rest, false))
        .flatten()
}

/// The end of the line holding `offset`, excluding the line terminator.
fn line_end(text: &str, offset: usize) -> usize {
    text[offset..]
        .find('\n')
        .map_or(text.len(), |index| offset + index)
}

/// The user comments on comment-only lines of `text[from..to]`, a range that
/// lies outside every block. Generated `;;` lines are dropped. Blank lines
/// between comments are kept as one empty string; leading and trailing blank
/// lines are not.
fn loose_comments(text: &str, from: usize, to: usize) -> Vec<String> {
    let mut comments = Vec::new();
    let mut blank = false;
    for line in text
        .get(from..to)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
    {
        if line.is_empty() {
            blank = true;
        } else if let Some(comment) = user_part(line, true) {
            if blank && !comments.is_empty() {
                comments.push(String::new());
            }
            blank = false;
            comments.push(comment);
        }
    }
    comments
}

/// The `.global` body as lines without trailing whitespace or trailing blank
/// lines.
fn global_lines(body: &str) -> Vec<String> {
    let mut lines: Vec<String> = body
        .lines()
        .map(|line| line.trim_end().to_owned())
        .collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

/// Attach the comments of `body` to its parsed `items` (sorted by start).
///
/// `inner` decides what happens to comments inside an item: for header
/// declarations they are kept with the declaration, for `.text` items they
/// become trailing comments.
fn collect<T>(
    source: &SourceFile,
    body: Span,
    items: Vec<(Span, T)>,
    allowed: &[char],
    out: &mut Diagnostics<'_>,
    mut inner: impl FnMut(&mut Entry<T>, Vec<Comment>, Vec<std::ops::Range<usize>>),
) -> (Vec<Entry<T>>, Vec<Comment>) {
    let text = source.text();
    let mut entries: Vec<Entry<T>> = Vec::new();
    let mut pending = Vec::new();
    let mut previous_end: Option<usize> = None;
    let gap = |range: std::ops::Range<usize>,
               entries: &mut Vec<Entry<T>>,
               pending: &mut Vec<Comment>,
               previous_end: &mut Option<usize>,
               out: &mut Diagnostics<'_>| {
        let scan = comments::scan(text, range, false, allowed);
        if let Some(offset) = scan.unexpected {
            out.error(
                Span::new(offset, offset + 1),
                "cannot format: unexpected text between items",
                "not part of any item",
            );
        }
        for raw in scan.comments {
            let own_line = starts_line(text, raw.start);
            let Some(user) = user_part(&text[raw.clone()], own_line) else {
                continue;
            };
            if !own_line && let Some(entry) = entries.last_mut() {
                entry.trailing.push(user);
                continue;
            }
            pending.push(Comment {
                text: user,
                start: raw.start,
                own_line: true,
                blank_before: previous_end
                    .is_some_and(|end| blank_line_between(text, end, raw.start)),
            });
            *previous_end = Some(raw.end);
        }
    };
    let mut cursor = body.start;
    for (span, value) in items {
        gap(
            cursor..span.start,
            &mut entries,
            &mut pending,
            &mut previous_end,
            out,
        );
        let mut entry = Entry {
            value,
            span,
            leading: std::mem::take(&mut pending),
            trailing: Vec::new(),
            blank_before: previous_end.is_some_and(|end| blank_line_between(text, end, span.start)),
        };
        let scan = comments::scan(text, span.range(), true, &[]);
        let mut found = Vec::new();
        for raw in &scan.comments {
            let own_line = starts_line(text, raw.start);
            if let Some(user) = user_part(&text[raw.clone()], own_line) {
                found.push(Comment {
                    text: user,
                    start: raw.start,
                    own_line,
                    blank_before: false,
                });
            }
        }
        let relative = scan
            .comments
            .iter()
            .map(|raw| raw.start - span.start..raw.end - span.start)
            .collect();
        inner(&mut entry, found, relative);
        entries.push(entry);
        previous_end = Some(span.end);
        cursor = span.end;
    }
    gap(
        cursor..body.end,
        &mut entries,
        &mut pending,
        &mut previous_end,
        out,
    );
    (entries, pending)
}

/// The comments and structure of a file outside its sections' blocks.
pub(crate) struct FileFrame {
    /// Own-line comments before `sst v2`.
    pub preamble: Vec<String>,
    pub version_comment: Option<String>,
    /// Own-line comments between `sst v2` and the first `.global` or
    /// `.section`.
    pub loose: Vec<String>,
    /// The `.global` directive comment and normalized body, if declared.
    pub global: Option<(Option<String>, Vec<String>)>,
    pub sections: Vec<SectionFrame>,
}

/// A section's directive comment and the comments before its first block.
pub(crate) struct SectionFrame {
    pub name: String,
    pub comment: Option<String>,
    /// Own-line comments between `.section` and its first block.
    pub loose: Vec<String>,
}

impl SectionFrame {
    /// A section with no comments.
    pub(crate) fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            comment: None,
            loose: Vec::new(),
        }
    }
}

/// Read the comments outside every block of a file whose framing is valid.
fn read_frame(source: &SourceFile, framing: &check::Framing<'_>) -> FileFrame {
    let text = source.text();
    let global = framing.global.directive().map(|directive| {
        (
            directive_comment(text, directive),
            global_lines(framing.global.text()),
        )
    });
    let first_after_version = framing
        .global
        .directive()
        .or_else(|| framing.sections.first().map(|section| section.directive()))
        .map_or(text.len(), |span| span.start);
    let loose = loose_comments(
        text,
        line_end(text, framing.version.end),
        first_after_version,
    );
    let sections = framing
        .sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let first_block = [section.header().directive(), section.text().directive()]
                .into_iter()
                .flatten()
                .map(|span| span.start)
                .min()
                .or_else(|| {
                    framing
                        .sections
                        .get(index + 1)
                        .map(|next| next.directive().start)
                })
                .unwrap_or(text.len());
            SectionFrame {
                name: section.name().to_owned(),
                comment: directive_comment(text, section.directive()),
                loose: loose_comments(text, line_end(text, section.directive().end), first_block),
            }
        })
        .collect();
    FileFrame {
        preamble: loose_comments(text, 0, framing.version.start),
        version_comment: directive_comment(text, framing.version),
        loose,
        global,
        sections,
    }
}

/// Read a checked section's blocks into their formatting model, in file
/// order. `None` (with diagnostics in `out`) if a block cannot be parsed.
fn read_blocks<I, D>(
    source: &SourceFile,
    section: &check::Section<'_>,
    out: &mut Diagnostics<'_>,
) -> Option<Vec<Block<I, D>>>
where
    for<'src> I: Parse<'src>,
    I: 'static,
    for<'src> D: Parse<'src>,
    D: 'static,
{
    let text = source.text();
    let before = out.error_count();
    let mut blocks = Vec::new();
    if let Some(directive) = section.header().directive() {
        let block = section.header();
        let declarations = check::parse_declarations::<D>(block, out)?;
        let items = declarations
            .into_iter()
            .map(|spanned| (Span::from(spanned.span), Declaration::new(spanned.value)))
            .collect();
        let (entries, tail) = collect(
            source,
            block.span(),
            items,
            &[],
            out,
            |entry, found, raw| {
                entry.value.inner = found;
                entry.value.raw = raw;
            },
        );
        blocks.push((
            directive.start,
            Block::Header(BlockModel {
                comment: directive_comment(text, directive),
                entries,
                tail,
            }),
        ));
    }
    if let Some(directive) = section.text().directive() {
        let block = section.text();
        let parsed = check::parse_instructions::<I>(block, out);
        if out.error_count() > before {
            return None;
        }
        let mut items: Vec<(Span, TextItem<I>)> = parsed
            .code
            .into_iter()
            .map(|item| (Span::from(item.span), TextItem::Instruction(item.value)))
            .chain(
                parsed
                    .symbols
                    .into_iter()
                    .map(|item| (Span::from(item.span), TextItem::Symbol(item.value))),
            )
            .collect();
        items.sort_by_key(|(span, _)| span.start);
        let (entries, tail) = collect(source, block.span(), items, &[], out, |entry, found, _| {
            entry
                .trailing
                .extend(found.into_iter().map(|comment| comment.text));
        });
        blocks.push((
            directive.start,
            Block::Text(BlockModel {
                comment: directive_comment(text, directive),
                entries,
                tail,
            }),
        ));
    }
    if out.error_count() > before {
        return None;
    }
    blocks.sort_by_key(|(start, _)| *start);
    Some(blocks.into_iter().map(|(_, block)| block).collect())
}

/// What formatting must preserve: structure, printed values, and the
/// multiset of user comments.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
    global: Option<Vec<String>>,
    sections: Vec<(String, Vec<BlockSnapshot>)>,
    comments: Vec<String>,
}

/// The snapshot of a file from its frame and its sections' snapshots.
pub(crate) fn snapshot(frame: &FileFrame, sections: Vec<SectionSnapshot>) -> Snapshot {
    let mut comments: Vec<String> = Vec::new();
    comments.extend(frame.version_comment.iter().cloned());
    if let Some((comment, _)) = &frame.global {
        comments.extend(comment.iter().cloned());
    }
    comments.extend(frame.preamble.iter().filter(|c| !c.is_empty()).cloned());
    comments.extend(frame.loose.iter().filter(|c| !c.is_empty()).cloned());
    let mut blocks = Vec::new();
    for (section, snapshot) in frame.sections.iter().zip(sections) {
        comments.extend(section.comment.iter().cloned());
        comments.extend(section.loose.iter().filter(|c| !c.is_empty()).cloned());
        comments.extend(snapshot.comments);
        blocks.push((section.name.clone(), snapshot.blocks));
    }
    comments.sort();
    Snapshot {
        global: frame.global.as_ref().map(|(_, lines)| lines.clone()),
        sections: blocks,
        comments,
    }
}

/// A section's printed blocks and user comments.
fn snapshot_blocks<I: Print, D: Print>(section: &[Block<I, D>]) -> SectionSnapshot {
    let mut comments: Vec<String> = Vec::new();
    let print = |value: &dyn Fn(&mut String) -> std::fmt::Result| {
        let mut text = String::new();
        value(&mut text).map_or_else(|_| "<unprintable>".to_owned(), |()| text)
    };
    let mut blocks = Vec::new();
    for block in section {
        match block {
            Block::Header(block) => {
                comments.extend(block.comment.iter().cloned());
                let mut values = Vec::new();
                for entry in &block.entries {
                    comments.extend(entry.leading.iter().map(|c| c.text.clone()));
                    comments.extend(entry.trailing.iter().cloned());
                    comments.extend(entry.value.inner.iter().map(|c| c.text.clone()));
                    values.push(print(&|out| entry.value.value.print(out)));
                }
                comments.extend(block.tail.iter().map(|c| c.text.clone()));
                blocks.push((true, values));
            }
            Block::Text(block) => {
                comments.extend(block.comment.iter().cloned());
                let mut values = Vec::new();
                for entry in &block.entries {
                    comments.extend(entry.leading.iter().map(|c| c.text.clone()));
                    comments.extend(entry.trailing.iter().cloned());
                    values.push(match &entry.value {
                        TextItem::Symbol(symbol) => format!("@{}:", symbol.name),
                        TextItem::Instruction(instruction) => print(&|out| instruction.print(out)),
                    });
                }
                comments.extend(block.tail.iter().map(|c| c.text.clone()));
                blocks.push((false, values));
            }
        }
    }
    SectionSnapshot { blocks, comments }
}

// ---------------------------------------------------------------------------
// Writing.

/// Gap between code and a trailing comment.
const COMMENT_GAP: usize = 2;

struct Writer<'w, 'src, I> {
    source: &'w SourceFile,
    options: &'w FormatOptions,
    annotators: &'w mut [Box<dyn Annotate<I>>],
    out: &'w mut Diagnostics<'src>,
}

/// Render a line's trailing user comment.
fn trailing_text(line: &Line) -> String {
    line.trailing.join(" ")
}

/// Write the generated notes of `line`, one `;;` line each, above it.
fn push_notes(line: &Line, output: &mut String) {
    for note in &line.notes {
        output.push_str(&" ".repeat(line.indent));
        output.push_str(";; ");
        output.push_str(note);
        output.push('\n');
    }
}

/// Lines of one block, with trailing comments aligned to a common column.
fn emit_block(lines: &[Line], options: &FormatOptions, output: &mut String) {
    let column = lines
        .iter()
        .filter(|line| line.has_comment())
        .map(|line| line.width() + COMMENT_GAP)
        .filter(|column| *column <= options.comment_column)
        .max();
    for line in lines {
        push_notes(line, output);
        let mut text = String::new();
        if !line.code.is_empty() {
            text.push_str(&" ".repeat(line.indent));
            text.push_str(&line.code);
        }
        if line.has_comment() {
            let width = line.width();
            let target = column
                .filter(|column| *column >= width + COMMENT_GAP)
                .unwrap_or(width + COMMENT_GAP);
            text.push_str(&" ".repeat(target - width));
            text.push_str(&trailing_text(line));
        }
        output.push_str(text.trim_end());
        output.push('\n');
    }
}

/// A directive line with its comment.
fn directive_line(directive: &str, comment: &Option<String>, output: &mut String) {
    output.push_str(directive);
    if let Some(comment) = comment {
        output.push_str(&" ".repeat(COMMENT_GAP));
        output.push_str(comment);
    }
    output.push('\n');
}

/// Push own-line comments, keeping single blank lines between them.
fn push_comments(lines: &mut Vec<Line>, comments: &[Comment], indent: usize) {
    for comment in comments {
        if comment.blank_before && !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(Line::new(indent, comment.text.clone()));
    }
}

/// Write a file from its frame and its sections' written bodies, in order.
pub(crate) fn write_file(frame: &FileFrame, bodies: &[String]) -> String {
    let mut output = String::new();
    let push_lines = |lines: &[String], output: &mut String| {
        for line in lines {
            output.push_str(line);
            output.push('\n');
        }
    };
    push_lines(&frame.preamble, &mut output);
    directive_line("sst v2", &frame.version_comment, &mut output);
    // Loose comments after `sst v2` sit directly above the directive that
    // follows them.
    let mut loose = frame.loose.as_slice();
    if let Some((comment, lines)) = &frame.global {
        output.push('\n');
        push_lines(std::mem::take(&mut loose), &mut output);
        directive_line(".global", comment, &mut output);
        push_lines(lines, &mut output);
    }
    if frame.sections.is_empty() && !loose.is_empty() {
        output.push('\n');
        push_lines(std::mem::take(&mut loose), &mut output);
    }
    for (section, body) in frame.sections.iter().zip(bodies) {
        output.push('\n');
        push_lines(std::mem::take(&mut loose), &mut output);
        directive_line(
            &format!(".section {}", section.name),
            &section.comment,
            &mut output,
        );
        push_lines(&section.loose, &mut output);
        output.push_str(body);
    }
    output
}

impl<I> Writer<'_, '_, I>
where
    for<'src> I: Parse<'src>,
    I: Print + 'static,
{
    /// Write the blocks of section `name`, each with its directive line.
    fn section<D>(&mut self, name: &str, blocks: &[Block<I, D>]) -> String
    where
        for<'src> D: Parse<'src>,
        D: Print + StandardDirective + 'static,
        D::Type: Print + Clone,
        D::Value: ConstantValue + Clone,
    {
        let mut output = String::new();
        let standard: Vec<&ProgramDirective<D::Value, D::Type>> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Header(block) => Some(block),
                Block::Text(_) => None,
            })
            .flat_map(|block| &block.entries)
            .filter_map(|entry| entry.value.value.as_standard())
            .collect();
        let text_items: Vec<&TextItem<I>> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Text(block) => Some(block),
                Block::Header(_) => None,
            })
            .flat_map(|block| &block.entries)
            .map(|entry| &entry.value)
            .collect();
        let symbols: Vec<Symbol> = text_items
            .iter()
            .filter_map(|item| match item {
                TextItem::Symbol(symbol) => Some(symbol.clone()),
                TextItem::Instruction(_) => None,
            })
            .collect();
        let printed: Vec<String> = text_items
            .iter()
            .filter_map(|item| match item {
                TextItem::Instruction(instruction) => instruction.to_source().ok(),
                TextItem::Symbol(_) => None,
            })
            .collect();
        let metadata = standard
            .iter()
            .map(|directive| (*directive).clone())
            .collect::<ProgramHeader<_, _>>()
            .assemble(NoInfo)
            .ok();
        let header_notes = match &metadata {
            Some(metadata) if self.options.generated_comments => {
                notes::header_notes(&standard, metadata, &symbols, printed.len())
            }
            _ => vec![Vec::new(); standard.len()],
        };
        let strings = notes::lowered_strings(
            metadata.as_ref().map_or(&[][..], |m| m.strings.as_slice()),
            printed.iter().map(String::as_str),
        );
        let info = SectionInfo::new(&symbols, metadata.as_ref(), &strings);
        for block in blocks {
            // Comments after the last item stay in their block.
            let (directive, comment, lines) = match block {
                Block::Header(block) => {
                    let mut lines = self.header(block, &header_notes);
                    push_comments(&mut lines, &block.tail, HEADER_INDENT);
                    (".header", &block.comment, lines)
                }
                Block::Text(block) => {
                    let mut lines = self.text(name, block, &info);
                    push_comments(&mut lines, &block.tail, INDENT);
                    (".text", &block.comment, lines)
                }
            };
            directive_line(directive, comment, &mut output);
            emit_block(&lines, self.options, &mut output);
        }
        output
    }

    /// Lay out a header block. `notes` holds the generated notes of each
    /// standard directive of the section, in order.
    fn header<D>(&mut self, block: &BlockModel<Declaration<D>>, notes: &[Vec<String>]) -> Vec<Line>
    where
        for<'src> D: Parse<'src>,
        D: Print + StandardDirective + 'static,
    {
        let mut lines = Vec::new();
        let mut notes = notes.iter();
        for entry in &block.entries {
            let own = match entry.value.value.as_standard() {
                Some(_) => notes.next().cloned().unwrap_or_default(),
                None => Vec::new(),
            };
            push_comments(&mut lines, &entry.leading, HEADER_INDENT);
            if entry.blank_before && !lines.is_empty() {
                lines.push(Line::default());
            }
            let Some(mut declaration) = self.declaration(entry) else {
                continue;
            };
            if let Some(first) = declaration.lines.first_mut() {
                first.notes = own;
            }
            let mut trailing = declaration.trailing;
            trailing.extend(entry.trailing.iter().cloned());
            let mut trailing = trailing.into_iter();
            let first = trailing.next();
            // One trailing comment per line; the others move above.
            for comment in trailing.chain(declaration.leading) {
                lines.push(Line::new(HEADER_INDENT, comment));
            }
            if let Some(last) = declaration.lines.last_mut() {
                last.trailing.extend(first);
            }
            lines.append(&mut declaration.lines);
        }
        lines
    }

    /// Lay out one declaration, or report why it cannot be printed.
    fn declaration<D>(&mut self, entry: &Entry<Declaration<D>>) -> Option<LaidOut>
    where
        for<'src> D: Parse<'src>,
        D: Print + 'static,
    {
        let value = &entry.value.value;
        let printed = match faithful_declaration(value) {
            Ok(printed) => printed,
            Err(message) => {
                self.out
                    .error(entry.span, message, "cannot be printed faithfully");
                return None;
            }
        };
        let hoisted = || {
            entry
                .value
                .inner
                .iter()
                .map(|comment| comment.text.clone())
                .collect::<Vec<_>>()
        };
        let one_line = LaidOut {
            leading: hoisted(),
            trailing: Vec::new(),
            lines: vec![Line::new(HEADER_INDENT, printed.clone())],
        };

        // A declaration without comments (such as a disassembled one) has no
        // source text to attach them from; its printed form stands in.
        let source_text = if entry.value.raw.is_empty() {
            printed.as_str()
        } else {
            self.source.slice(entry.span)
        };
        let relative: Vec<Comment> = entry
            .value
            .inner
            .iter()
            .map(|comment| Comment {
                start: comment.start - entry.span.start,
                ..comment.clone()
            })
            .collect();
        let (Some(source_tree), Some(mut printed_tree)) = (
            layout::parse_tree(source_text, &entry.value.raw),
            layout::parse_tree(&printed, &[]),
        ) else {
            return Some(one_line);
        };
        let placement = layout::attach_comments(
            &source_tree,
            source_text,
            &mut printed_tree,
            &printed,
            &relative,
        );
        let Ok(lines) = layout::render(
            &printed_tree,
            &printed,
            HEADER_INDENT,
            self.options.max_width,
        ) else {
            return Some(one_line);
        };
        if !reparses_as::<D>(&lines, &printed) {
            return Some(one_line);
        }
        Some(LaidOut {
            leading: placement.leading,
            trailing: placement.trailing,
            lines,
        })
    }

    fn text(
        &mut self,
        section: &str,
        block: &BlockModel<TextItem<I>>,
        info: &SectionInfo<'_>,
    ) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut address = 0u32;
        for entry in &block.entries {
            let indent = match entry.value {
                TextItem::Symbol(_) => 0,
                TextItem::Instruction(_) => INDENT,
            };
            push_comments(&mut lines, &entry.leading, indent);
            for comment in entry.trailing.iter().skip(1) {
                lines.push(Line::new(indent, comment.clone()));
            }
            if entry.blank_before && !lines.is_empty() {
                lines.push(Line::default());
            }
            let mut line = match &entry.value {
                TextItem::Symbol(symbol) => {
                    let mut line = Line::new(0, format!("@{}:", symbol.name));
                    if self.options.generated_comments {
                        line.notes.push(info.symbol_note(symbol));
                    }
                    line
                }
                TextItem::Instruction(instruction) => {
                    let printed = match faithful_instruction(instruction) {
                        Ok(printed) => printed,
                        Err(message) => {
                            self.out
                                .error(entry.span, message, "cannot be printed faithfully");
                            address += 1;
                            continue;
                        }
                    };
                    let mut line = Line::new(INDENT, printed);
                    if self.options.generated_comments {
                        line.notes = info.reference_notes(&line.code);
                        line.notes.extend(info.literal_notes(&line.code));
                        let context = InstructionContext {
                            section,
                            address,
                            symbols: info.symbols,
                            strings: info.strings,
                        };
                        for annotator in self.annotators.iter_mut() {
                            line.notes.extend(annotator.annotate(instruction, &context));
                        }
                    }
                    address += 1;
                    line
                }
            };
            line.trailing.extend(entry.trailing.first().cloned());
            lines.push(line);
        }
        lines
    }
}

/// A laid-out declaration and its declaration-level comments.
struct LaidOut {
    leading: Vec<String>,
    trailing: Vec<String>,
    lines: Vec<Line>,
}

const UNPRINTABLE: &str =
    "cannot format: this value has no source spelling that parses back to itself";

/// The printed declaration, checked to parse back to itself.
pub(crate) fn faithful_declaration<D>(value: &D) -> Result<String, String>
where
    for<'src> D: Parse<'src>,
    D: Print + 'static,
{
    let printed = value.to_source().map_err(|_| UNPRINTABLE.to_owned())?;
    let parsed = vihaco_syntax::parse_declarations::<D>(&printed);
    match parsed.declarations.as_slice() {
        [single]
            if parsed.errors.is_empty()
                && single.value.to_source().ok() == Some(printed.clone()) =>
        {
            Ok(printed)
        }
        _ => Err(format!(
            "cannot format: the printed declaration `{printed}` does not parse back to itself"
        )),
    }
}

/// The printed instruction, checked to parse back to itself.
pub(crate) fn faithful_instruction<I>(value: &I) -> Result<String, String>
where
    for<'src> I: Parse<'src>,
    I: Print + 'static,
{
    let printed = value.to_source().map_err(|_| UNPRINTABLE.to_owned())?;
    let parsed = vihaco_syntax::parse_text::<I>(&printed);
    match parsed.code.as_slice() {
        [single]
            if parsed.errors.is_empty()
                && parsed.symbols.is_empty()
                && single.value.to_source().ok() == Some(printed.clone()) =>
        {
            Ok(printed)
        }
        _ => Err(format!(
            "cannot format: the printed instruction `{printed}` does not parse back to itself"
        )),
    }
}

/// Whether the laid-out lines (with their comments) parse back to the
/// declaration printed as `printed`.
fn reparses_as<D>(lines: &[Line], printed: &str) -> bool
where
    for<'src> D: Parse<'src>,
    D: Print + 'static,
{
    let mut text = String::new();
    for line in lines {
        push_notes(line, &mut text);
        text.push_str(&line.code);
        if line.has_comment() {
            text.push_str("  ");
            text.push_str(&trailing_text(line));
        }
        text.push('\n');
    }
    let parsed = vihaco_syntax::parse_declarations::<D>(&text);
    matches!(
        parsed.declarations.as_slice(),
        [single] if parsed.errors.is_empty() && single.value.to_source().ok().as_deref() == Some(printed)
    )
}
