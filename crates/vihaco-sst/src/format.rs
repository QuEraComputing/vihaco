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
use vihaco_bytecode::NoContext;
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_module::module::{ProgramMetadata, Symbol};
use vihaco_parser::{Parse, Print};
use vihaco_runtime::HasInstructionSet;

use self::comments::{Comment, blank_line_between, starts_line, user_part};
use self::layout::{INDENT, Line};
use self::notes::SectionInfo;
use crate::check::{
    self, ConstantReferences, Declarations, MetadataDeclaration, MetadataLowering, ProgramChecker,
    SectionChecker,
};
use crate::diagnostic::{Diagnostics, Report};
use crate::source::{SourceFile, Span};

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
    /// The section's `metadata.strings`, or empty without metadata.
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
pub struct Formatter<M, D = ProgramMetadata, L = MetadataLowering, C = NoContext>
where
    M: HasInstructionSet,
{
    checker: ProgramChecker<M, Declarations<D>, L, C>,
    options: FormatOptions,
    annotators: Vec<Box<dyn Annotate<M::Syntax>>>,
    marker: PhantomData<fn() -> D>,
}

impl<M: HasInstructionSet> Formatter<M> {
    /// The standard formatter: `metadata { ... }` headers, checked with
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
        for<'src> M::Syntax: Parse<'src>,
        M::Syntax: Print + 'static,
        for<'src> D: Parse<'src>,
        D: Print + MetadataDeclaration + 'static,
        D::Type: Print,
        D::Value: ConstantReferences,
        ProgramChecker<M, Declarations<D>, L, C>: SectionChecker,
    {
        let report = self.checker.check(source);
        if report.has_errors() {
            return Err(report);
        }
        let mut out = Diagnostics::new(source);
        let Some(model) = read::<M::Syntax, D>(source, &mut out) else {
            return Err(out.finish());
        };
        let mut writer = Writer {
            source,
            options: &self.options,
            annotators: &mut self.annotators,
            out: &mut out,
        };
        let formatted = writer.file(&model);
        if out.error_count() > 0 {
            return Err(out.finish());
        }

        // The formatter must never change meaning: read the output back and
        // compare it with the input.
        let output = SourceFile::new(source.name(), formatted.as_str());
        let mut again = Diagnostics::new(&output);
        let reread = read::<M::Syntax, D>(&output, &mut again);
        let same = self.checker.check(&output).is_ok()
            && reread.is_some_and(|reread| snapshot(&reread) == snapshot(&model));
        if !same {
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
}

/// Format every section of `source` as a program for machine `M` with the
/// standard `metadata { ... }` header.
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
    D: Print + MetadataDeclaration + 'static,
    D::Type: Print + Clone,
    D::Value: ConstantReferences,
{
    Formatter::with_checker(ProgramChecker::<M>::new().with_header::<Declarations<D>>())
        .options(options)
        .format(source)
}

// ---------------------------------------------------------------------------
// Reading: framing, parsed items, and the comments attached to them.

/// A parsed item of a block with the comments that belong to it.
struct Entry<T> {
    value: T,
    span: Span,
    leading: Vec<Comment>,
    trailing: Vec<String>,
    blank_before: bool,
}

/// A header declaration and the comments inside it.
struct Declaration<D> {
    value: D,
    /// Comments inside the declaration, at file offsets.
    inner: Vec<Comment>,
    /// Every comment inside the declaration (including generated ones), as
    /// ranges relative to the declaration.
    raw: Vec<std::ops::Range<usize>>,
}

enum TextItem<I> {
    Symbol(Symbol),
    Instruction(I),
}

struct BlockModel<T> {
    comment: Option<String>,
    entries: Vec<Entry<T>>,
    /// Own-line comments after the last entry.
    tail: Vec<Comment>,
}

enum Block<I, D> {
    Header(BlockModel<Declaration<D>>),
    Text(BlockModel<TextItem<I>>),
}

struct SectionModel<I, D> {
    name: String,
    comment: Option<String>,
    blocks: Vec<Block<I, D>>,
}

struct FileModel<I, D> {
    version_comment: Option<String>,
    /// The `.global` directive comment and normalized body, if declared.
    global: Option<(Option<String>, Vec<String>)>,
    sections: Vec<SectionModel<I, D>>,
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

/// Read a checked file into its formatting model. `None` (with diagnostics in
/// `out`) if the file cannot be parsed.
fn read<I, D>(source: &SourceFile, out: &mut Diagnostics<'_>) -> Option<FileModel<I, D>>
where
    for<'src> I: Parse<'src>,
    I: 'static,
    for<'src> D: Parse<'src>,
    D: 'static,
{
    let text = source.text();
    let framing = check::scan(source, out);
    if out.error_count() > 0 {
        return None;
    }
    let global = framing.global.directive().map(|directive| {
        (
            directive_comment(text, directive),
            global_lines(framing.global.text()),
        )
    });
    let mut sections = Vec::new();
    for section in &framing.sections {
        let mut blocks = Vec::new();
        if let Some(directive) = section.header().directive() {
            let block = section.header();
            let declarations = check::parse_declarations::<D>(block, out)?;
            let items = declarations
                .into_iter()
                .map(|spanned| (Span::from(spanned.span), spanned.value))
                .map(|(span, value)| {
                    let declaration = Declaration {
                        value,
                        inner: Vec::new(),
                        raw: Vec::new(),
                    };
                    (span, declaration)
                })
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
            let before = out.error_count();
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
            let (entries, tail) =
                collect(source, block.span(), items, &[], out, |entry, found, _| {
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
        blocks.sort_by_key(|(start, _)| *start);
        sections.push(SectionModel {
            name: section.name().to_owned(),
            comment: directive_comment(text, section.directive()),
            blocks: blocks.into_iter().map(|(_, block)| block).collect(),
        });
    }
    if out.error_count() > 0 {
        return None;
    }
    Some(FileModel {
        version_comment: directive_comment(text, framing.version),
        global,
        sections,
    })
}

/// A section's name and, per block, whether it is a header and its printed
/// items.
type SectionSnapshot = (String, Vec<(bool, Vec<String>)>);

/// What formatting must preserve: structure, printed values, and the
/// multiset of user comments.
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    global: Option<Vec<String>>,
    sections: Vec<SectionSnapshot>,
    comments: Vec<String>,
}

fn snapshot<I: Print, D: Print>(model: &FileModel<I, D>) -> Snapshot {
    let mut comments: Vec<String> = Vec::new();
    let mut keep = |comment: &Option<String>| comments.extend(comment.iter().cloned());
    keep(&model.version_comment);
    if let Some((comment, _)) = &model.global {
        keep(comment);
    }
    let print = |value: &dyn Fn(&mut String) -> std::fmt::Result| {
        let mut text = String::new();
        value(&mut text).map_or_else(|_| "<unprintable>".to_owned(), |()| text)
    };
    let mut sections = Vec::new();
    for section in &model.sections {
        comments.extend(section.comment.iter().cloned());
        let mut blocks = Vec::new();
        for block in &section.blocks {
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
                            TextItem::Instruction(instruction) => {
                                print(&|out| instruction.print(out))
                            }
                        });
                    }
                    comments.extend(block.tail.iter().map(|c| c.text.clone()));
                    blocks.push((false, values));
                }
            }
        }
        sections.push((section.name.clone(), blocks));
    }
    comments.sort();
    Snapshot {
        global: model.global.as_ref().map(|(_, lines)| lines.clone()),
        sections,
        comments,
    }
}

// ---------------------------------------------------------------------------
// Writing.

/// Gap between code and a trailing comment.
const COMMENT_GAP: usize = 2;

struct Writer<'w, 'src, I> {
    source: &'src SourceFile,
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

impl<I> Writer<'_, '_, I>
where
    for<'src> I: Parse<'src>,
    I: Print + 'static,
{
    fn file<D>(&mut self, model: &FileModel<I, D>) -> String
    where
        for<'src> D: Parse<'src>,
        D: Print + MetadataDeclaration + 'static,
        D::Type: Print,
        D::Value: ConstantReferences,
    {
        let mut output = String::new();
        directive_line("sst v2", &model.version_comment, &mut output);
        if let Some((comment, lines)) = &model.global {
            output.push('\n');
            directive_line(".global", comment, &mut output);
            for line in lines {
                output.push_str(line);
                output.push('\n');
            }
        }
        for section in &model.sections {
            output.push('\n');
            directive_line(
                &format!(".section {}", section.name),
                &section.comment,
                &mut output,
            );
            let metadata = section.blocks.iter().find_map(|block| match block {
                Block::Header(block) => block
                    .entries
                    .iter()
                    .find_map(|entry| entry.value.value.as_metadata()),
                Block::Text(_) => None,
            });
            let symbols: Vec<Symbol> = section
                .blocks
                .iter()
                .filter_map(|block| match block {
                    Block::Text(block) => Some(block),
                    Block::Header(_) => None,
                })
                .flat_map(|block| &block.entries)
                .filter_map(|entry| match &entry.value {
                    TextItem::Symbol(symbol) => Some(symbol.clone()),
                    TextItem::Instruction(_) => None,
                })
                .collect();
            let info = SectionInfo::new(&symbols, metadata);
            for block in &section.blocks {
                // Comments after the last item stay in their block.
                let (directive, comment, lines) = match block {
                    Block::Header(block) => {
                        let mut lines = self.header(block, metadata);
                        push_comments(&mut lines, &block.tail, INDENT);
                        (".header", &block.comment, lines)
                    }
                    Block::Text(block) => {
                        let mut lines = self.text(&section.name, block, &info);
                        push_comments(&mut lines, &block.tail, INDENT);
                        (".text", &block.comment, lines)
                    }
                };
                directive_line(directive, comment, &mut output);
                emit_block(&lines, self.options, &mut output);
            }
        }
        output
    }

    fn header<D>(
        &mut self,
        block: &BlockModel<Declaration<D>>,
        metadata: Option<&ProgramMetadata<D::Value, D::Type, D::Info>>,
    ) -> Vec<Line>
    where
        for<'src> D: Parse<'src>,
        D: Print + MetadataDeclaration + 'static,
        D::Type: Print,
        D::Value: ConstantReferences,
    {
        let mut lines = Vec::new();
        for entry in &block.entries {
            push_comments(&mut lines, &entry.leading, INDENT);
            if entry.blank_before && !lines.is_empty() {
                lines.push(Line::default());
            }
            let Some(mut declaration) = self.declaration(entry, metadata) else {
                continue;
            };
            let mut trailing = declaration.trailing;
            trailing.extend(entry.trailing.iter().cloned());
            let mut trailing = trailing.into_iter();
            let first = trailing.next();
            // One trailing comment per line; the others move above.
            for comment in trailing.chain(declaration.leading) {
                lines.push(Line::new(INDENT, comment));
            }
            if let Some(last) = declaration.lines.last_mut() {
                last.trailing.extend(first);
            }
            lines.append(&mut declaration.lines);
        }
        lines
    }

    /// Lay out one declaration, or report why it cannot be printed.
    fn declaration<D>(
        &mut self,
        entry: &Entry<Declaration<D>>,
        metadata: Option<&ProgramMetadata<D::Value, D::Type, D::Info>>,
    ) -> Option<LaidOut>
    where
        for<'src> D: Parse<'src>,
        D: Print + MetadataDeclaration + 'static,
        D::Type: Print,
        D::Value: ConstantReferences,
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
            lines: vec![Line::new(INDENT, printed.clone())],
        };

        let source_text = self.source.slice(entry.span);
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

        // The standard metadata record is always broken, one field per line.
        if let Some(own) = value.as_metadata()
            && let Some(group) = layout::metadata_record(&mut printed_tree, &printed)
        {
            group.forced = true;
            if self.options.generated_comments && own_metadata(own, metadata) {
                notes::header_notes(own, group, &printed);
            }
        }

        let placement = layout::attach_comments(
            &source_tree,
            source_text,
            &mut printed_tree,
            &printed,
            &relative,
        );
        let Ok(lines) = layout::render(&printed_tree, &printed, INDENT, self.options.max_width)
        else {
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

/// Whether `own` is the section's metadata (the one notes describe).
fn own_metadata<T>(own: &T, metadata: Option<&T>) -> bool {
    metadata.is_some_and(|metadata| std::ptr::eq(own, metadata))
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
fn faithful_declaration<D>(value: &D) -> Result<String, String>
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
fn faithful_instruction<I>(value: &I) -> Result<String, String>
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
