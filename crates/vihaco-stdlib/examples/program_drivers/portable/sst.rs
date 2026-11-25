// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Check one SST file and load each named section into its own image.

use super::digitizer::{Instruction, Machine, SyntaxInstruction};
use eyre::{Result, ensure, eyre};
use vihaco_bytecode::{NoContext, SSTFile};
use vihaco_module::{
    loader::MetadataLoweringContext,
    module::{NoInfo, ProgramDirective, ProgramHeader},
};
use vihaco_sst::{ProgramChecker, SourceFile};
use vihaco_stdlib::Image;
use vihaco_syntax::{HeaderDeclarations, ParsedModule, lower_code_with_metadata};

pub(crate) const SOURCE: &str = include_str!("digitizers.sst");
pub(crate) const SECTIONS: [&str; 2] = ["digitizer-a", "digitizer-b"];

pub(crate) fn load(source: &str) -> Result<[Image<Instruction>; 2]> {
    let input = SourceFile::new("digitizers.sst", source);
    let report = ProgramChecker::<Machine>::new()
        .require_sections(SECTIONS)
        .check(&input);
    ensure!(!report.has_errors(), "{}", report.render(&input));

    let file = SSTFile::<NoContext>::from_text(source)?;
    let [a, b] = SECTIONS.map(|name| {
        let section = file
            .section(name)
            .ok_or_else(|| eyre!("missing section {name}"))?;
        let ParsedModule {
            header,
            code,
            symbols,
        } = ParsedModule::<SyntaxInstruction, HeaderDeclarations<ProgramDirective>>::parse_section(
            section,
        )?;
        let mut metadata = header
            .into_iter()
            .collect::<ProgramHeader>()
            .assemble(NoInfo)
            .map_err(|errors| eyre!("invalid program header: {errors:?}"))?;
        let mut context = MetadataLoweringContext::new(&metadata, &symbols)?;
        let code = lower_code_with_metadata(code, &symbols, &metadata, &mut context)?;
        metadata.strings = context.into_strings();
        Ok::<_, eyre::Report>(Image::new(metadata.into_module(code, symbols)?))
    });
    Ok([a?, b?])
}
