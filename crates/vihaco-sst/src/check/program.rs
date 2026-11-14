// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The typed checker for one machine's instruction set.

use std::marker::PhantomData;

use vihaco_abi::Lower;
use vihaco_bytecode::{NoContext, SSTGlobalContext};
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_module::module::{
    ConstantValue, NoInfo, ProgramDirective, ProgramHeader, ProgramMetadata, StandardDirective,
    Symbol,
};
use vihaco_parser::Parse;
use vihaco_runtime::HasInstructionSet;
use vihaco_syntax::Spanned;

use super::metadata;
use super::{
    Framing, Section, SectionChecker, check_file, check_global_context, parse_declarations,
    parse_instructions,
};
use crate::diagnostic::{Diagnostics, Report};
use crate::source::{SourceFile, Span};

/// A section's assembled program metadata, with the file span of each
/// function's `.func` directive (in table order) for locating problems.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckedMetadata<V, Ty> {
    pub metadata: ProgramMetadata<V, Ty>,
    pub functions: Vec<Span>,
}

/// How a [`ProgramChecker`] parses a section header and assembles its
/// metadata.
pub trait CheckHeader: Sized {
    type Value;
    type Type;

    /// Parse the section's `.header` block, reporting syntax errors. Return
    /// `None` if the header is unusable.
    fn parse(section: &Section<'_>, out: &mut Diagnostics<'_>) -> Option<Self>;

    /// Assemble the section's program metadata, reporting every header-level
    /// problem. Return `None` if the metadata cannot be assembled.
    fn metadata(
        &self,
        section: &Section<'_>,
        out: &mut Diagnostics<'_>,
    ) -> Option<CheckedMetadata<Self::Value, Self::Type>>;
}

/// Located header declarations, one per line: the grammar of
/// `vihaco_syntax::HeaderDeclarations<D>`, with spans.
///
/// As a [`CheckHeader`], the standard directives among them (see
/// [`StandardDirective`]) are assembled with `ProgramHeader`; the other
/// declarations are only parsed.
#[derive(Debug, Clone, PartialEq)]
pub struct Declarations<D>(pub Vec<Spanned<D>>);

/// The standard header: only standard directives (`.file`, `.func`,
/// `.entry`, `.const`, `.source_symbol`).
pub type StandardHeader = Declarations<ProgramDirective>;

impl<D> CheckHeader for Declarations<D>
where
    for<'src> D: Parse<'src>,
    D: StandardDirective + 'static,
    D::Value: ConstantValue + Clone,
    D::Type: Clone,
{
    type Value = D::Value;
    type Type = D::Type;

    fn parse(section: &Section<'_>, out: &mut Diagnostics<'_>) -> Option<Self> {
        parse_declarations::<D>(section.header(), out).map(Declarations)
    }

    fn metadata(
        &self,
        _section: &Section<'_>,
        out: &mut Diagnostics<'_>,
    ) -> Option<CheckedMetadata<D::Value, D::Type>> {
        type Located<'d, V, Ty> = (Span, &'d ProgramDirective<V, Ty>);
        let standard: Vec<Located<'_, D::Value, D::Type>> = self
            .0
            .iter()
            .filter_map(|declaration| {
                let directive = declaration.value.as_standard()?;
                Some((Span::from(declaration.span.clone()), directive))
            })
            .collect();
        let assemble = |skip: &[usize]| {
            let mut functions = Vec::new();
            let mut header = ProgramHeader::new();
            for (position, (span, directive)) in standard.iter().enumerate() {
                if skip.contains(&position) {
                    continue;
                }
                if matches!(directive, ProgramDirective::Function(_)) {
                    functions.push(*span);
                }
                header.push((*directive).clone());
            }
            header.assemble(NoInfo).map(|metadata| CheckedMetadata {
                metadata,
                functions,
            })
        };
        match assemble(&[]) {
            Ok(checked) => Some(checked),
            Err(errors) => {
                let spans: Vec<Span> = standard.iter().map(|(span, _)| *span).collect();
                metadata::report_header_errors(&errors, &spans, out);
                // Keep checking `.text` against the directives that were
                // fine, so one bad directive does not hide other problems.
                let bad: Vec<usize> = errors.iter().map(|error| error.directive).collect();
                assemble(&bad).ok()
            }
        }
    }
}

/// Lowers a section's instructions during checking, reporting failures per
/// instruction. Nothing is executed and the lowered code is discarded.
pub trait Lowering<I, V, Ty> {
    /// Lower every instruction in `code`, returning the index and error of
    /// each one that failed. An outer error means no context could be built.
    fn lower_all(
        &mut self,
        metadata: &ProgramMetadata<V, Ty>,
        symbols: &[Symbol],
        code: Vec<I>,
    ) -> eyre::Result<Vec<(usize, eyre::Report)>>;
}

fn lower_each<I: Lower<C>, C>(code: Vec<I>, context: &mut C) -> Vec<(usize, eyre::Report)> {
    code.into_iter()
        .enumerate()
        .filter_map(|(index, instruction)| instruction.lower(context).err().map(|e| (index, e)))
        .collect()
}

/// Skip lowering: check syntax, symbols, and metadata only.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoLowering;

impl<I, V, Ty> Lowering<I, V, Ty> for NoLowering {
    fn lower_all(
        &mut self,
        _: &ProgramMetadata<V, Ty>,
        _: &[Symbol],
        _: Vec<I>,
    ) -> eyre::Result<Vec<(usize, eyre::Report)>> {
        Ok(Vec::new())
    }
}

/// Lower with [`MetadataLoweringContext`], the standard loading path: catches
/// unresolved `@name` operands and other lowering failures.
#[derive(Debug, Clone, Copy, Default)]
pub struct MetadataLowering;

impl<I, V, Ty> Lowering<I, V, Ty> for MetadataLowering
where
    I: Lower<MetadataLoweringContext>,
{
    fn lower_all(
        &mut self,
        metadata: &ProgramMetadata<V, Ty>,
        symbols: &[Symbol],
        code: Vec<I>,
    ) -> eyre::Result<Vec<(usize, eyre::Report)>> {
        let mut context = MetadataLoweringContext::new(metadata, symbols)?;
        Ok(lower_each(code, &mut context))
    }
}

/// Lower with a custom context built from the metadata and symbol table, for
/// loaders that wrap `MetadataLoweringContext` with extra capabilities.
pub struct ContextLowering<F, C> {
    prepare: F,
    context: PhantomData<fn() -> C>,
}

impl<F, C> ContextLowering<F, C> {
    pub fn new(prepare: F) -> Self {
        Self {
            prepare,
            context: PhantomData,
        }
    }
}

impl<I, V, Ty, F, C> Lowering<I, V, Ty> for ContextLowering<F, C>
where
    F: FnMut(&ProgramMetadata<V, Ty>, &[Symbol]) -> eyre::Result<C>,
    I: Lower<C>,
{
    fn lower_all(
        &mut self,
        metadata: &ProgramMetadata<V, Ty>,
        symbols: &[Symbol],
        code: Vec<I>,
    ) -> eyre::Result<Vec<(usize, eyre::Report)>> {
        let mut context = (self.prepare)(metadata, symbols)?;
        Ok(lower_each(code, &mut context))
    }
}

/// The typed checker for machine `M`'s source syntax, `M::Syntax`.
///
/// For each section it reports header and `.text` syntax errors, duplicate
/// symbols, header directive problems (duplicate functions, a bad `.entry`,
/// too few locals, constants naming unknown functions, duplicate source
/// symbols), `.func` directives without a symbol, and (with the default
/// [`MetadataLowering`]) instructions that fail to lower, such as unresolved
/// `@name` operands. It never executes anything.
///
/// - `H` parses the header and assembles the metadata ([`StandardHeader`] by
///   default; see [`with_header`](Self::with_header)).
/// - `L` lowers instructions ([`MetadataLowering`]; see
///   [`without_lowering`](Self::without_lowering) and
///   [`with_lowering`](Self::with_lowering)).
/// - `C` parses the `.global` block ([`NoContext`]; see
///   [`with_global_context`](Self::with_global_context)).
pub struct ProgramChecker<M, H = StandardHeader, L = MetadataLowering, C = NoContext> {
    lowering: L,
    required: Option<Vec<String>>,
    marker: Marker<M, H, C>,
}

type Marker<M, H, C> = PhantomData<fn() -> (M, H, C)>;

impl<M> ProgramChecker<M> {
    /// The standard checker: [`StandardHeader`], [`MetadataLowering`], and
    /// [`NoContext`]. Every section is checked.
    pub fn new() -> Self {
        Self {
            lowering: MetadataLowering,
            required: None,
            marker: PhantomData,
        }
    }
}

impl<M> Default for ProgramChecker<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M, H, L, C> ProgramChecker<M, H, L, C> {
    /// Parse headers with `H2`, for example `Declarations<MyDeclaration>`.
    pub fn with_header<H2>(self) -> ProgramChecker<M, H2, L, C> {
        ProgramChecker {
            lowering: self.lowering,
            required: self.required,
            marker: PhantomData,
        }
    }

    /// Parse the `.global` block as `C2`.
    pub fn with_global_context<C2>(self) -> ProgramChecker<M, H, L, C2> {
        ProgramChecker {
            lowering: self.lowering,
            required: self.required,
            marker: PhantomData,
        }
    }

    /// Check syntax, symbols, and metadata, but do not lower instructions.
    pub fn without_lowering(self) -> ProgramChecker<M, H, NoLowering, C> {
        self.with_lowering_strategy(NoLowering)
    }

    /// Lower with a context prepared from each section's metadata and symbols.
    pub fn with_lowering<F, Ctx>(
        self,
        prepare: F,
    ) -> ProgramChecker<M, H, ContextLowering<F, Ctx>, C> {
        self.with_lowering_strategy(ContextLowering::new(prepare))
    }

    /// Lower with any [`Lowering`] strategy.
    pub fn with_lowering_strategy<L2>(self, lowering: L2) -> ProgramChecker<M, H, L2, C> {
        ProgramChecker {
            lowering,
            required: self.required,
            marker: PhantomData,
        }
    }

    /// Check only the named sections, and report each one that is missing.
    pub fn require_sections<S: Into<String>>(mut self, names: impl IntoIterator<Item = S>) -> Self {
        self.required = Some(names.into_iter().map(Into::into).collect());
        self
    }

    /// Check a whole file.
    pub fn check(&mut self, source: &SourceFile) -> Report
    where
        Self: SectionChecker,
    {
        check_file(source, self)
    }
}

/// Narrow an instruction span to the operand a lowering error names, such as
/// `@helper` in ``undefined symbol `@helper` `` or `"hi"` in a message
/// quoting `` `hi` ``.
fn operand_span(source: &SourceFile, instruction: Span, message: &str) -> Span {
    let text = source.slice(instruction);
    for quoted in message.split('`').skip(1).step_by(2) {
        let needle = if quoted.starts_with('@') {
            quoted.to_owned()
        } else {
            format!("\"{quoted}\"")
        };
        let bounded = text.match_indices(&needle).find(|(index, _)| {
            text[index + needle.len()..]
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || matches!(c, ',' | ')' | ']' | '}' | ';'))
        });
        if let Some((index, _)) = bounded {
            let start = instruction.start + index;
            return Span::new(start, start + needle.len());
        }
    }
    instruction
}

impl<M, H, L, C> SectionChecker for ProgramChecker<M, H, L, C>
where
    M: HasInstructionSet,
    for<'src> M::Syntax: Parse<'src>,
    M::Syntax: 'static,
    H: CheckHeader,
    H::Type: Clone,
    L: Lowering<M::Syntax, H::Value, H::Type>,
    C: SSTGlobalContext,
{
    fn check_global(&mut self, global: &super::Block<'_>, out: &mut Diagnostics<'_>) {
        check_global_context::<C>(global, global.span(), out);
    }

    fn check_section(&mut self, section: &Section<'_>, out: &mut Diagnostics<'_>) {
        if let Some(required) = &self.required
            && !required.iter().any(|name| name == section.name())
        {
            return;
        }
        let start_errors = out.error_count();
        let header = H::parse(section, out);
        let before_text = out.error_count();
        let text = parse_instructions::<M::Syntax>(section.text(), out);
        let text_ok = out.error_count() == before_text;
        metadata::check_duplicate_symbols(&text.symbols, out);

        let Some(header) = header else { return };
        let Some(checked) = header.metadata(section, out) else {
            return;
        };
        if !text_ok {
            // Missing symbols and lowering failures could be caused by the
            // syntax errors already reported.
            return;
        }
        let program = &checked.metadata;
        metadata::check_bindings(program, &checked.functions, &text.symbols, out);

        let anchor = section.block_anchor(section.header());
        let symbols: Vec<Symbol> = text.symbols.iter().map(|s| s.value.clone()).collect();
        if out.error_count() == start_errors
            && let Err(error) = program.validate(&symbols, text.code.len())
        {
            // A rule this checker does not model; report the loader's message.
            out.error(anchor, format!("{error:#}"), "in this header");
        }

        // Lower against the first definition of each symbol so that a
        // duplicate (already reported) does not hide unrelated problems.
        let mut unique = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            if !unique.iter().any(|seen: &Symbol| seen.name == symbol.name) {
                unique.push(symbol);
            }
        }
        let (spans, code): (Vec<Span>, Vec<M::Syntax>) = text
            .code
            .into_iter()
            .map(|item| (Span::from(item.span), item.value))
            .unzip();
        match self.lowering.lower_all(program, &unique, code) {
            Ok(failures) => {
                for (index, error) in failures {
                    let message = format!("{error:#}");
                    let span = operand_span(out.source(), spans[index], &message);
                    out.error(span, message, "cannot be lowered");
                }
            }
            Err(error) => {
                if out.error_count() == start_errors {
                    out.error(
                        anchor,
                        format!("cannot prepare lowering: {error:#}"),
                        "for this header",
                    );
                }
            }
        }
    }

    fn finish(&mut self, framing: &Framing<'_>, out: &mut Diagnostics<'_>) {
        let Some(required) = &self.required else {
            return;
        };
        for name in required {
            if framing.section(name).is_none() {
                out.error(framing.version, format!("missing section `{name}`"), "");
            }
        }
    }
}

/// Check every section of `source` as a program for machine `M`, with the
/// standard directive header and `MetadataLoweringContext` lowering.
pub fn check_program<M>(source: &SourceFile) -> Report
where
    M: HasInstructionSet,
    for<'src> M::Syntax: Parse<'src>,
    M::Syntax: Lower<MetadataLoweringContext> + 'static,
{
    ProgramChecker::<M>::new().check(source)
}

/// Check every section of `source` as a program for machine `M` with header
/// type `H`, lowering with `MetadataLoweringContext`.
pub fn check_machine<M, H>(source: &SourceFile) -> Report
where
    M: HasInstructionSet,
    for<'src> M::Syntax: Parse<'src>,
    M::Syntax: Lower<MetadataLoweringContext> + 'static,
    H: CheckHeader,
    H::Type: Clone,
{
    ProgramChecker::<M>::new().with_header::<H>().check(source)
}
