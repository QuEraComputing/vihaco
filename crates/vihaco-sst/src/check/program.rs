// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The typed checker for one machine's instruction set.

use std::marker::PhantomData;

use vihaco_abi::{Lower, Value};
use vihaco_bytecode::{NoContext, SSTGlobalContext};
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_module::module::{ProgramMetadata, Symbol};
use vihaco_parser::Parse;
use vihaco_runtime::HasInstructionSet;
use vihaco_syntax::Spanned;

use super::metadata::{self, Site};
use super::{
    Framing, Section, SectionChecker, check_file, check_global_context, parse_declarations,
    parse_instructions,
};
use crate::diagnostic::{Diagnostics, Report};
use crate::source::{SourceFile, Span};

/// Table references held by a constant value, checked against the metadata
/// tables. Custom value types without references can use the defaults.
pub trait ConstantReferences {
    /// The `metadata.strings` index this constant refers to, if any.
    fn string_index(&self) -> Option<u32> {
        None
    }

    /// The `metadata.functions` index this constant refers to, if any.
    fn function_index(&self) -> Option<u32> {
        None
    }
}

impl ConstantReferences for Value {
    fn string_index(&self) -> Option<u32> {
        match self {
            Value::String(id) => Some(*id),
            _ => None,
        }
    }

    fn function_index(&self) -> Option<u32> {
        match self {
            Value::FunctionRef(id) => Some(*id),
            _ => None,
        }
    }
}

/// A header declaration that may carry the section's program metadata.
///
/// Implemented for `ProgramMetadata` itself; implement it for an application
/// declaration enum by returning the metadata variant.
pub trait MetadataDeclaration {
    type Value;
    type Type;
    type Info;

    fn as_metadata(&self) -> Option<&ProgramMetadata<Self::Value, Self::Type, Self::Info>>;
}

impl<V, Ty, Info> MetadataDeclaration for ProgramMetadata<V, Ty, Info> {
    type Value = V;
    type Type = Ty;
    type Info = Info;

    fn as_metadata(&self) -> Option<&ProgramMetadata<V, Ty, Info>> {
        Some(self)
    }
}

/// How a [`ProgramChecker`] parses a section header and finds its metadata.
pub trait CheckHeader: Sized {
    type Value;
    type Type;
    type Info;

    /// Parse the section's `.header` block, reporting syntax errors. Return
    /// `None` if the header is unusable.
    fn parse(section: &Section<'_>, out: &mut Diagnostics<'_>) -> Option<Self>;

    /// The section's program metadata and the file span of the declaration
    /// that supplied it, reporting header-level rules such as a missing or
    /// duplicate declaration. Field problems are located inside that span.
    fn metadata(
        &self,
        section: &Section<'_>,
        out: &mut Diagnostics<'_>,
    ) -> Option<LocatedMetadata<'_, Self>>;
}

/// A header's program metadata and the file span of its declaration.
pub type LocatedMetadata<'h, H> = (
    &'h ProgramMetadata<
        <H as CheckHeader>::Value,
        <H as CheckHeader>::Type,
        <H as CheckHeader>::Info,
    >,
    Span,
);

/// Located header declarations, one per line: the grammar of
/// `vihaco_syntax::HeaderDeclarations<D>`, with spans.
///
/// As a [`CheckHeader`], exactly one declaration must carry program metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct Declarations<D>(pub Vec<Spanned<D>>);

/// The standard header: `HeaderDeclarations<ProgramMetadata>` syntax, with
/// one `metadata { ... }` declaration.
pub type StandardHeader = Declarations<ProgramMetadata>;

impl<D> CheckHeader for Declarations<D>
where
    for<'src> D: Parse<'src>,
    D: MetadataDeclaration + 'static,
{
    type Value = D::Value;
    type Type = D::Type;
    type Info = D::Info;

    fn parse(section: &Section<'_>, out: &mut Diagnostics<'_>) -> Option<Self> {
        parse_declarations::<D>(section.header(), out).map(Declarations)
    }

    fn metadata(
        &self,
        section: &Section<'_>,
        out: &mut Diagnostics<'_>,
    ) -> Option<LocatedMetadata<'_, Self>> {
        let mut found: Option<(&ProgramMetadata<_, _, _>, Span)> = None;
        for declaration in &self.0 {
            let Some(metadata) = declaration.value.as_metadata() else {
                continue;
            };
            let span = Span::from(declaration.span.clone());
            match &found {
                None => found = Some((metadata, span)),
                Some((_, first)) => {
                    let head = |span: Span| Site::new(out.source(), span).head();
                    let diagnostic = out
                        .build_error(
                            head(span),
                            format!(
                                "section `{}` declares program metadata more than once",
                                section.name()
                            ),
                        )
                        .with_label("declared again here")
                        .with_secondary(head(*first), "first declared here");
                    out.push(diagnostic);
                }
            }
        }
        if found.is_none() {
            let span = section.block_anchor(section.header());
            let diagnostic = out
                .build_error(
                    span,
                    format!(
                        "section `{}` has no program metadata declaration",
                        section.name()
                    ),
                )
                .with_label("expected `metadata { ... }` in this header")
                .with_note(
                    "every loadable section declares its strings, functions, constants, \
                     entry function, source symbols, and extra metadata",
                );
            out.push(diagnostic);
        }
        found
    }
}

/// Lowers a section's instructions during checking, reporting failures per
/// instruction. Nothing is executed and the lowered code is discarded.
pub trait Lowering<I, V, Ty, Info> {
    /// Lower every instruction in `code`, returning the index and error of
    /// each one that failed. An outer error means no context could be built.
    fn lower_all(
        &mut self,
        metadata: &ProgramMetadata<V, Ty, Info>,
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

impl<I, V, Ty, Info> Lowering<I, V, Ty, Info> for NoLowering {
    fn lower_all(
        &mut self,
        _: &ProgramMetadata<V, Ty, Info>,
        _: &[Symbol],
        _: Vec<I>,
    ) -> eyre::Result<Vec<(usize, eyre::Report)>> {
        Ok(Vec::new())
    }
}

/// Lower with [`MetadataLoweringContext`], the standard loading path: catches
/// unresolved `@name` operands and string literals missing from
/// `metadata.strings`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MetadataLowering;

impl<I, V, Ty, Info> Lowering<I, V, Ty, Info> for MetadataLowering
where
    I: Lower<MetadataLoweringContext>,
{
    fn lower_all(
        &mut self,
        metadata: &ProgramMetadata<V, Ty, Info>,
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

impl<I, V, Ty, Info, F, C> Lowering<I, V, Ty, Info> for ContextLowering<F, C>
where
    F: FnMut(&ProgramMetadata<V, Ty, Info>, &[Symbol]) -> eyre::Result<C>,
    I: Lower<C>,
{
    fn lower_all(
        &mut self,
        metadata: &ProgramMetadata<V, Ty, Info>,
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
/// symbols, metadata table errors, function declarations without a symbol,
/// and (with the default [`MetadataLowering`]) instructions that fail to
/// lower, such as unresolved `@name` operands or missing string literals. It
/// never executes anything.
///
/// - `H` parses the header and finds the metadata ([`StandardHeader`] by
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
/// `@helper` in ``undefined symbol `@helper` `` or `"hi"` in
/// ``string literal `hi` is missing``.
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
    H::Value: ConstantReferences,
    H::Type: Clone,
    L: Lowering<M::Syntax, H::Value, H::Type, H::Info>,
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
        let Some((program, declaration)) = header.metadata(section, out) else {
            return;
        };
        let site = Site::new(out.source(), declaration);
        metadata::check_tables(program, &site, out);
        if !text_ok {
            // Missing symbols and lowering failures could be caused by the
            // syntax errors already reported.
            return;
        }
        metadata::check_bindings(program, &text.symbols, &site, out);

        let symbols: Vec<Symbol> = text.symbols.iter().map(|s| s.value.clone()).collect();
        if out.error_count() == start_errors
            && let Err(error) = program.validate(&symbols, text.code.len())
        {
            // A rule this checker does not model; report the loader's message.
            out.error(site.head(), format!("{error:#}"), "in this metadata");
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
                        site.head(),
                        format!("cannot prepare lowering: {error:#}"),
                        "for this metadata",
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
/// standard `metadata { ... }` header and `MetadataLoweringContext`
/// lowering.
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
    H::Value: ConstantReferences,
    H::Type: Clone,
{
    ProgramChecker::<M>::new().with_header::<H>().check(source)
}
