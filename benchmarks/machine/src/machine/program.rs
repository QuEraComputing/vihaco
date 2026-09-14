// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Full SST loading and immutable code shared by prepared invocations.

use eyre::{Result, ensure, eyre};
use vihaco::{
    NoContext, ProgramImage, SstFile, Type,
    syntax::{ParsedModule, Resolve},
};
use vihaco_cpu::{RuntimeInstruction as Inst, SurfaceType, Word};

use super::fixture;
use crate::resolve::{EmptyHeader, Resolver};

/// Parsed code and entry metadata. Loading and cloning happen outside timing.
pub struct Program {
    pub(super) direct: Vec<Inst>,
    pub(super) image: ProgramImage<fixture::runtime::Instruction, NoContext, Word, Type>,
    pub(super) main: usize,
}

impl Program {
    /// Load a full SST file through vihaco's container and typed module parsers.
    /// Machine-specific resolution supplies CPU words and symbol addresses.
    pub fn parse(source: &str) -> Result<Self> {
        // Loading limits catch oversized fixtures before parsing or allocation.
        // Execution deadlines are enforced by the runner, outside timed loops.
        ensure!(
            source.len() <= 1_048_576,
            "benchmark SST source exceeds size limit"
        );
        let file = SstFile::<NoContext>::from_text(source)?;
        let section = file.root();
        ensure!(
            section.children().next().is_none(),
            "benchmark fixture has no child sections"
        );
        let parsed =
            ParsedModule::<fixture::syntax::Instruction, SurfaceType, EmptyHeader>::parse_section(
                section.clone(),
            )?;
        let module = Resolver::default().resolve_module(parsed)?;
        let main = module.main_function.ok_or_else(|| eyre!("missing @main"))? as usize;
        let image = ProgramImage {
            pc: module.functions[main].start_address,
            module,
            context: Some(section.context_handle()),
        };
        let direct = image
            .module
            .code
            .iter()
            .map(|fixture::runtime::Instruction::Cpu(inst)| inst.clone())
            .collect();
        Ok(Self {
            direct,
            image,
            main,
        })
    }
}
