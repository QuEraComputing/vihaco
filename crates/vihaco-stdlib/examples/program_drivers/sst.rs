// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Check named SST sections and lower them into images for a declared machine.

use eyre::{Result, ensure, eyre};
use vihaco_bytecode::{NoContext, SSTFile};
use vihaco_module::{
    loader::MetadataLoweringContext,
    module::{NoInfo, ProgramDirective, ProgramHeader},
};
use vihaco_runtime::{HasInstructionSet, Lower, Parse};
use vihaco_sst::{ProgramChecker, SourceFile};
use vihaco_stdlib::Image;
use vihaco_syntax::{HeaderDeclarations, ParsedModule};

pub(super) fn load_sections<M>(
    filename: &str,
    source: &str,
    sections: &[&str],
) -> Result<Vec<Image<M::Runtime>>>
where
    M: HasInstructionSet,
    M::Syntax:
        for<'src> Parse<'src> + Lower<MetadataLoweringContext, Runtime = M::Runtime> + 'static,
{
    let input = SourceFile::new(filename, source);
    let report = ProgramChecker::<M>::new()
        .require_sections(sections.iter().copied())
        .check(&input);
    ensure!(!report.has_errors(), "{}", report.render(&input));

    let file = SSTFile::<NoContext>::from_text(source)?;
    sections
        .iter()
        .map(|name| {
            let section = file
                .section(name)
                .ok_or_else(|| eyre!("missing section {name}"))?;
            let mut parsed =
                ParsedModule::<M::Syntax, HeaderDeclarations<ProgramDirective>>::parse_section(
                    section,
                )?;
            let metadata = std::mem::take(&mut parsed.header.0)
                .into_iter()
                .collect::<ProgramHeader>()
                .assemble(NoInfo)
                .map_err(|errors| eyre!("invalid program header: {errors:?}"))?;
            let mut context = MetadataLoweringContext::new(&metadata, &parsed.symbols)?;
            Ok(Image::new(
                parsed.lower_with_metadata(metadata, &mut context)?,
            ))
        })
        .collect()
}
