// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Disassemble loaded programs back to SST text.
//!
//! A [`Disassembler`] turns loaded sections (`LocalModule`s) into one SST
//! file. Each section's header is rebuilt from the module's tables
//! (`vihaco_module::raising::header_directives`, plus the application's
//! [`ExtraDirectives`]), and each instruction is raised back to its source
//! syntax with [`Raise`] and a [`RaiseContext`]. The result is written by
//! the same writer as the formatter, so its layout and generated `;;` notes
//! are those of `format_program`, and it is checked and read back before it
//! is returned.

use std::collections::HashSet;

use eyre::WrapErr;
use vihaco_abi::{Lower, Raise, Type, Value};
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_module::module::{
    ConstantValue, LocalModule, NoInfo, ProgramDirective, ProgramHeader, StandardDirective, Symbol,
};
use vihaco_module::raising::{ExtraDirectives, RaiseContext, header_directives};
use vihaco_parser::{Parse, Print};
use vihaco_runtime::HasInstructionSet;

use crate::check::{Declarations, ProgramChecker, Section, SectionChecker};
use crate::diagnostic::{Diagnostics, Report};
use crate::format::{
    Block, BlockModel, Declaration, FileFrame, FileRoutes, FormatOptions, FormattedSection,
    Formatter, SectionFormat, SectionFrame, TextItem, faithful_declaration, faithful_instruction,
    reads_back_as, snapshot, write_file,
};
use crate::source::SourceFile;

/// The name of the file a disassembly is checked as.
const DISASSEMBLY: &str = "<disassembly>";

/// Writes loaded programs as one SST file, one section per program, in the
/// order they are added.
///
/// ```ignore
/// let text = Disassembler::new()
///     .section::<Controller>("controller", &controller_module)
///     .section_with::<Fpga, FpgaDeclaration>("fpga", &fpga_module)
///     .finish(FormatOptions::default())?;
/// ```
///
/// For each section:
///
/// - The header holds the application directives of the module's `extra`
///   metadata ([`ExtraDirectives`]; none for `NoInfo`), then the standard
///   directives rebuilt from its tables: `.file`, `.func` in table order,
///   `.entry`, `.const`, `.source_symbol`.
/// - `.text` holds every symbol of the module's symbol table and every
///   instruction, raised with a [`RaiseContext`], in address order. A
///   function without a symbol gets one named after it, and every address a
///   branch targets that has no symbol gets a synthesized label `@L` plus
///   its address in four or more lowercase hex digits (`@L0006`), with a
///   numeric suffix if that name is taken (`@L0006_1`).
///
/// The output is checked with each section's checker and read back before
/// it is returned. Loading it gives back the same code, functions,
/// constants, entry, and source symbols, the symbol table plus any
/// synthesized symbols, and string operands with the same text. A program
/// that SST cannot express exactly is an error rather than a different
/// program: a call to an address where no function starts, a literal word
/// that no literal lowers to, a value without a faithful spelling (such as a
/// NaN `f64`), or a function whose range does not end at the next function.
#[derive(Default)]
pub struct Disassembler<'a> {
    sections: Vec<Box<dyn Listing + 'a>>,
}

impl<'a> Disassembler<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add section `name`: `module`, a program for machine `M` with the
    /// standard header, checked with `ProgramChecker::<M>::new()`.
    #[must_use]
    pub fn section<M>(
        self,
        name: impl Into<String>,
        module: &'a LocalModule<
            M::Runtime,
            Value,
            Type,
            impl ExtraDirectives<ProgramDirective> + 'a,
        >,
    ) -> Self
    where
        M: HasInstructionSet + 'a,
        M::Runtime: Raise<RaiseContext, Syntax = M::Syntax> + 'a,
        for<'src> M::Syntax: Parse<'src>,
        M::Syntax: Print + Lower<MetadataLoweringContext> + 'static,
    {
        self.section_with_formatter(name, module, Formatter::<M>::new())
    }

    /// Add section `name`: `module`, a program for machine `M` whose header
    /// holds declarations `D`, such as an application declaration enum,
    /// checked with `ProgramChecker::<M>::new().with_header::<Declarations<D>>()`.
    #[must_use]
    pub fn section_with<M, D>(
        self,
        name: impl Into<String>,
        module: &'a LocalModule<M::Runtime, D::Value, D::Type, impl ExtraDirectives<D> + 'a>,
    ) -> Self
    where
        M: HasInstructionSet + 'a,
        M::Runtime: Raise<RaiseContext, Syntax = M::Syntax> + 'a,
        for<'src> M::Syntax: Parse<'src>,
        M::Syntax: Print + Lower<MetadataLoweringContext> + 'static,
        for<'src> D: Parse<'src>,
        D: StandardDirective + From<ProgramDirective<D::Value, D::Type>> + Print + 'static,
        D::Value: ConstantValue + Clone + 'a,
        D::Type: Print + Clone + 'a,
    {
        let checker = ProgramChecker::<M>::new().with_header::<Declarations<D>>();
        self.section_with_formatter(name, module, Formatter::with_checker(checker))
    }

    /// Add section `name`: `module`, written and checked with `formatter`,
    /// for example one with annotators or a custom lowering context. The
    /// formatter's own options are not used.
    #[must_use]
    pub fn section_with_formatter<M, D, L, C>(
        mut self,
        name: impl Into<String>,
        module: &'a LocalModule<M::Runtime, D::Value, D::Type, impl ExtraDirectives<D> + 'a>,
        formatter: Formatter<M, D, L, C>,
    ) -> Self
    where
        M: HasInstructionSet + 'a,
        M::Runtime: Raise<RaiseContext, Syntax = M::Syntax> + 'a,
        for<'src> M::Syntax: Parse<'src>,
        M::Syntax: Print + 'static,
        for<'src> D: Parse<'src>,
        D: StandardDirective + From<ProgramDirective<D::Value, D::Type>> + Print + 'static,
        D::Value: ConstantValue + Clone + 'a,
        D::Type: Print + Clone + 'a,
        L: 'a,
        C: 'a,
        ProgramChecker<M, Declarations<D>, L, C>: SectionChecker,
    {
        self.sections.push(Box::new(ModuleListing {
            name: name.into(),
            module,
            formatter,
        }));
        self
    }

    /// Write every section and return the file.
    ///
    /// # Errors
    /// Fails if two sections share a name, if a section cannot be expressed
    /// in SST exactly (see [`Disassembler`]), or if the output does not check
    /// clean or read back as the same program, which is a bug.
    pub fn finish(mut self, options: FormatOptions) -> eyre::Result<String> {
        let mut seen = HashSet::new();
        for listing in &self.sections {
            eyre::ensure!(
                seen.insert(listing.name().to_owned()),
                "section `{}` is added more than once",
                listing.name()
            );
        }
        let source = SourceFile::new(DISASSEMBLY, "");
        let mut bodies = Vec::new();
        let mut snapshots = Vec::new();
        for listing in &mut self.sections {
            let mut out = Diagnostics::new(&source);
            let formatted = listing
                .write(&source, &options, &mut out)
                .wrap_err_with(|| format!("cannot disassemble section `{}`", listing.name()))?;
            let report = out.finish();
            if report.has_errors() {
                return Err(eyre::eyre!(
                    "cannot disassemble section `{}`: {}",
                    listing.name(),
                    messages(&report)
                ));
            }
            bodies.push(formatted.text);
            snapshots.push(formatted.snapshot);
        }
        let frame = FileFrame {
            preamble: Vec::new(),
            version_comment: None,
            loose: Vec::new(),
            global: None,
            sections: self
                .sections
                .iter()
                .map(|listing| SectionFrame::named(listing.name()))
                .collect(),
        };
        let text = write_file(&frame, &bodies);

        let expected = snapshot(&frame, snapshots);
        let output = SourceFile::new(DISASSEMBLY, text.as_str());
        let mut routes = Listings(&mut self.sections);
        if !reads_back_as(&mut routes, &output, &expected) {
            let report = routes.check(&output);
            return Err(eyre::eyre!(
                "the disassembly does not read back as the same program; this is a \
                 disassembler bug{}",
                if report.has_errors() {
                    format!(": {}", messages(&report))
                } else {
                    String::new()
                }
            ));
        }
        Ok(text)
    }
}

/// Every diagnostic, as `line:column: message`, joined.
fn messages(report: &Report) -> String {
    report
        .iter()
        .map(|diagnostic| diagnostic.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

/// One section of a disassembly.
trait Listing {
    fn name(&self) -> &str;

    /// Raise and write the section's body.
    fn write(
        &mut self,
        source: &SourceFile,
        options: &FormatOptions,
        out: &mut Diagnostics<'_>,
    ) -> eyre::Result<FormattedSection>;

    fn formatter(&mut self) -> &mut dyn SectionFormat;
}

struct ModuleListing<'a, M: HasInstructionSet, D: StandardDirective, L, C, Info> {
    name: String,
    module: &'a LocalModule<M::Runtime, D::Value, D::Type, Info>,
    formatter: Formatter<M, D, L, C>,
}

impl<M, D, L, C, Info> Listing for ModuleListing<'_, M, D, L, C, Info>
where
    M: HasInstructionSet,
    M::Runtime: Raise<RaiseContext, Syntax = M::Syntax>,
    for<'src> M::Syntax: Parse<'src>,
    M::Syntax: Print + 'static,
    for<'src> D: Parse<'src>,
    D: StandardDirective + From<ProgramDirective<D::Value, D::Type>> + Print + 'static,
    D::Value: ConstantValue + Clone,
    D::Type: Print + Clone,
    Info: ExtraDirectives<D>,
    ProgramChecker<M, Declarations<D>, L, C>: SectionChecker,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn write(
        &mut self,
        source: &SourceFile,
        options: &FormatOptions,
        out: &mut Diagnostics<'_>,
    ) -> eyre::Result<FormattedSection> {
        let blocks = blocks::<M, D, Info>(self.module)?;
        Ok(self
            .formatter
            .write_blocks(source, &self.name, &blocks, options, out))
    }

    fn formatter(&mut self) -> &mut dyn SectionFormat {
        &mut self.formatter
    }
}

/// The `.header` and `.text` blocks that describe `module`.
fn blocks<M, D, Info>(
    module: &LocalModule<M::Runtime, D::Value, D::Type, Info>,
) -> eyre::Result<Vec<Block<M::Syntax, D>>>
where
    M: HasInstructionSet,
    M::Runtime: Raise<RaiseContext, Syntax = M::Syntax>,
    for<'src> M::Syntax: Parse<'src>,
    M::Syntax: Print + 'static,
    for<'src> D: Parse<'src>,
    D: StandardDirective + From<ProgramDirective<D::Value, D::Type>> + Print + 'static,
    D::Value: ConstantValue + Clone,
    D::Type: Clone,
    Info: ExtraDirectives<D>,
{
    let mut symbols = function_symbols(module)?;

    // Raise once to find the branch targets that have no symbol, name them,
    // and raise again with the complete table.
    let probe = RaiseContext::with_symbols(module, &symbols)?;
    let first: eyre::Result<Vec<M::Syntax>> = module
        .code
        .iter()
        .map(|instruction| instruction.raise(&probe))
        .collect();
    let unlabeled = probe.unlabeled();
    let code = if unlabeled.is_empty()
        && let Ok(code) = first
    {
        code
    } else {
        let labels = synthesize_labels(&symbols, &unlabeled);
        symbols.extend(labels);
        let context = RaiseContext::with_symbols(module, &symbols)?;
        module
            .code
            .iter()
            .enumerate()
            .map(|(address, instruction)| {
                instruction
                    .raise(&context)
                    .wrap_err_with(|| format!("cannot raise the instruction at {address:#06x}"))
            })
            .collect::<eyre::Result<_>>()?
    };
    for (address, instruction) in code.iter().enumerate() {
        faithful_instruction(instruction)
            .map_err(|message| eyre::eyre!("the instruction at {address:#06x}: {message}"))?;
    }

    let standard = header_directives(module)?;
    check_ranges(module, &standard, &symbols)?;
    let mut header = module.extra.extra_directives()?;
    header.extend(standard.into_iter().map(D::from));
    for declaration in &header {
        faithful_declaration(declaration).map_err(|message| eyre::eyre!("{message}"))?;
    }

    // Symbols in address order, keeping table order at each address.
    symbols.sort_by_key(|symbol| symbol.address);
    let mut symbols = symbols.into_iter().peekable();
    let mut items = Vec::with_capacity(code.len() + symbols.len());
    for (address, instruction) in code.into_iter().enumerate() {
        while let Some(symbol) = symbols.next_if(|symbol| symbol.address as usize <= address) {
            items.push(TextItem::Symbol(symbol));
        }
        items.push(TextItem::Instruction(instruction));
    }
    items.extend(symbols.map(TextItem::Symbol));
    Ok(vec![
        Block::Header(BlockModel::bare(header.into_iter().map(Declaration::new))),
        Block::Text(BlockModel::bare(items)),
    ])
}

/// The module's symbol table, plus a symbol for each function that has none.
fn function_symbols<I, V, Ty, Info>(
    module: &LocalModule<I, V, Ty, Info>,
) -> eyre::Result<Vec<Symbol>> {
    let mut symbols = module.symbols.clone();
    for (index, function) in module.functions.iter().enumerate() {
        let name = module.strings.get(function.name as usize).ok_or_else(|| {
            eyre::eyre!(
                "functions[{index}].name: index {} is outside the string table of length {}",
                function.name,
                module.strings.len()
            )
        })?;
        match symbols.iter().find(|symbol| symbol.name == *name) {
            Some(symbol) => eyre::ensure!(
                symbol.address == function.start_address,
                "function `@{name}` starts at {:#06x}, but symbol `@{name}` is at {:#06x}",
                function.start_address,
                symbol.address
            ),
            None => symbols.push(Symbol {
                name: name.clone(),
                address: function.start_address,
            }),
        }
    }
    Ok(symbols)
}

/// A label for each address: `L` and the address in four or more lowercase
/// hex digits, with a numeric suffix if that name is taken.
fn synthesize_labels(symbols: &[Symbol], addresses: &[u32]) -> Vec<Symbol> {
    let mut taken: HashSet<String> = symbols.iter().map(|symbol| symbol.name.clone()).collect();
    addresses
        .iter()
        .map(|&address| {
            let base = format!("L{address:04x}");
            let mut name = base.clone();
            let mut suffix = 0;
            while taken.contains(&name) {
                suffix += 1;
                name = format!("{base}_{suffix}");
            }
            taken.insert(name.clone());
            Symbol { name, address }
        })
        .collect()
}

/// Require that the rebuilt header gives every function the range it has.
fn check_ranges<I, V, Ty, Info>(
    module: &LocalModule<I, V, Ty, Info>,
    standard: &[ProgramDirective<V, Ty>],
    symbols: &[Symbol],
) -> eyre::Result<()>
where
    V: ConstantValue + Clone,
    Ty: Clone,
{
    let metadata = standard
        .iter()
        .cloned()
        .collect::<ProgramHeader<V, Ty>>()
        .finish(NoInfo)?;
    let functions = metadata.resolve_functions(symbols, module.code.len())?;
    for (index, (function, rebuilt)) in module.functions.iter().zip(&functions).enumerate() {
        eyre::ensure!(
            function.end_address == rebuilt.end_address,
            "functions[{index}] ends at {:#06x}, but in SST a function ends where the next one \
             starts ({:#06x})",
            function.end_address,
            rebuilt.end_address
        );
    }
    Ok(())
}

/// The listings as the formatter's routes, for checking and reading back.
struct Listings<'l, 'a>(&'l mut [Box<dyn Listing + 'a>]);

impl FileRoutes for Listings<'_, '_> {
    fn check(&mut self, source: &SourceFile) -> Report {
        crate::check::check_file(source, self)
    }

    fn route(&mut self, name: &str) -> Option<&mut dyn SectionFormat> {
        let listing = self.0.iter_mut().find(|listing| listing.name() == name)?;
        Some(listing.formatter())
    }
}

impl SectionChecker for Listings<'_, '_> {
    fn check_section(&mut self, section: &Section<'_>, out: &mut Diagnostics<'_>) {
        match FileRoutes::route(self, section.name()) {
            Some(formatter) => formatter.checker().check_section(section, out),
            None => out.error(
                section.name_span(),
                format!("unexpected section `{}`", section.name()),
                "not disassembled",
            ),
        }
    }
}
